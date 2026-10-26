use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{ChannelKind, Display, MemberListEntry, OpenMembers, ProfileCard, Row};
use chrono::{TimeDelta, Utc};
use plugins::{HostConfig, Limits, Permission, PluginHost};
use protocol::Snowflake;
use serde_json::json;
use session::{Formatting, Person};

use crate::harness::Running;

const SHOW_ME_YOUR_NAME: &str = "show-me-your-name";
const DONT_ROUND: &str = "dont-round-my-timestamps";
const WAIT: Duration = Duration::from_secs(15);

struct Plugins {
	host: PluginHost,
	display: Display,
	root: PathBuf,
}

impl Plugins {
	fn start(name: &str, enabled: &[&str]) -> Plugins {
		Self::start_with(name, enabled, Limits::default(), |_| {})
	}

	fn start_with(
		name: &str,
		enabled: &[&str],
		limits: Limits,
		prepare: impl FnOnce(&std::path::Path),
	) -> Plugins {
		let root = std::env::temp_dir().join(format!("kyoko-bridge-display-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&root);
		prepare(&root.join("dev"));
		let host = PluginHost::new(HostConfig {
			data_dir: Some(root.clone()),
			user_dir: None,
			development_dirs: vec![root.join("dev")],
			bundled: true,
			limits: Limits {
				load_deadline: Duration::from_secs(15),
				..limits
			},
		});
		for id in enabled {
			let declared: Vec<Permission> = host
				.snapshot()
				.into_iter()
				.find(|plugin| plugin.id == *id)
				.map(|plugin| plugin.permissions)
				.expect("the bundled plugin is listed");
			host.approve(id, &declared).expect("the plugin starts");
		}
		let display = Display::new(Arc::new(host.clone()), |_| {});
		host.on_change(display.refresher());
		Plugins { host, display, root }
	}

	fn stop(self) {
		drop(self.display);
		let _ = std::fs::remove_dir_all(&self.root);
	}
}

async fn eventually(what: &str, mut ready: impl FnMut() -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready() {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		tokio::time::sleep(Duration::from_millis(20)).await;
	}
}

async fn open_first_channel(running: &mut Running) -> (Snowflake, Snowflake) {
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	let guild_id = running.view.sidebar.selected().expect("a selected guild");
	let command = running.open_channel(channel_id).expect("the channel opens");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	(guild_id, channel_id)
}

fn author_person(running: &Running, guild_id: Snowflake, author: Snowflake) -> Person {
	let state = running.session().state().read();
	let user = state.user(author).expect("the author is known").as_ref().clone();
	bridge::person(&state, Some(guild_id), &user)
}

fn redraw(running: &mut Running, formatting: Formatting) {
	let state = running.session().state().clone();
	running.view.timeline.redraw(&state.read(), formatting);
}

fn expected(person: &Person, mode: &str) -> (String, Option<String>) {
	let name = person.name().to_owned();
	if mode == "user" {
		return (person.username.clone(), None);
	}
	if name.to_lowercase() == person.username.to_lowercase() {
		return (name, None);
	}
	(name, Some(person.username.clone()))
}

fn timeline_matches(running: &Running, guild_id: Snowflake, mode: &str) -> bool {
	running
		.view
		.timeline
		.messages()
		.filter(|entry| entry.is_plain())
		.all(|entry| {
			let person = author_person(running, guild_id, entry.message.author.id);
			let (name, extra) = expected(&person, mode);
			entry.author == name && entry.author_extra == extra
		})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn show_me_your_name_reaches_the_timeline_member_list_and_profiles() {
	let mut running = Running::start("display-names").await;
	let plugins = Plugins::start("names", &[SHOW_ME_YOUR_NAME]);
	running.view.timeline.set_display(plugins.display.clone());
	running.view.members.set_display(plugins.display.clone());
	let (guild_id, channel_id) = open_first_channel(&mut running).await;

	eventually("the timeline to show usernames", || {
		redraw(&mut running, Formatting::Names);
		timeline_matches(&running, guild_id, "nick-user")
	})
	.await;
	let suffixed = running
		.view
		.timeline
		.messages()
		.filter(|entry| entry.author_extra.is_some())
		.count();
	assert!(suffixed > 0, "some author shows a username beside the name");
	let authors: std::collections::HashSet<Snowflake> = running
		.view
		.timeline
		.messages()
		.filter(|entry| entry.is_plain())
		.map(|entry| entry.message.author.id)
		.collect();
	let mut calls = plugins.display.calls();
	for _ in 0..20 {
		tokio::time::sleep(Duration::from_millis(300)).await;
		redraw(&mut running, Formatting::Names);
		tokio::time::sleep(Duration::from_millis(100)).await;
		let settled = plugins.display.calls();
		if settled == calls {
			break;
		}
		calls = settled;
	}
	assert!(
		calls <= 2 * authors.len() as u64,
		"about one call per author: {calls} calls for {} authors",
		authors.len()
	);
	for _ in 0..50 {
		redraw(&mut running, Formatting::Names);
	}
	assert_eq!(plugins.display.calls(), calls, "redrawing asks no plugin again");

	running.view.members.show(
		Some(OpenMembers { guild_id, channel_id }),
		std::time::Instant::now(),
	);
	running
		.until("the member list to load", |view| {
			view.members.members().next().is_some()
		})
		.await;
	eventually("the member list to show usernames", || {
		running.view.members.redraw(std::time::Instant::now());
		running.flush_members();
		let members: Vec<_> = running.view.members.members().cloned().collect();
		!members.is_empty()
			&& members.iter().all(|member| {
				let (name, extra) = expected(&author_person(&running, guild_id, member.user_id), "nick-user");
				member.name == name && member.extra == extra
			})
	})
	.await;
	assert!(
		running
			.view
			.members
			.entries()
			.iter()
			.any(|entry| matches!(entry, MemberListEntry::Member(member) if member.extra.is_some()))
	);

	running.view.members.reset();
	running.view.members.show(
		Some(OpenMembers { guild_id, channel_id }),
		std::time::Instant::now(),
	);
	eventually("the member list to keep usernames after a reset", || {
		running.flush_members();
		running
			.view
			.members
			.members()
			.any(|member| member.extra.is_some())
	})
	.await;

	let profiled = running
		.view
		.timeline
		.messages()
		.find(|entry| entry.author_extra.is_some())
		.map(|entry| entry.message.author.id)
		.expect("an author with a username suffix");
	eventually("the profile to show the username", || {
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, profiled, Some(guild_id), Utc::now())
			.expect("a profile card")
			.shown_with(&state, &plugins.display);
		let person = bridge::person(&state, Some(guild_id), &card.user);
		let (name, extra) = expected(&person, "nick-user");
		card.name == name && card.name_extra == extra
	})
	.await;

	running.view.timeline.take_delta();
	plugins
		.host
		.set_setting(SHOW_ME_YOUR_NAME, "mode", &json!("user"))
		.expect("mode is a setting");
	eventually("the timeline to follow the new mode", || {
		redraw(&mut running, Formatting::Names);
		timeline_matches(&running, guild_id, "user")
	})
	.await;
	assert!(
		running
			.view
			.timeline
			.take_delta()
			.is_some_and(|delta| !delta.updated.is_empty())
	);

	plugins.host.disable(SHOW_ME_YOUR_NAME);
	redraw(&mut running, Formatting::Names);
	let state = running.session().state().clone();
	let own: Vec<(String, Option<String>)> = running
		.view
		.timeline
		.messages()
		.map(|entry| (entry.author.clone(), entry.author_extra.clone()))
		.collect();
	let shown: Vec<(String, Option<String>)> = {
		let state = state.read();
		running
			.view
			.timeline
			.messages()
			.map(|entry| (state.display_name(Some(guild_id), entry.message.author.id), None))
			.collect()
	};
	assert_eq!(own, shown, "turning the plugin off shows kyoko's names at once");
	plugins.stop();
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dont_round_my_timestamps_rounds_down_and_redraws_the_rows_that_show_one() {
	let mut running = Running::start("display-times").await;
	let plugins = Plugins::start("times", &[DONT_ROUND]);
	running.view.timeline.set_display(plugins.display.clone());
	let (_, channel_id) = open_first_channel(&mut running).await;
	let now = Utc::now();
	let unix = (now - TimeDelta::hours(2 * 24 + 23)).timestamp();
	assert_eq!(Display::default().relative(unix, now), "3 days ago");
	let first = plugins.display.relative(unix, now);
	assert!(first == "3 days ago" || first == "2 days ago", "{first}");
	eventually("the plugin's text", || {
		plugins.display.relative(unix, now) == "2 days ago"
	})
	.await;

	let posted = running
		.server
		.post_message(channel_id, &format!("due <t:{unix}:R>"))
		.expect("mockcord posts the message");
	running
		.until("the timestamped message", |view| {
			view.timeline.messages().any(|entry| entry.id == posted)
		})
		.await;
	running.view.timeline.take_delta();
	redraw(&mut running, Formatting::RelativeTimes);
	let delta = running.view.timeline.take_delta().expect("rows to redraw");
	let timed: Vec<_> = running
		.view
		.timeline
		.rows()
		.iter()
		.filter(|row| match &row.row {
			Row::Message(entry) => entry.content.contains(":R>"),
			_ => false,
		})
		.map(|row| row.key.clone())
		.collect();
	assert!(!timed.is_empty());
	assert!(timed.iter().all(|key| delta.updated.contains(key)), "{delta:?}");
	assert_eq!(
		delta.updated.len(),
		timed.len(),
		"only rows with a relative time redraw"
	);

	plugins.host.disable(DONT_ROUND);
	assert_eq!(plugins.display.relative(unix, now), "3 days ago");
	plugins.stop();
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stuck_name_plugin_never_holds_up_a_row_and_falls_back() {
	let mut running = Running::start("display-stuck").await;
	let limits = Limits {
		name_deadline: Duration::from_millis(200),
		..Limits::default()
	};
	let plugins = Plugins::start_with("stuck", &["stuck-names"], limits, |dev| {
		let dir = dev.join("stuck-names");
		std::fs::create_dir_all(&dir).expect("plugin dir");
		std::fs::write(
			dir.join("plugin.toml"),
			"id = \"stuck-names\"
api = 1
permissions = [\"hook:display_name\"]
",
		)
		.expect("manifest");
		std::fs::write(
			dir.join("logic.luau"),
			"return { format_name = function(person) while true do end end }",
		)
		.expect("logic");
	});
	assert!(plugins.display.formats(Formatting::Names));
	running.view.timeline.set_display(plugins.display.clone());
	let (guild_id, _) = open_first_channel(&mut running).await;
	let started = Instant::now();
	for _ in 0..20 {
		redraw(&mut running, Formatting::Names);
	}
	assert!(
		started.elapsed() < Duration::from_millis(150),
		"{:?}",
		started.elapsed()
	);
	let state = running.session().state().clone();
	let own = |running: &Running| {
		let state = state.read();
		running.view.timeline.messages().all(|entry| {
			entry.author_extra.is_none()
				&& entry.author == state.display_name(Some(guild_id), entry.message.author.id)
		})
	};
	assert!(own(&running), "kyoko's own names show while the plugin is stuck");
	eventually("the stuck plugin to be suspended", || {
		!plugins.display.formats(Formatting::Names)
	})
	.await;
	redraw(&mut running, Formatting::Names);
	assert!(own(&running));
	plugins.stop();
	running.stop().await;
}
