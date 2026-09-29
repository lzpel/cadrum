//! Generate mdbook markdown and README examples section from numbered examples.
//! 番号付き example から mdbook 用 markdown と README の Examples 節を生成する。
//!
//! Usage / 使い方:
//!   cargo run --example markdown -- docs/SUMMARY.md ./README.md
//!
//! 1. Discover NN_*.rs in examples/ / examples/ 配下の NN_*.rs を収集
//! 2. Run each example, collect outputs / 各 example を実行し生成物を回収
//! 3. Write SUMMARY.md + per-example .md / SUMMARY.md と各 example 用 .md を出力
//! 4. Update README.md ## Examples section / README.md の ## Examples 節を更新

use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A numbered example file with its source content.
/// 番号付き example ファイルとそのソースコード。
struct Entry {
	path: PathBuf,   // absolute path to the .rs file / .rs ファイルの絶対パス
	content: String, // source code / ソースコード
}

impl Entry {
	/// File stem, e.g. "01_primitives" / ファイル名（拡張子なし）
	fn stem(&self) -> &str {
		self.path.file_stem().unwrap().to_str().unwrap()
	}

	/// Numeric prefix, e.g. 1 for "01_primitives", 100 for "100_chijin".
	fn number(&self) -> usize {
		self.stem().split('_').next().unwrap().parse().unwrap()
	}

	/// Plain title without the numeric prefix, e.g. "primitives" or "write read".
	/// 数字プレフィックス除去 + `_` → 空白。
	fn plain_title(&self) -> String {
		self.stem().split_once('_').unwrap().1.replace('_', " ")
	}

	/// Display title, e.g. "Primitives" / 表示タイトル
	fn title(&self) -> String {
		let raw = self.plain_title();
		let mut chars = raw.chars();
		match chars.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
		}
	}

	/// GitHub-style slug for linking to `#### Title` anchors, e.g. "write-read".
	fn slug(&self) -> String {
		self.plain_title().replace(' ', "-")
	}

	/// First `//!` doc comment line as description / 冒頭の `//!` 行から説明文を抽出
	fn description(&self) -> &str {
		self.content.lines().find(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim()).unwrap_or("")
	}
}
// 生成物
type Output = (PathBuf, Vec<u8>); // (relative path, contents) / (相対パス, 内容)

fn main() {
	let entries: Vec<Entry> = collect_entries();
	let outputs: Vec<Output> = collect_outputs(&entries);

	// Each arg is a file path: dispatch by filename / 各引数をファイル名で判別して処理
	for arg in std::env::args().skip(1) {
		let path = PathBuf::from(&arg);
		let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
		if name.starts_with("SUMMARY") {
			update_summary(&path, &entries, &outputs);
		} else if name.starts_with("README") {
			update_readme(&path, &entries[..entries.partition_point(|e| e.number() < 100)], &outputs);
		} else {
			eprintln!("unknown target: {arg} (expected SUMMARY.md or README.md)");
		}
	}
}

/// Collect numbered example files (NN_*.rs) sorted by name.
/// 番号付き example (NN_*.rs) をファイル名順に収集する。
fn collect_entries() -> Vec<Entry> {
	let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
	let mut entries: Vec<Entry> = fs::read_dir(&examples_dir)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().into_string().ok()?;
			if name.len() >= 4 && name.ends_with(".rs") && name.as_bytes()[0].is_ascii_digit() && name.as_bytes()[1].is_ascii_digit() {
				let path = e.path();
				let content = fs::read_to_string(&path).ok()?;
				Some(Entry { path, content })
			} else {
				None
			}
		})
		.collect();
	entries.sort_by_key(Entry::number);
	entries
}

/// Run each example in a temp directory and collect all generated files, sorted by path.
/// 一時ディレクトリで各 example を実行し、生成されたファイルをパス順で回収する。
fn collect_outputs(entries: &[Entry]) -> Vec<Output> {
	let tmp = std::env::temp_dir().join("cadrum_examples");
	clean_dir(&tmp);

	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
	for entry in entries {
		let stem = entry.stem();
		eprintln!("running example: {stem}");
		let status = Command::new("cargo").args(["run", "--manifest-path", manifest.to_str().unwrap(), "--example", stem]).current_dir(&tmp).status().unwrap_or_else(|e| panic!("failed to run example {stem}: {e}"));
		assert!(status.success(), "example {stem} failed with {status}");
	}

	// Read all files from the temp directory / 一時ディレクトリの全ファイルを読み込む
	let mut outputs: Vec<Output> = fs::read_dir(&tmp)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let path = PathBuf::from(e.file_name());
			let contents = fs::read(e.path()).ok()?;
			Some((path, contents))
		})
		.collect();
	outputs.sort_by(|a, b| a.0.cmp(&b.0));

	let _ = fs::remove_dir_all(&tmp);
	outputs
}

fn update_template(path: &Path, insert: HashMap<String, String>) {
	let contents_old = fs::read_to_string(path).expect("failed to read template");
	let mut tag: Option<&str> = None;
	let mut contents_new = String::new();
	for line in contents_old.lines() {
		match (tag, line.trim().starts_with("<!--*")) {
			(Some(x), true) => {
				if x == line.trim() {
					tag = None;
					writeln!(contents_new, "{x}\n{}\n{x}", insert.get(x).map(String::as_str).unwrap_or("")).unwrap();
				} else {
					panic!("tag {x} is not closed")
				}
			}
			(Some(_), false) => continue,
			(None, true) => tag = Some(line.trim()),
			(None, false) => writeln!(contents_new, "{line}").unwrap(),
		}
	}
	assert!(tag.is_none(), "tag {tag:?} is not closed");
	fs::write(path, contents_new).unwrap();
	eprintln!("updated: {}", path.display());
}
/// Write example outputs and pages next to SUMMARY.md, and fill its `<!--*SUMMARY_EXAMPLES*-->` block.
fn update_summary(summary_path: &Path, entries: &[Entry], outputs: &[Output]) {
	let out_dir = summary_path.parent().unwrap();
	// Write example output files (svg, step, brep, etc.) / example の生成物を書き出す
	for (path, contents) in outputs {
		fs::write(out_dir.join(path), contents).unwrap();
	}

	// Build SUMMARY.md and individual pages / SUMMARY.md と個別ページを生成する
	let mut summary = String::from("\n");
	for entry in entries {
		summary.push_str(&format!("- [{}]({}.md)\n", entry.title(), entry.stem()));

		// Format assets as markdown / 生成物を markdown 形式に変換
		let assets = render_assets(entry, outputs);

		let desc_section = if entry.description().is_empty() { String::new() } else { format!("\n{}\n", entry.description()) };
		let assets_section = if assets.is_empty() { String::new() } else { format!("\n{}", assets) };
		let md = format!("# {}\n{}\n```rust\n{}\n```{}", entry.title(), desc_section, entry.content, assets_section);
		fs::write(out_dir.join(format!("{}.md", entry.stem())), md).unwrap();
	}

	update_template(summary_path, HashMap::from([("<!--*SUMMARY_EXAMPLES*-->".to_string(), summary)]));
}

/// Render README asset markdown for an entry: Output links + preview images.
fn render_assets(entry: &Entry, outputs: &[Output]) -> String {
	let stem = entry.stem();
	let links: Vec<String> = ["png", "step", "glb", "brep", "stl", "svg"].iter().filter_map(|ext| find_output(outputs, stem, ext).map(|v| format!("[{}]({})", format!("{}.{}", stem, ext), link_output(&v)))).collect();
	let previews = ["svg", "png"].iter().find_map(|ext| find_output(outputs, stem, ext).map(|out| format!("<img src='{}' alt='{}' width='360'/>", link_output(&out), stem)));
	format!("Output: {}\n\n{}", links.join(" | "), previews.unwrap_or_default())
}

/// Render the `## Usage` section: thumbnail table + install instructions.
fn render_gallery(entries: &[Entry], outputs: &[Output]) -> String {
	const COLS: usize = 4;
	let cells: Vec<[String; 2]> = entries
		.iter()
		.map(|entry| {
			let img = &find_output(outputs, entry.stem(), "png").map(link_output).unwrap_or_default();
			let th = format!("<a href='#{anchor}'>{title}</a>", anchor = entry.slug(), title = entry.plain_title());
			let td = format!("<a href='#{anchor}'><img src='{img}' width='100%' height='auto' alt='{title}'/></a>", anchor = entry.slug(), title = entry.plain_title());
			[th, td]
		})
		.collect();
	{
		let mut s = String::from("\n<table>\n");
		for chunk in cells.chunks(COLS) {
			for (i, tag) in ["th", "td"].iter().enumerate() {
				let row: String = (0..COLS).map(|col| format!("<{tag} width='25%'>{}</{tag}>", chunk.get(col).map(|cell| cell[i].as_str()).unwrap_or_default())).collect();
				s.push_str(&format!("<tr>{row}</tr>\n"));
			}
		}
		s.push_str("</table>\n");
		s
	}
}

/// Render the `## Example` section: every entry listed with `#### Title`.
fn render_example_section(entries: &[Entry], outputs: &[Output]) -> String {
	fn render_example(entry: &Entry, outputs: &[Output]) -> String {
		let header = format!("\n{}\n\n```sh\ncargo run --example {}\n```\n\n```rust,no_run\n{}\n```\n", entry.description(), entry.stem(), entry.content);
		let asset = render_assets(entry, outputs);
		format!("{}\n{}", header, asset)
	}
	let examples: Vec<String> = entries.iter().map(|entry| format!("\n#### {}\n{}", entry.title(), &render_example(entry, outputs))).collect();
	format!("{}\n", examples.join("\n"))
}

/// Fill the `<!--*GALLERY*-->` and `<!--*EXAMPLES*-->` blocks of README.md.
fn update_readme(readme_path: &Path, entries: &[Entry], outputs: &[Output]) {
	update_template(readme_path, HashMap::from([("<!--*GALLERY*-->".to_string(), render_gallery(entries, outputs)), ("<!--*EXAMPLES*-->".to_string(), render_example_section(entries, outputs))]));
}

/// Remove and recreate a directory.
/// ディレクトリを削除して再作成する。
fn clean_dir(dir: &Path) {
	if dir.exists() {
		fs::remove_dir_all(dir).expect("failed to clean directory");
	}
	fs::create_dir_all(dir).expect("failed to create directory");
}

fn find_output<'a>(outputs: &'a [Output], stem: &str, ext: &str) -> Option<&'a Output> {
	outputs.iter().find(|(p, _)| p.extension().is_some_and(|v| v == ext) && p.file_stem().is_some_and(|s| s == stem))
}

fn link_output(output: &Output) -> String {
	format!("https://lzpel.github.io/cadrum/{}", output.0.file_name().unwrap_or_default().to_string_lossy())
}
