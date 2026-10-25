use std::collections::HashMap;

use super::color::parse_color;
use super::palette::{Layer, Scheme};

const MAX_DEPTH: usize = 16;
const MAX_FIELD_LENGTH: usize = 200;

pub const CSS_MAPPING: &[(&str, &[&str], &[&str])] = &[
	(
		"background_primary",
		&["--background-primary", "--background-base-low"],
		&["--bg-4"],
	),
	(
		"background_secondary",
		&["--background-secondary", "--background-base-lower"],
		&["--bg-4"],
	),
	(
		"background_tertiary",
		&["--background-tertiary", "--background-base-lowest"],
		&["--bg-3"],
	),
	(
		"background_floating",
		&["--background-floating", "--background-surface-highest"],
		&["--bg-3"],
	),
	(
		"user_panel_background",
		&["--background-secondary-alt"],
		&["--bg-4"],
	),
	("modifier_hover", &["--background-modifier-hover"], &["--hover"]),
	(
		"modifier_selected",
		&["--background-modifier-selected"],
		&["--active"],
	),
	(
		"modifier_active",
		&["--background-modifier-active"],
		&["--active-2"],
	),
	(
		"message_hover",
		&["--background-message-hover"],
		&["--message-hover"],
	),
	("text_normal", &["--text-normal", "--text-default"], &["--text-3"]),
	("text_muted", &["--text-muted"], &["--text-4"]),
	(
		"header_primary",
		&["--header-primary", "--text-strong"],
		&["--text-2"],
	),
	(
		"header_secondary",
		&["--header-secondary", "--text-subtle"],
		&["--text-4"],
	),
	("interactive_normal", &["--interactive-normal"], &["--text-4"]),
	("interactive_hover", &["--interactive-hover"], &["--text-3"]),
	("interactive_active", &["--interactive-active"], &["--text-1"]),
	("interactive_muted", &["--interactive-muted"], &["--text-5"]),
	("text_on_brand", &["--white-500"], &["--text-0"]),
	(
		"brand",
		&["--brand-experiment", "--brand-500", "--brand-experiment-500"],
		&["--accent-3"],
	),
	(
		"brand_hover",
		&["--brand-experiment-560", "--brand-560"],
		&["--accent-4"],
	),
	("link", &["--text-link"], &["--accent-1"]),
	("mention_background", &["--mention-background"], &[]),
	("mention_text", &["--mention-foreground"], &["--accent-1"]),
	("unread_pill", &[], &["--accent-2"]),
	("mention_badge", &[], &["--accent-new"]),
	(
		"status_online",
		&["--status-online", "--status-positive"],
		&["--online"],
	),
	("status_idle", &["--status-idle", "--status-warning"], &["--idle"]),
	("status_dnd", &["--status-dnd", "--status-danger"], &["--dnd"]),
	("status_offline", &["--status-offline"], &["--offline"]),
	("status_streaming", &[], &["--streaming"]),
	("danger", &["--status-danger", "--text-danger"], &["--red-3"]),
	(
		"danger_hover",
		&["--button-danger-background-hover"],
		&["--red-4"],
	),
	(
		"success",
		&["--status-positive", "--text-positive"],
		&["--green-2"],
	),
	(
		"warning",
		&["--status-warning", "--text-warning"],
		&["--yellow-2"],
	),
	(
		"scrollbar_thumb",
		&["--scrollbar-auto-thumb", "--scrollbar-thin-thumb"],
		&["--bg-1"],
	),
	("scrollbar_track", &["--scrollbar-auto-track"], &[]),
	("divider", &["--background-modifier-accent"], &["--active"]),
	(
		"input_background",
		&["--input-background", "--channeltextarea-background"],
		&["--bg-3"],
	),
	("code_background", &[], &["--bg-3"]),
	(
		"button_secondary",
		&["--button-secondary-background"],
		&["--bg-2"],
	),
	(
		"button_secondary_hover",
		&["--button-secondary-background-hover"],
		&["--bg-1"],
	),
	("spoiler", &["--spoiler-hidden-background"], &[]),
	("focus_ring", &["--focus-primary"], &[]),
];

const GLOBAL_SELECTORS: &[&str] = &[":root", "html", "body", "#app-mount", "*"];
const GLOBAL_PREFIXES: &[&str] = &[".theme-", ".platform-", ".visual-refresh", ".app-focused"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeMeta {
	pub name: String,
	pub author: String,
	pub description: String,
	pub version: String,
	pub source: String,
	pub website: String,
}

pub fn parse_meta(text: &str, fallback_name: &str) -> ThemeMeta {
	let mut meta = ThemeMeta::default();
	if let Some(start) = text.find("/**") {
		let rest = &text[start + 3..];
		let body = rest.find("*/").map_or(rest, |end| &rest[..end]);
		for line in body.lines() {
			let line = line.trim().trim_start_matches('*').trim();
			let Some(tagged) = line.strip_prefix('@') else {
				continue;
			};
			let (tag, value) = tagged.split_once(char::is_whitespace).unwrap_or((tagged, ""));
			let value = clean(value);
			let slot = match tag {
				"name" => &mut meta.name,
				"author" => &mut meta.author,
				"description" => &mut meta.description,
				"version" => &mut meta.version,
				"source" => &mut meta.source,
				"website" => &mut meta.website,
				_ => continue,
			};
			if slot.is_empty() {
				*slot = value;
			}
		}
	}
	if meta.name.is_empty() {
		meta.name = clean(fallback_name);
	}
	meta
}

fn clean(value: &str) -> String {
	value
		.chars()
		.filter(|character| !character.is_control())
		.take(MAX_FIELD_LENGTH)
		.collect::<String>()
		.trim()
		.to_owned()
}

struct Declaration {
	name: String,
	value: String,
	scheme: Scheme,
}

pub fn import_css(text: &str) -> Layer {
	let declarations = declarations(text);
	let any = environment(&declarations, None);
	let light = environment(&declarations, Some(Scheme::Light));
	let dark = environment(&declarations, Some(Scheme::Dark));
	let mut layer = Layer::default();
	for (token, discord, midnight) in CSS_MAPPING {
		let candidates: Vec<&str> = discord.iter().chain(midnight.iter()).copied().collect();
		let shared = pick(&any, &candidates);
		if let Some(value) = &shared {
			layer.set_color(token, value.clone(), Scheme::Any);
		}
		for (scheme, environment) in [(Scheme::Light, &light), (Scheme::Dark, &dark)] {
			if let Some(value) = pick(environment, &candidates)
				&& Some(&value) != shared.as_ref()
			{
				layer.set_color(token, value, scheme);
			}
		}
	}
	layer
}

fn pick(environment: &HashMap<&str, &str>, candidates: &[&str]) -> Option<String> {
	candidates.iter().find_map(|name| {
		let value = environment.get(name)?;
		let resolved = resolve(value, environment, 0)?;
		parse_color(&resolved).map(|color| color.hex())
	})
}

fn environment(declarations: &[Declaration], scheme: Option<Scheme>) -> HashMap<&str, &str> {
	let mut environment = HashMap::new();
	for declaration in declarations
		.iter()
		.filter(|declaration| declaration.scheme == Scheme::Any)
	{
		environment.insert(declaration.name.as_str(), declaration.value.as_str());
	}
	if let Some(scheme) = scheme {
		for declaration in declarations
			.iter()
			.filter(|declaration| declaration.scheme == scheme)
		{
			environment.insert(declaration.name.as_str(), declaration.value.as_str());
		}
	}
	environment
}

fn resolve(value: &str, environment: &HashMap<&str, &str>, depth: usize) -> Option<String> {
	if depth > MAX_DEPTH {
		return None;
	}
	let mut output = String::new();
	let mut rest = value;
	while let Some(start) = rest.find("var(") {
		output.push_str(&rest[..start]);
		let inner_start = start + 4;
		let end = closing(&rest[inner_start..])? + inner_start;
		let inner = &rest[inner_start..end];
		let (name, fallback) = match top_level_comma(inner) {
			Some(comma) => (inner[..comma].trim(), Some(inner[comma + 1..].trim())),
			None => (inner.trim(), None),
		};
		let replaced = match environment.get(name) {
			Some(found) => resolve(found, environment, depth + 1),
			None => None,
		}
		.or_else(|| fallback.and_then(|fallback| resolve(fallback, environment, depth + 1)))?;
		output.push_str(&replaced);
		rest = &rest[end + 1..];
	}
	output.push_str(rest);
	Some(output.trim().to_owned())
}

fn closing(text: &str) -> Option<usize> {
	let mut depth = 0usize;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' if depth == 0 => return Some(index),
			')' => depth -= 1,
			_ => {}
		}
	}
	None
}

fn top_level_comma(text: &str) -> Option<usize> {
	let mut depth = 0usize;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' => depth = depth.saturating_sub(1),
			',' if depth == 0 => return Some(index),
			_ => {}
		}
	}
	None
}

fn declarations(text: &str) -> Vec<Declaration> {
	let mut found = Vec::new();
	let mut stack: Vec<String> = Vec::new();
	let mut buffer = String::new();
	let mut depth = 0usize;
	let mut quote: Option<char> = None;
	let mut characters = text.chars().peekable();
	while let Some(character) = characters.next() {
		if let Some(open) = quote {
			buffer.push(character);
			if character == '\\' {
				if let Some(escaped) = characters.next() {
					buffer.push(escaped);
				}
			} else if character == open {
				quote = None;
			}
			continue;
		}
		match character {
			'/' if characters.peek() == Some(&'*') => {
				characters.next();
				let mut previous = '\0';
				for inside in characters.by_ref() {
					if previous == '*' && inside == '/' {
						break;
					}
					previous = inside;
				}
			}
			'"' | '\'' => {
				quote = Some(character);
				buffer.push(character);
			}
			'(' => {
				depth += 1;
				buffer.push(character);
			}
			')' => {
				depth = depth.saturating_sub(1);
				buffer.push(character);
			}
			'{' if depth == 0 => {
				stack.push(buffer.trim().to_owned());
				buffer.clear();
			}
			'}' if depth == 0 => {
				take(&buffer, &stack, &mut found);
				buffer.clear();
				stack.pop();
			}
			';' if depth == 0 => {
				take(&buffer, &stack, &mut found);
				buffer.clear();
			}
			_ => buffer.push(character),
		}
	}
	found
}

fn take(buffer: &str, stack: &[String], found: &mut Vec<Declaration>) {
	let Some((name, value)) = buffer.split_once(':') else {
		return;
	};
	let name = name.trim();
	if !name.starts_with("--") || name.len() <= 2 || stack.is_empty() {
		return;
	}
	let Some(scheme) = scope(stack) else {
		return;
	};
	let value = value.trim();
	let value = value
		.strip_suffix("!important")
		.map_or(value, str::trim_end)
		.to_owned();
	if value.is_empty() {
		return;
	}
	found.push(Declaration {
		name: name.to_owned(),
		value,
		scheme,
	});
}

fn scope(stack: &[String]) -> Option<Scheme> {
	let mut scheme = Scheme::Any;
	for selector in stack {
		let lower = selector.to_ascii_lowercase();
		if lower.starts_with('@') {
			if lower.contains("prefers-color-scheme") {
				scheme = if lower.contains("light") {
					Scheme::Light
				} else {
					Scheme::Dark
				};
			}
			continue;
		}
		if !is_global(&lower) {
			return None;
		}
		let light = lower.contains(".theme-light");
		let dark = lower.contains(".theme-dark") || lower.contains(".theme-midnight");
		match (light, dark) {
			(true, false) => scheme = Scheme::Light,
			(false, true) => scheme = Scheme::Dark,
			_ => {}
		}
	}
	Some(scheme)
}

fn is_global(selector: &str) -> bool {
	selector.split(',').all(|part| {
		let part = part.trim();
		!part.is_empty()
			&& part
				.split(|character: char| character.is_whitespace() || character == '>')
				.filter(|compound| !compound.is_empty())
				.all(is_global_compound)
	})
}

fn is_global_compound(compound: &str) -> bool {
	let compound = compound
		.trim_start_matches(":is(")
		.trim_start_matches(":where(")
		.trim_end_matches(')');
	let mut pieces = Vec::new();
	let mut start = 0;
	for (index, character) in compound.char_indices().skip(1) {
		if matches!(character, '.' | '#' | ':') && !compound[..index].ends_with(':') {
			pieces.push(&compound[start..index]);
			start = index;
		}
	}
	pieces.push(&compound[start..]);
	pieces.iter().all(|piece| {
		let piece = piece.trim_end_matches(')');
		piece.is_empty()
			|| GLOBAL_SELECTORS.contains(&piece)
			|| GLOBAL_PREFIXES.iter().any(|prefix| {
				piece.starts_with(prefix)
					&& piece[prefix.len()..].chars().all(|character| {
						character.is_ascii_alphanumeric() || character == '-' || character == '_'
					})
			})
	})
}
