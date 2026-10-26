use protocol::Snowflake;
use state::State;

use super::command::{CommandApp, CommandEntry, CommandPicker, CommandSection, command_catalog};

pub const RECENT_TILES: usize = 7;

#[derive(Debug, Clone, PartialEq)]
pub struct LauncherApp {
	pub app: CommandApp,
	pub description: String,
}

impl LauncherApp {
	pub fn id(&self) -> Option<Snowflake> {
		self.app.id
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppsLauncher {
	pub recents: Vec<LauncherApp>,
	pub apps: Vec<LauncherApp>,
}

impl AppsLauncher {
	pub fn recent_tiles(&self) -> &[LauncherApp] {
		&self.recents[..self.recents.len().min(RECENT_TILES)]
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppSearch {
	pub query: String,
	pub apps: Vec<LauncherApp>,
	pub commands: Vec<CommandEntry>,
}

impl AppSearch {
	pub fn is_empty(&self) -> bool {
		self.apps.is_empty() && self.commands.is_empty()
	}
}

fn app_commands(state: &State, guild_id: Option<Snowflake>, channel_id: Snowflake) -> Vec<CommandEntry> {
	command_catalog(state, guild_id, channel_id)
		.into_iter()
		.filter(|entry| !entry.app.built_in())
		.collect()
}

fn launcher_apps(state: &State, channel_id: Snowflake, commands: &[CommandEntry]) -> Vec<LauncherApp> {
	let Some(index) = state.command_index(channel_id) else {
		return Vec::new();
	};
	let mut apps: Vec<LauncherApp> = Vec::new();
	for entry in commands {
		if apps.iter().any(|known| known.app == entry.app) {
			continue;
		}
		let description = entry
			.app
			.id
			.and_then(|id| index.application(id))
			.and_then(|application| application.description.clone())
			.unwrap_or_default();
		apps.push(LauncherApp {
			app: entry.app.clone(),
			description,
		});
	}
	apps
}

pub fn apps_launcher(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	now_ms: u64,
) -> AppsLauncher {
	let commands = app_commands(state, guild_id, channel_id);
	let apps = launcher_apps(state, channel_id, &commands);
	let mut recents: Vec<LauncherApp> = Vec::new();
	for used in state.frequent_commands(now_ms) {
		let Some(entry) = commands.iter().find(|entry| entry.key == used.key) else {
			continue;
		};
		if recents.iter().any(|known| known.app == entry.app) {
			continue;
		}
		if let Some(app) = apps.iter().find(|app| app.app == entry.app) {
			recents.push(app.clone());
		}
	}
	AppsLauncher { recents, apps }
}

fn rank(name: &str, query: &str) -> Option<u8> {
	let name = name.to_lowercase();
	if name.starts_with(query) {
		Some(0)
	} else if name.split_whitespace().any(|word| word.starts_with(query)) {
		Some(1)
	} else {
		name.contains(query).then_some(2)
	}
}

fn ranked<T>(items: impl IntoIterator<Item = T>, name: impl Fn(&T) -> String, query: &str) -> Vec<T> {
	let mut found: Vec<(u8, T)> = items
		.into_iter()
		.filter_map(|item| rank(&name(&item), query).map(|rank| (rank, item)))
		.collect();
	found.sort_by_key(|(rank, _)| *rank);
	found.into_iter().map(|(_, item)| item).collect()
}

pub fn search_apps(
	state: &State,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	query: &str,
) -> AppSearch {
	let query = query.trim().trim_start_matches('/').to_lowercase();
	if query.is_empty() {
		return AppSearch::default();
	}
	let commands = app_commands(state, guild_id, channel_id);
	let apps = launcher_apps(state, channel_id, &commands);
	AppSearch {
		apps: ranked(apps, |app| app.app.name.clone(), &query),
		commands: ranked(commands, CommandEntry::name, &query),
		query,
	}
}

impl CommandPicker {
	pub fn only_app(self, app_id: Snowflake) -> CommandPicker {
		let mut picker = CommandPicker {
			query: self.query,
			..CommandPicker::default()
		};
		for section in self.sections {
			if section.app.as_ref().and_then(|app| app.id) != Some(app_id) {
				continue;
			}
			let rows = &self.rows[section.first..section.first + section.count];
			picker.sections.push(CommandSection {
				first: picker.rows.len(),
				..section
			});
			picker.rows.extend_from_slice(rows);
		}
		picker
	}
}
