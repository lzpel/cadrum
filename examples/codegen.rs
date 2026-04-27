//! Regenerate `////////// codegen.rs` marker regions in the given .rs files.
//!
//! Each input file is BOTH a trait-definition source AND a target for marker
//! rewriting — trait defs are pooled across all inputs, then every input file
//! is rewritten in place. This means it doesn't matter whether traits live in
//! their own file or are merged with the consumer; just point codegen at the
//! union of files involved.
//!
//! Marker syntax (the marker line itself is preserved; everything from the
//! line below the marker down to the closing `}` of the enclosing scope —
//! or to EOF when at module level — is replaced):
//!
//!     ////////// codegen.rs           — context inferred from enclosing block:
//!                                       inside `impl X { ... }`            → render `XStruct` chain inherent methods
//!                                       inside `pub trait X: Y, Z { ... }` → render forwarder defaults from supertraits Y, Z
//!     ////////// codegen.rs <Tag>     — module-level free fns delegating to `<Tag>Module`
//!
//! Run via:  `cargo run --example codegen -- src/traits.rs src/lib.rs`
//!
//! Replaces the previous `build.rs` → `OUT_DIR/generated_delegation.rs` →
//! `include!` flow with in-place rewrite so the result is checked into `src/`
//! and visible in PR diffs (mirroring how `examples/markdown.rs` updates
//! README.md).
//!
//! Parser constraints (carried over from the prior build_delegation.rs):
//!   - fn signatures must fit on one line (where/lifetime/generics included)
//!   - #[cfg(...)] attaches to the next fn only (single-line attribute)
//!   - default impl bodies may span multiple lines (skipped via brace counting)

use regex::Regex;
use std::collections::HashSet;

fn main() {
	let paths: Vec<String> = std::env::args().skip(1).collect();
	if paths.is_empty() {
		eprintln!("usage: cargo run --example codegen -- <file.rs> [<file.rs> ...]");
		eprintln!("       each file is parsed for trait defs AND rewritten in place at marker regions.");
		std::process::exit(1);
	}

	// Read every input file once so we can use the same buffer for both
	// parsing (pooling trait defs) and rewriting (comparing for diff).
	let sources: Vec<(String, String)> = paths
		.iter()
		.map(|p| (p.clone(), std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {}", p, e))))
		.collect();

	let mut traits: Vec<TraitDef> = Vec::new();
	for (_, src) in &sources {
		traits.extend(parse_traits(src));
	}

	for (path, original) in &sources {
		let updated = regenerate(original, &traits);
		if &updated != original {
			std::fs::write(path, &updated).unwrap_or_else(|e| panic!("write {}: {}", path, e));
			eprintln!("updated {}", path);
		} else {
			eprintln!("no diff {}", path);
		}
	}
}

// ============================ data types ============================

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

// ============================ parser ============================

fn parse_traits(src: &str) -> Vec<TraitDef> {
	let header_re = Regex::new(r"^\s*(?:pub\s+)?trait\s+(\w+)\s*(?::\s*([^{]+?))?\s*\{").unwrap();
	let mut traits = Vec::new();
	let lines: Vec<&str> = src.lines().collect();
	let mut i = 0;

	while i < lines.len() {
		let line = lines[i];
		if line.trim_start().starts_with("//") {
			i += 1;
			continue;
		}
		if let Some(caps) = header_re.captures(line) {
			let name = caps.get(1).unwrap().as_str().to_string();
			let supertraits: Vec<String> = caps.get(2).map_or_else(Vec::new, |s| {
				s.as_str()
					.split('+')
					.map(|p| p.trim().to_string())
					.filter(|p| !p.is_empty() && !p.starts_with('\''))
					.collect()
			});

			let mut methods = Vec::new();
			i += 1;
			let mut pending_cfg: Option<String> = None;
			while i < lines.len() {
				let l = lines[i].trim();
				if l == "}" {
					break;
				}
				if l.starts_with("#[cfg(") {
					pending_cfg = Some(l.to_string());
					i += 1;
					continue;
				}
				if l.starts_with("type ") || l.starts_with("//") || l.is_empty() {
					i += 1;
					continue;
				}
				if l.starts_with("fn ") {
					if let Some(m) = parse_method(l, pending_cfg.take(), name.clone()) {
						methods.push(m);
					}
					// Skip multi-line default impl body via brace counting.
					if l.ends_with('{') {
						let mut depth = 1usize;
						while depth > 0 && i + 1 < lines.len() {
							i += 1;
							let body = lines[i].trim();
							depth += body.matches('{').count();
							depth = depth.saturating_sub(body.matches('}').count());
						}
					}
				} else {
					pending_cfg = None;
				}
				i += 1;
			}
			traits.push(TraitDef { name, supertraits, methods });
		}
		i += 1;
	}
	traits
}

fn parse_method(line: &str, cfg: Option<String>, origin_trait: String) -> Option<Method> {
	let line = line.trim_end_matches(';');
	let line = if let Some(brace) = line.find('{') { line[..brace].trim_end() } else { line };
	let fn_idx = line.find("fn ")?;
	let rest = &line[fn_idx + 3..];
	let paren_open = rest.find('(')?;
	let name_with_generics = rest[..paren_open].trim();
	let name = name_with_generics
		.find('<')
		.map_or_else(|| name_with_generics.to_string(), |a| name_with_generics[..a].trim().to_string());
	let paren_close = rest.rfind(')')?;
	let args_str = &rest[paren_open + 1..paren_close];

	let mut has_self = false;
	let mut args = Vec::new();
	for arg in split_args(args_str) {
		let arg = arg.trim();
		if arg.is_empty() {
			continue;
		}
		if matches!(arg, "self" | "&self" | "mut self" | "&mut self") {
			has_self = true;
			continue;
		}
		if let Some(colon) = arg.find(':') {
			args.push(arg[..colon].trim().to_string());
		}
	}
	let signature = line[fn_idx..].trim().to_string();
	Some(Method { cfg, signature, name, args, has_self, origin_trait })
}

/// Split an argument list by `,` while respecting `<>` and `()` nesting.
/// regex can't help here — balanced brackets are not regular.
fn split_args(s: &str) -> Vec<&str> {
	let mut result = Vec::new();
	let mut angle = 0usize;
	let mut paren = 0usize;
	let mut start = 0;
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

// ============================ type substitution ============================
//
// In `impl X` rendering, associated types (`Self::Edge`/`Self::Face`/`Self::Solid`/
// `Self::Elem`) and bare `Self` get rewritten to concrete names. This mirrors
// the prior build_delegation.rs::resolve_types but uses regex word boundaries
// instead of hand-rolled identifier-char checks.

fn resolve_types_for_impl(sig: &str, concrete: &str) -> String {
	let mut s = sub_word(sig, r"\bSelf::Elem\b", concrete);
	s = sub_word(&s, r"\bSelf::Edge\b", "Edge");
	s = sub_word(&s, r"\bSelf::Face\b", "Face");
	s = sub_word(&s, r"\bSelf::Solid\b", "Solid");
	replace_self_bare(&s, concrete)
}

fn resolve_types_for_module(sig: &str, concrete: &str) -> String {
	let mut s = sub_word(sig, r"\bSelf::Edge\b", "Edge");
	s = sub_word(&s, r"\bSelf::Face\b", "Face");
	s = sub_word(&s, r"\bSelf::Solid\b", "Solid");
	replace_self_bare(&s, concrete)
}

fn sub_word(text: &str, pattern: &str, to: &str) -> String {
	Regex::new(pattern).unwrap().replace_all(text, to).into_owned()
}

/// Replace bare `Self` with `concrete`, but leave `Self:` (where-clause /
/// path-prefix usage like `Self::Output` from std traits) alone. The associated
/// types we DO know about are rewritten by earlier `sub_word` calls; this
/// guard catches the rest.
fn replace_self_bare(text: &str, concrete: &str) -> String {
	let re = Regex::new(r"\bSelf\b").unwrap();
	re.replace_all(text, |caps: &regex::Captures| {
		let end = caps.get(0).unwrap().end();
		if text[end..].starts_with(':') {
			"Self".to_string()
		} else {
			concrete.to_string()
		}
	})
	.into_owned()
}

// ============================ method aggregation ============================

fn collect_methods<'a>(td: &'a TraitDef, all: &'a [TraitDef]) -> Vec<&'a Method> {
	let mut seen: HashSet<String> = HashSet::new();
	let mut out: Vec<&Method> = Vec::new();
	for m in &td.methods {
		if seen.insert(m.name.clone()) {
			out.push(m);
		}
	}
	walk_supers(&td.supertraits, all, &mut seen, &mut out);
	out
}

fn walk_supers<'a>(supers: &[String], all: &'a [TraitDef], seen: &mut HashSet<String>, out: &mut Vec<&'a Method>) {
	for super_name in supers {
		let Some(parent) = all.iter().find(|t| &t.name == super_name) else { continue };
		for m in &parent.methods {
			if seen.insert(m.name.clone()) {
				out.push(m);
			}
		}
		walk_supers(&parent.supertraits, all, seen, out);
	}
}

// ============================ region rewriting ============================

fn regenerate(src: &str, traits: &[TraitDef]) -> String {
	let lines: Vec<&str> = src.split('\n').collect();
	let depths = compute_depths(&lines);
	let marker_re = Regex::new(r"^\s*//////////\s+codegen\.rs(?:\s+(\w+))?\s*$").unwrap();

	let mut out: Vec<String> = Vec::with_capacity(lines.len());
	let mut cursor = 0usize;
	let mut i = 0usize;
	while i < lines.len() {
		if let Some(caps) = marker_re.captures(lines[i]) {
			let tag = caps.get(1).map(|m| m.as_str().to_string());
			for j in cursor..=i {
				out.push(lines[j].to_string());
			}
			let indent: String = lines[i].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
			let depth = depths[i];
			let region_end = compute_region_end(&depths, i, depth);
			let context = determine_context(&lines, i, &depths, depth, tag);
			out.extend(render(&context, &indent, traits));
			cursor = region_end;
			i = region_end;
		} else {
			i += 1;
		}
	}
	for j in cursor..lines.len() {
		out.push(lines[j].to_string());
	}
	out.join("\n")
}

fn compute_depths(lines: &[&str]) -> Vec<i32> {
	let mut depths = Vec::with_capacity(lines.len() + 1);
	depths.push(0i32);
	for line in lines {
		let stripped = strip_line_comment(line);
		let opens = stripped.matches('{').count() as i32;
		let closes = stripped.matches('}').count() as i32;
		depths.push(*depths.last().unwrap() + opens - closes);
	}
	depths
}

fn strip_line_comment(line: &str) -> String {
	line.find("//").map_or_else(|| line.to_string(), |idx| line[..idx].to_string())
}

fn compute_region_end(depths: &[i32], marker_idx: usize, marker_depth: i32) -> usize {
	let lines_len = depths.len() - 1;
	if marker_depth == 0 {
		return lines_len;
	}
	for j in (marker_idx + 1)..lines_len {
		if depths[j + 1] < marker_depth {
			return j;
		}
	}
	lines_len
}

enum Context {
	Impl { ty: String },
	TraitBody { name: String },
	Module { tag: String },
}

fn determine_context(lines: &[&str], marker_idx: usize, depths: &[i32], marker_depth: i32, tag: Option<String>) -> Context {
	if marker_depth == 0 {
		let tag = tag.unwrap_or_else(|| {
			panic!("module-level marker at line {} requires a tag like `////////// codegen.rs Io`", marker_idx + 1)
		});
		return Context::Module { tag };
	}
	let target = marker_depth - 1;
	let mut j = marker_idx;
	while j > 0 {
		j -= 1;
		if depths[j] == target && depths[j + 1] > target {
			return classify_opener(lines[j]);
		}
	}
	panic!("could not find enclosing block opener for marker at line {}", marker_idx + 1)
}

fn classify_opener(line: &str) -> Context {
	let trait_re = Regex::new(r"^\s*(?:pub\s+)?trait\s+(\w+)").unwrap();
	if let Some(caps) = trait_re.captures(line) {
		return Context::TraitBody { name: caps.get(1).unwrap().as_str().to_string() };
	}
	let impl_re = Regex::new(r"^\s*impl(?:\s*<[^>]*>)?\s+(\w+)").unwrap();
	if let Some(caps) = impl_re.captures(line) {
		return Context::Impl { ty: caps.get(1).unwrap().as_str().to_string() };
	}
	panic!("unrecognized enclosing opener: {}", line);
}

// ============================ rendering ============================

fn render(context: &Context, indent: &str, traits: &[TraitDef]) -> Vec<String> {
	match context {
		Context::Impl { ty } => render_impl(ty, indent, traits),
		Context::TraitBody { name } => render_trait_body(name, indent, traits),
		Context::Module { tag } => render_module(tag, traits),
	}
}

fn render_impl(ty: &str, indent: &str, traits: &[TraitDef]) -> Vec<String> {
	let trait_name = format!("{}Struct", ty);
	let td = traits
		.iter()
		.find(|t| t.name == trait_name)
		.unwrap_or_else(|| panic!("no trait `{}` for impl `{}`", trait_name, ty));
	let methods = collect_methods(td, traits);
	let concrete = format!("crate::{}", ty);

	let mut out = Vec::new();
	for m in methods {
		if let Some(cfg) = &m.cfg {
			out.push(format!("{}{}", indent, cfg));
		}
		let sig = resolve_types_for_impl(&m.signature, &concrete);
		let trait_path = format!("crate::traits::{}", m.origin_trait);
		let body_args = format_call_args(m, true);
		out.push(format!("{}pub {} {{<Self as {}>::{}({})}}", indent, sig, trait_path, m.name, body_args));
	}
	out
}

fn render_trait_body(name: &str, indent: &str, traits: &[TraitDef]) -> Vec<String> {
	let td = traits.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no trait `{}`", name));
	let mut out = Vec::new();
	for super_name in &td.supertraits {
		let Some(parent) = traits.iter().find(|t| &t.name == super_name) else { continue };
		for m in &parent.methods {
			if let Some(cfg) = &m.cfg {
				out.push(format!("{}{}", indent, cfg));
			}
			let body_args = format_call_args(m, true);
			out.push(format!("{}{} {{ <Self as {}>::{}({}) }}", indent, m.signature, super_name, m.name, body_args));
		}
	}
	out
}

fn render_module(tag: &str, traits: &[TraitDef]) -> Vec<String> {
	let trait_name = format!("{}Module", tag);
	let td = traits
		.iter()
		.find(|t| t.name == trait_name)
		.unwrap_or_else(|| panic!("no trait `{}` for tag `{}`", trait_name, tag));
	let concrete = format!("crate::{}", tag);
	let trait_path = format!("crate::traits::{}", trait_name);

	let mut out = Vec::new();
	for m in &td.methods {
		if let Some(cfg) = &m.cfg {
			out.push(cfg.clone());
		}
		let sig = resolve_types_for_module(&m.signature, &concrete);
		let body_args = format_call_args(m, false);
		out.push(format!("pub {} {{<{} as {}>::{}({})}}", sig, concrete, trait_path, m.name, body_args));
	}
	out
}

fn format_call_args(m: &Method, include_self: bool) -> String {
	let mut parts: Vec<String> = Vec::new();
	if include_self && m.has_self {
		parts.push("self".to_string());
	}
	parts.extend(m.args.iter().cloned());
	parts.join(", ")
}
