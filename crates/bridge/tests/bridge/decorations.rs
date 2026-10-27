use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{ChannelKind, Display, MemberEntry, OpenMembers, ProfileCard};
use chrono::Utc;
use plugins::{HostConfig, Limits, Permission, PluginHost};
use protocol::Snowflake;
use session::{Badge, Formatting};
use state::State;

use crate::harness::Running;

const PLATFORMS: &str = "platform-indicators";
const CROWN: &str = "force-owner-crown";
const WAIT: Duration = Duration::from_secs(15);

struct Plugins {
	host: PluginHost,
	display: Display,
	root: PathBuf,
}

impl Plugins {
	fn start(name: &str) -> Plugins {
		let root =
			std::env::temp_dir().join(format!("kyoko-bridge-decorations-{name}-{}", std::process::id()));
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
		let display = Display::new(Arc::new(host.clone()), |_| {});
		host.on_change(display.refresher());
		Plugins { host, display, root }
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

fn platforms_of(state: &State, user_id: Snowflake) -> Vec<String> {
	let Some(user) = state.user(user_id) else {
		return Vec::new();
	};
	if user.bot == Some(true) {
		return Vec::new();
	}
	let Some(client) = state
		.presence(user_id)
		.and_then(|presence| presence.client_status.as_ref())
	else {
		return Vec::new();
	};
	[
		("monitor", client.desktop.is_some()),
		("smartphone", client.mobile.is_some()),
		("globe", client.web.is_some()),
	]
	.into_iter()
	.filter(|(_, shown)| *shown)
	.map(|(key, _)| key.to_owned())
	.collect()
}

fn keys(badges: &[Badge]) -> Vec<String> {
	badges.iter().map(|badge| badge.key.clone()).collect()
}

fn redraw(running: &mut Running) {
	let state = running.session().state().clone();
	running
		.view
		.timeline
		.redraw(&state.read(), Formatting::Decorations);
	running.view.members.redraw(std::time::Instant::now());
	running.flush_members();
}

fn members(running: &Running) -> Vec<MemberEntry> {
	running.view.members.members().cloned().collect()
}

fn owner(running: &Running, guild_id: Snowflake) -> Snowflake {
	let state = running.session().state().read();
	state
		.guild(guild_id)
		.and_then(|guild| guild.owner_id)
		.expect("the guild has an owner")
}

fn all_decorated(running: &Running, guild_id: Snowflake) -> bool {
	let state = running.session().state().read();
	let owner = owner(running, guild_id);
	let timeline = running
		.view
		.timeline
		.messages()
		.filter(|entry| entry.is_plain())
		.all(|entry| keys(&entry.decorations) == platforms_of(&state, entry.message.author.id));
	let listed = members(running);
	let list = !listed.is_empty()
		&& listed.iter().all(|member| {
			keys(&member.decorations) == platforms_of(&state, member.user_id)
				&& member.crown == (member.user_id == owner)
		});
	timeline
		&& list
		&& running
			.view
			.members
			.members()
			.any(|member| member.user_id == owner)
}

async fn open(running: &mut Running) -> Snowflake {
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
	running.view.members.show(
		Some(OpenMembers { guild_id, channel_id }),
		std::time::Instant::now(),
	);
	running
		.until("the member list to load", |view| {
			view.members.members().next().is_some()
		})
		.await;
	guild_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn platform_indicators_and_the_owner_crown_reach_the_right_people_once() {
	let mut running = Running::start("decorations").await;
	let plugins = Plugins::start("decorations");
	running.view.timeline.set_display(plugins.display.clone());
	running.view.members.set_display(plugins.display.clone());
	let guild_id = open(&mut running).await;
	let owner = owner(&running, guild_id);

	redraw(&mut running);
	assert!(
		running
			.view
			.timeline
			.messages()
			.all(|entry| entry.decorations.is_empty()),
		"nothing beside names while no plugin runs"
	);
	assert!(
		members(&running)
			.iter()
			.all(|member| member.decorations.is_empty() && !member.crown),
		"mockcord's guild shows its admins apart, so Discord's rule hides the crown"
	);

	plugins.approve(PLATFORMS);
	plugins.approve(CROWN);
	eventually(
		"every author and member to show their platforms and the owner a crown",
		|| {
			redraw(&mut running);
			all_decorated(&running, guild_id) && plugins.display.pending() == 0
		},
	)
	.await;
	let decorated = members(&running)
		.iter()
		.filter(|member| !member.decorations.is_empty())
		.count();
	assert!(decorated > 0, "some members are online on some platform");
	{
		let state = running.session().state().read();
		let kinds: std::collections::HashSet<String> = members(&running)
			.iter()
			.flat_map(|member| platforms_of(&state, member.user_id))
			.collect();
		assert!(
			kinds.len() >= 2,
			"mockcord spreads people over platforms: {kinds:?}"
		);
	}

	let mut calls = plugins.display.decoration_calls();
	for _ in 0..20 {
		tokio::time::sleep(Duration::from_millis(200)).await;
		redraw(&mut running);
		let settled = plugins.display.decoration_calls();
		if settled == calls && plugins.display.pending() == 0 {
			break;
		}
		calls = settled;
	}
	let people: std::collections::HashSet<Snowflake> = running
		.view
		.timeline
		.messages()
		.map(|entry| entry.message.author.id)
		.chain(members(&running).iter().map(|member| member.user_id))
		.collect();
	assert!(
		calls <= 2 * people.len() as u64,
		"about one call per person: {calls} calls for {} people",
		people.len()
	);
	for _ in 0..50 {
		redraw(&mut running);
	}
	assert_eq!(
		plugins.display.decoration_calls(),
		calls,
		"redrawing asks no plugin again"
	);

	let profiled = members(&running)
		.into_iter()
		.find(|member| !member.decorations.is_empty())
		.expect("a decorated member");
	eventually("the profile to show the platforms first among its badges", || {
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, profiled.user_id, Some(guild_id), Utc::now())
			.expect("a profile card")
			.shown_with(&state, &plugins.display);
		keys(&card.decorations) == platforms_of(&state, profiled.user_id)
	})
	.await;

	plugins.host.disable(PLATFORMS);
	plugins.host.disable(CROWN);
	redraw(&mut running);
	assert!(
		running
			.view
			.timeline
			.messages()
			.all(|entry| entry.decorations.is_empty())
	);
	eventually(
		"the member list to drop the platforms and the owner's crown",
		|| {
			redraw(&mut running);
			members(&running)
				.iter()
				.all(|member| member.decorations.is_empty() && !member.crown)
		},
	)
	.await;
	assert!(
		members(&running).iter().any(|member| member.user_id == owner),
		"the owner is still listed, without a crown"
	);
	running.stop().await;
	plugins.stop();
}
