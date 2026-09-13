//! Regenerate `////////// codegen.rs` marker regions in the given .rs files.
//!
//! Usage: `cargo run --example codegen -- src/traits.rs src/lib.rs`
//!
//! Each input file is both a trait-definition source AND a rewrite target;
//! trait defs are pooled across all inputs, then every file is rewritten in
//! place. Splitting traits / consumers across files (or merging them) doesn't
//! affect the result — just point codegen at the union of files involved.
//!
//! The marker line itself is preserved; everything from the next line down to
//! the closing `}` of the enclosing scope is replaced:
//!
//!   - inside `impl X { ... }`            → `XStruct` chain inherent methods (supertrait walk + dedup)
//!   - inside `pub trait X: Y, Z { ... }` → forwarder default methods for parent traits Y, Z
//!
//! Parser constraints:
//!
//!   - the trait header must fit on one line, `{` included
//!   - `#[cfg(...)]` attaches to the next fn only (single-line attribute)
//!
//! Each forwarder is emitted as a 3-line block — the canonical form rustfmt
//! produces under this repo's `rustfmt.toml` (`hard_tabs=true`, `max_width=1000`),
//! so `cargo fmt` and codegen never rewrite each other's output.

use std::collections::HashSet;

const MARKER: &str = "//////////";
const LIFETIME: char = '\'';

fn main() {
	let paths: Vec<String> = std::env::args().skip(1).collect();
	if paths.is_empty() {
		eprintln!("usage: cargo run --example codegen -- <file.rs> [<file.rs> ...]");
		eprintln!("       each file is parsed for trait defs AND rewritten in place at marker regions.");
		std::process::exit(1);
	}

	let sources: Vec<(String, String)> = paths.iter().map(|p| (p.clone(), std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {p}: {e}")))).collect();
	let traits: Vec<TraitDef> = sources.iter().flat_map(|(_, src)| parse_traits(src)).collect();

	for (path, original) in &sources {
		let updated = regenerate(original, &traits);
		if &updated == original {
			eprintln!("no diff {path}");
		} else {
			std::fs::write(path, &updated).unwrap_or_else(|e| panic!("write {path}: {e}"));
			eprintln!("updated {path}");
		}
	}
}

struct Method {
	cfg: Option<String>,
	signature: String,
	name: String,
	args: Vec<String>,
	has_self: bool,
	origin_trait: String,
}

struct TraitDef {
	name: String,
	supertraits: Vec<String>,
	methods: Vec<Method>,
}

fn is_marker(line: &str) -> bool {
	line.trim().strip_prefix(MARKER).is_some_and(|rest| rest.trim() == "codegen.rs")
}

fn leading_ident(s: &str) -> String {
	s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

fn is_word(c: Option<char>) -> bool {
	c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// `(name, supertraits)` for a one-line trait header, or `None` for any other line.
fn parse_trait_header(line: &str) -> Option<(String, Vec<String>)> {
	let trimmed = line.trim();
	let head = trimmed.strip_prefix("pub ").unwrap_or(trimmed).strip_prefix("trait ")?;
	let head = &head[..head.find('{')?];
	let (name, bounds) = head.split_once(':').unwrap_or((head, ""));
	// `where` は supertrait リストより先に切り落とす。`where` 節自身が `+` を含む bound
	// (例 `for<'a> &'a Self: Add + Sub`) を持つため、split('+') が先だと
	// `"Compound where for<'a> &'a Self: Add"` のような誤った supertrait 名が混入する。
	let bounds = bounds.split(" where ").next().unwrap_or(bounds);
	let supertraits = bounds.split('+').map(|b| b.trim().to_string()).filter(|b| !b.is_empty() && !b.starts_with(LIFETIME)).collect();
	Some((name.trim().to_string(), supertraits))
}

fn parse_traits(src: &str) -> Vec<TraitDef> {
	let lines: Vec<&str> = src.lines().collect();
	let mut traits = Vec::new();
	let mut i = 0;
	while i < lines.len() {
		let header = if lines[i].trim_start().starts_with("//") { None } else { parse_trait_header(lines[i]) };
		let Some((name, supertraits)) = header else {
			i += 1;
			continue;
		};
		let mut methods = Vec::new();
		let mut pending_cfg: Option<String> = None;
		i += 1;
		while i < lines.len() && lines[i].trim() != "}" {
			let l = lines[i].trim();
			if l.starts_with("#[cfg(") {
				pending_cfg = Some(l.to_string());
			} else if l.starts_with("fn ") {
				// rustfmt puts a `where` clause on its own line, so a signature may span
				// several lines. Join until `;` (declaration) or `{` (default body) ends it.
				let mut signature = l.to_string();
				while !signature.ends_with(';') && !signature.ends_with('{') && i + 1 < lines.len() {
					i += 1;
					signature.push(' ');
					signature.push_str(lines[i].trim());
				}
				if let Some(m) = parse_method(&signature, pending_cfg.take(), name.clone()) {
					methods.push(m);
				}
				let mut depth = usize::from(signature.ends_with('{'));
				while depth > 0 && i + 1 < lines.len() {
					i += 1;
					let body = lines[i].trim();
					depth += body.matches('{').count();
					depth = depth.saturating_sub(body.matches('}').count());
				}
			} else if !(l.starts_with("type ") || l.starts_with("//") || l.is_empty()) {
				pending_cfg = None;
			}
			i += 1;
		}
		traits.push(TraitDef { name, supertraits, methods });
		i += 1;
	}
	traits
}

fn parse_method(line: &str, cfg: Option<String>, origin_trait: String) -> Option<Method> {
	let line = line.trim_end_matches(';');
	let line = line.find('{').map_or(line, |brace| line[..brace].trim_end());
	let rest = &line[line.find("fn ")? + 3..];
	let paren_open = rest.find('(')?;
	let name = leading_ident(rest[..paren_open].trim());
	let args: Vec<&str> = split_args(&rest[paren_open + 1..rest.rfind(')')?]).into_iter().map(str::trim).filter(|a| !a.is_empty()).collect();
	let has_self = args.iter().any(|a| matches!(*a, "self" | "&self" | "mut self" | "&mut self"));
	let args = args.iter().filter_map(|a| a.split_once(':')).map(|(n, _)| n.trim().to_string()).collect();
	// A forwarder never needs the `where`: `Self` is concrete in the inherent impl, so
	// lifetime / assoc-type bounds are auto-satisfied, and emitting one would make rustfmt
	// break the block and fight codegen. Bounds the forwarder does need (loft's
	// `S: IntoIterator<Item = I>`) must be inline in the generic list instead.
	let signature = line.split(" where ").next().unwrap_or(line).trim().to_string();
	Some(Method { cfg, signature, name, args, has_self, origin_trait })
}

/// Split an argument list by `,` while respecting `<>` and `()` nesting.
fn split_args(s: &str) -> Vec<&str> {
	let mut result = Vec::new();
	let (mut angle, mut paren, mut start) = (0usize, 0usize, 0usize);
	for (i, b) in s.bytes().enumerate() {
		match b {
			b'<' => angle += 1,
			b'>' if angle > 0 => angle -= 1,
			b'(' => paren += 1,
			b')' if paren > 0 => paren -= 1,
			b',' if angle == 0 && paren == 0 => {
				result.push(&s[start..i]);
				start = i + 1;
			}
			_ => {}
		}
	}
	result.push(&s[start..]);
	result
}

/// Rewrite `Self` and the known associated types to concrete names, for `impl X`
/// rendering only — trait-body forwarders keep `Self` verbatim.
fn resolve_self(sig: &str, concrete: &str) -> String {
	let mut out = String::with_capacity(sig.len());
	let mut pos = 0;
	while let Some(at) = sig[pos..].find("Self").map(|off| pos + off) {
		out.push_str(&sig[pos..at]);
		pos = at + "Self".len();
		let tail = &sig[pos..];
		if is_word(sig[..at].chars().next_back()) || is_word(tail.chars().next()) {
			out.push_str("Self");
			continue;
		}
		let assoc = [("::Elem", concrete), ("::Edge", "Edge"), ("::Face", "Face")];
		match assoc.into_iter().find(|(path, _)| tail.strip_prefix(path).is_some_and(|t| !is_word(t.chars().next()))) {
			Some((path, name)) => {
				out.push_str(name);
				pos += path.len();
			}
			// `Self::Output` and other unknown associated types keep their prefix.
			None if tail.starts_with(':') => out.push_str("Self"),
			None => out.push_str(concrete),
		}
	}
	out.push_str(&sig[pos..]);
	out
}

fn collect_methods<'a>(td: &'a TraitDef, all: &'a [TraitDef], seen: &mut HashSet<String>, out: &mut Vec<&'a Method>) {
	for m in &td.methods {
		if seen.insert(m.name.clone()) {
			out.push(m);
		}
	}
	for parent in td.supertraits.iter().filter_map(|s| all.iter().find(|t| &t.name == s)) {
		collect_methods(parent, all, seen, out);
	}
}

fn emit(out: &mut Vec<String>, indent: &str, m: &Method, visibility: &str, signature: &str, trait_path: &str) {
	let args: Vec<&str> = m.has_self.then_some("self").into_iter().chain(m.args.iter().map(String::as_str)).collect();
	if let Some(cfg) = &m.cfg {
		out.push(format!("{indent}{cfg}"));
	}
	out.push(format!("{indent}{visibility}{signature} {{"));
	out.push(format!("{indent}\t<Self as {trait_path}>::{}({})", m.name, args.join(", ")));
	out.push(format!("{indent}}}"));
}

enum Context {
	Impl { ty: String },
	TraitBody { name: String },
}

fn classify_opener(line: &str) -> Context {
	let trimmed = line.trim();
	let trimmed = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
	if let Some(rest) = trimmed.strip_prefix("trait ") {
		return Context::TraitBody { name: leading_ident(rest) };
	}
	if let Some(rest) = trimmed.strip_prefix("impl").filter(|r| r.starts_with([' ', '<'])) {
		let rest = rest.trim_start();
		let rest = rest.strip_prefix('<').map_or(rest, |g| g.split_once('>').map_or("", |(_, r)| r).trim_start());
		return Context::Impl { ty: leading_ident(rest) };
	}
	panic!("unrecognized enclosing opener: {line}");
}

fn render(context: &Context, indent: &str, traits: &[TraitDef]) -> Vec<String> {
	let mut out = Vec::new();
	match context {
		Context::Impl { ty } => {
			let trait_name = format!("{ty}Struct");
			let td = traits.iter().find(|t| t.name == trait_name).unwrap_or_else(|| panic!("no trait `{trait_name}` for impl `{ty}`"));
			let (mut seen, mut methods) = (HashSet::new(), Vec::new());
			collect_methods(td, traits, &mut seen, &mut methods);
			let concrete = format!("crate::{ty}");
			for m in methods {
				emit(&mut out, indent, m, "pub ", &resolve_self(&m.signature, &concrete), &format!("crate::traits::{}", m.origin_trait));
			}
		}
		Context::TraitBody { name } => {
			let td = traits.iter().find(|t| &t.name == name).unwrap_or_else(|| panic!("no trait `{name}`"));
			for super_name in &td.supertraits {
				let Some(parent) = traits.iter().find(|t| &t.name == super_name) else { continue };
				for m in &parent.methods {
					emit(&mut out, indent, m, "", &m.signature, super_name);
				}
			}
		}
	}
	out
}

fn regenerate(src: &str, traits: &[TraitDef]) -> String {
	let lines: Vec<&str> = src.split('\n').collect();
	let mut depths: Vec<i32> = Vec::with_capacity(lines.len() + 1);
	depths.push(0);
	for line in &lines {
		let code = line.find("//").map_or(*line, |idx| &line[..idx]);
		depths.push(depths.last().unwrap() + code.matches('{').count() as i32 - code.matches('}').count() as i32);
	}

	let mut out: Vec<String> = Vec::with_capacity(lines.len());
	let mut i = 0;
	while i < lines.len() {
		out.push(lines[i].to_string());
		if !is_marker(lines[i]) {
			i += 1;
			continue;
		}
		let depth = depths[i];
		assert!(depth > 0, "marker at line {} is at module level — markers must be inside `impl X {{ ... }}` or `pub trait X: ... {{ ... }}`", i + 1);
		let opener = (0..i).rev().find(|&j| depths[j] == depth - 1 && depths[j + 1] > depth - 1).unwrap_or_else(|| panic!("could not find enclosing block opener for marker at line {}", i + 1));
		out.extend(render(&classify_opener(lines[opener]), &"\t".repeat(depth as usize), traits));
		i = (i + 1..lines.len()).find(|&j| depths[j + 1] < depth).unwrap_or(lines.len());
	}
	out.join("\n")
}
