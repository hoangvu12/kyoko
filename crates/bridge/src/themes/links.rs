use session::MAX_THEME_LINKS;

use super::palette::Scheme;

const LIGHT_PREFIX: &str = "@light";
const DARK_PREFIX: &str = "@dark";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLink {
	pub line: String,
	pub url: String,
	pub scheme: Scheme,
}

pub fn link_lines(text: &str) -> Vec<String> {
	let mut lines: Vec<String> = Vec::new();
	for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
		if !lines.iter().any(|seen| seen == line) {
			lines.push(line.to_owned());
		}
	}
	lines.truncate(MAX_THEME_LINKS);
	lines
}

pub fn theme_link(line: &str) -> ThemeLink {
	let line = line.trim();
	let (scheme, url) = match prefixed(line, LIGHT_PREFIX) {
		Some(url) => (Scheme::Light, url),
		None => match prefixed(line, DARK_PREFIX) {
			Some(url) => (Scheme::Dark, url),
			None => (Scheme::Any, line),
		},
	};
	ThemeLink {
		line: line.to_owned(),
		url: url.to_owned(),
		scheme,
	}
}

pub fn theme_links(lines: &[String]) -> Vec<ThemeLink> {
	lines.iter().map(|line| theme_link(line)).collect()
}

pub fn link_urls(lines: &[String]) -> Vec<String> {
	session::unique_links(
		&theme_links(lines)
			.into_iter()
			.map(|link| link.url)
			.collect::<Vec<_>>(),
	)
}

fn prefixed<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
	let rest = line.strip_prefix(prefix)?;
	rest.starts_with(char::is_whitespace).then(|| rest.trim())
}
