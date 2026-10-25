use std::path::Path;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct ScreenEntry {
	pub name: String,
	pub ticket: String,
	pub steps: String,
}

pub fn load(list: &Path) -> Result<Vec<ScreenEntry>> {
	let text = std::fs::read_to_string(list).with_context(|| format!("read {}", list.display()))?;
	let screens: Vec<ScreenEntry> = text.lines().filter_map(parse_row).collect();
	if screens.is_empty() {
		bail!("{} lists no screens", list.display());
	}
	Ok(screens)
}

pub fn select(listed: &[ScreenEntry], wanted: &[String]) -> Result<Vec<ScreenEntry>> {
	if wanted.is_empty() {
		return Ok(listed.to_vec());
	}
	wanted
		.iter()
		.map(|name| {
			listed
				.iter()
				.find(|screen| screen.name == *name)
				.cloned()
				.with_context(|| format!("{name} is not in the parity list"))
		})
		.collect()
}

fn parse_row(line: &str) -> Option<ScreenEntry> {
	let mut cells = line.trim().strip_prefix('|')?.split('|').map(str::trim);
	let name = cells.next()?.strip_prefix('`')?.strip_suffix('`')?;
	let ticket = cells.next()?;
	let steps = cells.next().unwrap_or_default();
	if name.is_empty() || !ticket.chars().all(|character| character.is_ascii_digit()) {
		return None;
	}
	Some(ScreenEntry {
		name: name.to_owned(),
		ticket: ticket.to_owned(),
		steps: steps.to_owned(),
	})
}
