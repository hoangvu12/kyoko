use bridge::{AppsLauncher, ChannelKind, Composer, apps_launcher, search_apps};
use chrono::Utc;
use mockcord::MockConfig;
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

const SERVER_APPS: [&str; 8] = [
	"Mockbot",
	"Remindy",
	"Polly",
	"Snapper",
	"Warden",
	"Lyra",
	"Ticketeer",
	"Countr",
];
const RECENT_APPS: [&str; 6] = ["Mockbot", "Remindy", "Polly", "Snapper", "Lyra", "Warden"];
const ACTIVITY_APP: &str = "Arcade";

struct Opened {
	running: Running,
	composer: Composer,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
}

async fn with_apps(name: &str) -> Opened {
	let config = MockConfig {
		more_apps: true,
		..MockConfig::default()
	};
	let mut running = Running::start_with(name, config).await;
	let guild_id = running.view.sidebar.selected();
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	if let Some(command) = running.open_channel(channel_id) {
		running.session().send(command);
	}
	running
		.until("the channel history", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until_state("the command index and the frequently used commands", |state| {
			state.command_index(channel_id).is_some() && state.command_uses().len() == RECENT_APPS.len()
		})
		.await;
	let mut composer = Composer::default();
	composer.open(&running.session().state().read(), guild_id, Some(channel_id));
	Opened {
		running,
		composer,
		guild_id,
		channel_id,
	}
}

impl Opened {
	fn launcher(&self) -> AppsLauncher {
		let now = u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default();
		apps_launcher(
			&self.running.session().state().read(),
			self.guild_id,
			self.channel_id,
			now,
		)
	}

	fn search(&self, query: &str) -> (Vec<String>, Vec<String>) {
		let found = search_apps(
			&self.running.session().state().read(),
			self.guild_id,
			self.channel_id,
			query,
		);
		(
			found.apps.iter().map(|app| app.app.name.clone()).collect(),
			found.commands.iter().map(|entry| entry.name()).collect(),
		)
	}

	fn app_id(&self, name: &str) -> Snowflake {
		self.launcher()
			.apps
			.iter()
			.find(|app| app.app.name == name)
			.and_then(|app| app.id())
			.expect("the app is listed")
	}

	fn sections(&self) -> Vec<(String, Vec<String>)> {
		let picker = self.composer.picker().expect("the command picker is open");
		picker
			.sections
			.iter()
			.map(|section| {
				let rows = picker.rows[section.first..section.first + section.count]
					.iter()
					.map(|row| row.name())
					.collect();
				(section.title.clone(), rows)
			})
			.collect()
	}

	fn echoed(&mut self, text: &str) {
		let state = self.running.session().state().clone();
		self.composer.restored(&state.read(), text, text.len());
	}

	fn typed(&mut self, text: &str) {
		let state = self.running.session().state().clone();
		self.composer.changed(&state.read(), text, text.len());
	}
}

fn names(apps: &[bridge::LauncherApp]) -> Vec<&str> {
	apps.iter().map(|app| app.app.name.as_str()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_launcher_lists_recent_apps_and_the_apps_in_this_server_but_no_activities() {
	let opened = with_apps("apps-launcher").await;
	let launcher = opened.launcher();
	assert_eq!(names(&launcher.apps), SERVER_APPS);
	assert_eq!(names(&launcher.recents), RECENT_APPS);
	assert_eq!(names(launcher.recent_tiles()), RECENT_APPS);
	let mockbot = &launcher.apps[0];
	assert_eq!(
		mockbot.description,
		"Test bot for kyoko's command and component states"
	);
	assert!(
		launcher.apps.iter().all(|app| !app.app.built_in()),
		"built-in commands are not an app"
	);
	assert!(
		!names(&launcher.apps).contains(&ACTIVITY_APP),
		"an app that only launches an activity is not offered"
	);
	let (apps, commands) = opened.search("arc");
	assert!(apps.is_empty() && commands.is_empty(), "{apps:?} {commands:?}");
	let (apps, commands) = opened.search("launch");
	assert!(
		apps.is_empty() && commands.is_empty(),
		"an activity's entry point is not offered: {apps:?} {commands:?}"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_finds_apps_and_their_commands() {
	let opened = with_apps("apps-search").await;
	let (apps, commands) = opened.search("re");
	assert_eq!(apps, ["Remindy"]);
	assert_eq!(
		commands,
		["remind", "resize", "ignore"],
		"commands starting with the query come first"
	);
	let (apps, commands) = opened.search("Snap");
	assert_eq!(apps, ["Snapper"]);
	assert_eq!(commands, ["snap"]);
	let (apps, commands) = opened.search("/play");
	assert!(apps.is_empty());
	assert_eq!(commands, ["play"], "a typed slash still finds the command");
	let (apps, commands) = opened.search("ly");
	assert_eq!(apps, ["Lyra", "Polly"], "name prefixes rank before inner matches");
	assert!(commands.is_empty());
	let (apps, commands) = opened.search("zzz");
	assert!(apps.is_empty() && commands.is_empty());
	let (apps, commands) = opened.search("  ");
	assert!(apps.is_empty() && commands.is_empty());
	let found = search_apps(
		&opened.running.session().state().read(),
		opened.guild_id,
		opened.channel_id,
		"warn",
	);
	let warn = &found.commands[0];
	assert_eq!(warn.app.name, "Warden");
	assert_eq!(
		warn.required()
			.map(|option| option.name.as_str())
			.collect::<Vec<_>>(),
		["reason"]
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn picking_an_app_opens_the_command_picker_filtered_to_it() {
	let mut opened = with_apps("apps-pick").await;
	let remindy = opened.app_id("Remindy");
	let replaced = opened.composer.open_app(remindy);
	assert_eq!(replaced, Some(("/".to_owned(), 1)));
	opened.echoed("/");
	assert_eq!(opened.composer.app_filter(), Some(remindy));
	assert_eq!(
		opened.sections(),
		[("Remindy".to_owned(), vec!["remind".to_owned()])]
	);

	let mockbot = opened.app_id("Mockbot");
	opened.composer.open_app(mockbot);
	opened.echoed("/");
	opened.typed("/r");
	let sections = opened.sections();
	assert_eq!(sections.len(), 1, "typing filters inside the picked app");
	assert_eq!(sections[0].0, "Mockbot");
	assert_eq!(sections[0].1[0], "roll");
	let chosen = opened.composer.choose("/r", None);
	assert_eq!(chosen, Some((String::new(), 0)));
	assert_eq!(
		opened.composer.command().map(|command| command.entry.name()),
		Some("roll".to_owned())
	);
	assert_eq!(opened.composer.app_filter(), None);

	opened.composer.cancel_command();
	opened.composer.open_app(mockbot);
	opened.echoed("/");
	assert!(opened.composer.dismiss());
	opened.typed("/");
	let titles: Vec<String> = opened.sections().into_iter().map(|(title, _)| title).collect();
	assert!(
		titles.iter().any(|title| title == "Remindy") && titles.iter().any(|title| title == "Built-In"),
		"after Escape the picker lists every app again: {titles:?}"
	);

	opened.typed("hello");
	let found = search_apps(
		&opened.running.session().state().read(),
		opened.guild_id,
		opened.channel_id,
		"remind",
	);
	assert!(opened.composer.pick_command(found.commands[0].clone()));
	assert_eq!(
		opened.composer.command().map(|command| command.entry.name()),
		Some("remind".to_owned()),
		"a command found by the launcher's search is picked at once"
	);
}
