use std::path::PathBuf;
use std::time::{Duration, Instant};

use bridge::{ChannelKind, MemberListEntry, MemberPanel, OpenMembers};
use chrono::{DateTime, Utc};
use mockcord::Recording;
use mockcord::fixtures::CURRENT_USER_ID;
use plugins::{HostConfig, Limits, Permission, PluginHost, Slot};
use protocol::{Opcode, Snowflake};
use serde_json::json;
use session::Command;

use crate::harness::Running;

const CHANNELS: &str = "show-hidden-channels";
const THINGS: &str = "show-hidden-things";
const QUIET: Duration = Duration::from_millis(800);
const WAIT: Duration = Duration::from_secs(15);

struct Plugins {
	host: PluginHost,
	root: PathBuf,
}

impl Plugins {
	fn start(name: &str) -> Plugins {
		let root = std::env::temp_dir().join(format!("kyoko-bridge-hidden-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&root);
		let host = PluginHost::new(HostConfig {
			data_dir: Some(root.clone()),
			user_dir: None,
			development_dirs: Vec::new(),
			bundled: true,
			limits: Limits {
				load_deadline: Duration::from_secs(15),
				..Limits::default()
			},
		});
		Plugins { host, root }
	}

	fn approve(&self, id: &str) {
		let declared: Vec<Permission> = self
			.host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == id)
			.map(|plugin| plugin.permissions)
			.expect("the plugin is bundled");
		self.host.approve(id, &declared).expect("the plugin starts");
	}

	async fn until_shows(&self, slot: Slot, shown: bool) {
		let deadline = Instant::now() + WAIT;
		while self.host.shows(slot) != shown {
			assert!(
				Instant::now() < deadline,
				"timed out waiting for {slot:?} to be {shown}"
			);
			tokio::time::sleep(Duration::from_millis(20)).await;
		}
	}

	fn stop(self) {
		let _ = std::fs::remove_dir_all(&self.root);
	}
}

fn hidden_in(running: &Running) -> (Snowflake, Snowflake) {
	let state = running.session().state().read();
	state
		.guilds()
		.find_map(|guild| {
			guild
				.channels
				.values()
				.find(|channel| state.is_hidden(channel.id))
				.map(|channel| (guild.id, channel.id))
		})
		.expect("a channel the user cannot view in the fixtures")
}

fn traffic_naming(recording: &Recording, channel_id: Snowflake) -> Vec<String> {
	let id = channel_id.to_string();
	let requests = recording
		.requests
		.iter()
		.filter(|request| {
			request.path.contains(&id)
				|| request.query.as_deref().is_some_and(|query| query.contains(&id))
				|| String::from_utf8_lossy(&request.body).contains(&id)
		})
		.map(|request| format!("{} {}", request.method, request.path));
	let frames = recording
		.frames
		.iter()
		.filter(|frame| frame.raw.contains(&id))
		.map(|frame| format!("op {}: {}", frame.op, frame.raw));
	requests.chain(frames).collect()
}

fn assert_silent(running: &Running, channel_id: Snowflake, when: &str) {
	let traffic = running
		.server
		.control()
		.inspect(|recording| traffic_naming(recording, channel_id));
	assert!(
		traffic.is_empty(),
		"nothing names the hidden channel {when}: {traffic:?}"
	);
}

async fn open_visible(running: &mut Running, guild_id: Snowflake) {
	if let Some(command) = running.select_guild(guild_id) {
		running.session().send(command);
	}
	let visible = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text && channel.selectable())
		.map(|channel| channel.id)
		.expect("a text channel the user can view");
	if let Some(command) = running.open_channel(visible) {
		running.session().send(command);
	}
	running
		.until("the visible channel to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
}

fn show_hidden(running: &mut Running, show: bool) -> bool {
	let state = running.session().state().clone();
	running.view.sidebar.set_show_hidden(&state.read(), show)
}

fn misdirected(guild_id: Snowflake, channel_id: Snowflake, last: Snowflake) -> Vec<Command> {
	vec![
		Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id,
		},
		Command::LoadOlder { channel_id },
		Command::LoadPins { channel_id },
		Command::Typing { channel_id },
		Command::AckChannel {
			channel_id,
			message_id: last,
		},
		Command::MarkChannelRead { channel_id },
		Command::SetMemberViewport {
			guild_id,
			channel_id,
			first: 0,
			last: 99,
		},
		Command::SubscribeGuild { guild_id },
	]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hidden_channels_show_with_their_details_and_never_reach_discord() {
	let plugins = Plugins::start("channels");
	let mut running = Running::start("hidden-channels").await;
	let (guild_id, hidden_id) = hidden_in(&running);
	open_visible(&mut running, guild_id).await;
	assert!(
		running.view.sidebar.channel(hidden_id).is_none(),
		"without the plugin a channel the user cannot view is not listed"
	);

	plugins.approve(CHANNELS);
	plugins.until_shows(Slot::HiddenChannels, true).await;
	assert!(show_hidden(&mut running, true));
	let entry = running
		.view
		.sidebar
		.channel(hidden_id)
		.cloned()
		.expect("the hidden channel is listed while the plugin runs");
	assert!(entry.locked && entry.opens_info() && !entry.selectable());
	assert_eq!(entry.name, "staff-only");

	let (last, info) = {
		let state = running.session().state().clone();
		let state = state.read();
		let last = state
			.channel(hidden_id)
			.and_then(|channel| channel.last_message_id)
			.expect("the hidden channel has a last message id");
		(last, running.view.sidebar.open_hidden(&state, hidden_id))
	};
	let info = info.expect("clicking the hidden channel opens its details");
	assert_eq!(running.view.sidebar.hidden_open(), Some(hidden_id));
	assert_eq!(info.topic.as_deref(), Some("talk about staff-only here"));
	assert_eq!(info.kind, "Text Channel");
	assert_eq!(info.last_message_at, Some(last.timestamp()));
	assert!(info.activity_text(Utc::now()).starts_with("Last message "));
	let names = |list: &[bridge::Overwriter]| list.iter().map(|entry| entry.name.clone()).collect::<Vec<_>>();
	assert_eq!(names(&info.allowed), ["Admin"]);
	assert_eq!(names(&info.denied), ["@everyone"]);

	for command in misdirected(guild_id, hidden_id, last) {
		running.session().send(command);
	}
	open_visible(&mut running, guild_id).await;
	tokio::time::sleep(QUIET).await;
	assert_silent(&running, hidden_id, "while it is listed and opened");

	running.restart().await;
	assert!(show_hidden(&mut running, true));
	open_visible(&mut running, guild_id).await;
	let reopened = {
		let state = running.session().state().clone();
		running.view.sidebar.open_hidden(&state.read(), hidden_id)
	};
	assert!(reopened.is_some(), "the details open again after a restart");
	for command in misdirected(guild_id, hidden_id, last) {
		running.session().send(command);
	}
	running.session().send(Command::MarkGuildRead { guild_id });
	tokio::time::sleep(QUIET).await;
	assert_silent(&running, hidden_id, "after a restart");

	plugins.host.disable(CHANNELS);
	plugins.until_shows(Slot::HiddenChannels, false).await;
	assert!(show_hidden(&mut running, false));
	assert!(running.view.sidebar.channel(hidden_id).is_none());
	assert_eq!(
		running.view.sidebar.hidden_open(),
		None,
		"turning the plugin off closes the details"
	);
	running.stop().await;
	plugins.stop();
}

fn entry_for(panel: &MemberPanel, user_id: Snowflake) -> Option<bridge::MemberEntry> {
	panel.entries().iter().find_map(|entry| match entry {
		MemberListEntry::Member(member) if member.user_id == user_id => Some(member.clone()),
		_ => None,
	})
}

fn wire_count(recording: &Recording) -> (usize, usize) {
	let frames = recording
		.frames
		.iter()
		.filter(|frame| frame.opcode() != Opcode::Heartbeat)
		.count();
	(recording.requests.len(), frames)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn moderator_notes_show_timeouts_already_in_state_and_fetch_nothing() {
	let plugins = Plugins::start("things");
	let mut running = Running::start("hidden-things").await;
	let open = {
		let state = running.session().state().clone();
		bridge::first_channel(&state.read()).expect("a channel to open")
	};
	let guild_id = open.guild_id.expect("a server channel");
	if let Some(command) = running.select_guild(guild_id) {
		running.session().send(command);
	}
	let command = running.open_channel(open.channel_id).expect("the channel opens");
	running.session().send(command);
	running.view.members.show(
		Some(OpenMembers {
			guild_id,
			channel_id: open.channel_id,
		}),
		Instant::now(),
	);
	running
		.until("the member list to load", |view| {
			view.members
				.members()
				.any(|member| member.user_id.get() != CURRENT_USER_ID)
		})
		.await;
	let target = running
		.view
		.members
		.members()
		.find(|member| member.user_id.get() != CURRENT_USER_ID)
		.map(|member| member.user_id)
		.expect("another member");
	let until: DateTime<Utc> = Utc::now() + chrono::Duration::days(2);
	let user = running
		.session()
		.state()
		.read()
		.user(target)
		.cloned()
		.expect("the member's user is known");
	running.server.control().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({
			"guild_id": guild_id.to_string(),
			"user": { "id": target.to_string(), "username": user.username },
			"roles": [],
			"pending": true,
			"communication_disabled_until": until.to_rfc3339(),
		}),
	);
	running
		.until_state("the timeout to reach state", move |state| {
			state
				.guild(guild_id)
				.and_then(|guild| guild.members.get(&target))
				.is_some_and(|member| member.communication_disabled_until.is_some())
		})
		.await;
	running.flush_members();
	let before = entry_for(&running.view.members, target).expect("the member is listed");
	assert!(
		before.notes.is_empty(),
		"without the plugin no moderator notes show"
	);

	let quiet = running.server.control().inspect(wire_count);
	plugins.approve(THINGS);
	plugins.until_shows(Slot::ModeratorNotes, true).await;
	assert!(running.view.members.set_moderator_notes(true, Instant::now()));
	running
		.until("the member list to show the notes", move |view| {
			entry_for(&view.members, target).is_some_and(|member| !member.notes.is_empty())
		})
		.await;
	let shown = entry_for(&running.view.members, target).expect("the member is listed");
	assert_eq!(
		shown.notes.timed_out_until.map(|at| at.timestamp()),
		Some(until.timestamp())
	);
	assert!(shown.notes.pending);
	assert!(
		shown
			.notes
			.timeout_text(Utc::now())
			.is_some_and(|text| text.starts_with("Timed out until "))
	);
	assert_eq!(
		shown.notes.flag_text().as_deref(),
		Some("Pending membership screening")
	);
	tokio::time::sleep(QUIET).await;
	assert_eq!(
		running.server.control().inspect(wire_count),
		quiet,
		"showing the notes sends nothing to Discord"
	);
	running.stop().await;
	plugins.stop();
}
