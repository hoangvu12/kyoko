use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use mockcord::Fixtures;
use parking_lot::Mutex;
use protocol::{ChannelType, Message, NotificationLevel, Snowflake};
use session::{Alert, AlertKind, AlertPacing, Command, SessionHooks};
use state::State;

use crate::harness::{Options, Running};

const QUIET: &str = "keep this one quiet";
const SETTLE: Duration = Duration::from_millis(300);

fn roomy(name: &'static str) -> Options {
	Options::new(name).alerts(AlertPacing {
		burst: 100,
		window: Duration::from_secs(1),
	})
}

struct Scene {
	me: Snowflake,
	guild_id: Snowflake,
	channel_id: Snowflake,
	author: u64,
	dm_id: Snowflake,
	dm_author: u64,
}

struct Seen {
	alerts: Mutex<Vec<Alert>>,
}

impl Seen {
	fn new() -> Self {
		Self {
			alerts: Mutex::new(Vec::new()),
		}
	}

	fn collect(&self, running: &Running) -> usize {
		let mut alerts = self.alerts.lock();
		alerts.extend(running.session().take_alerts());
		alerts.len()
	}

	fn contents(&self) -> Vec<(AlertKind, String)> {
		self.alerts
			.lock()
			.iter()
			.map(|alert| (alert.kind, alert.message.content.clone()))
			.collect()
	}
}

struct Veto;

impl SessionHooks for Veto {
	fn name(&self) -> &str {
		"test.veto"
	}

	fn allow_notification(&self, message: &Message) -> Result<bool> {
		Ok(message.content != QUIET)
	}
}

fn blocked(state: &State, user_id: u64) -> bool {
	state
		.blocked()
		.iter()
		.any(|row| row.user_id == Snowflake(user_id))
}

fn plain_channel(state: &State, guild_id: Snowflake) -> Option<Snowflake> {
	let overridden: Vec<Snowflake> = state
		.guild_settings(Some(guild_id))
		.map(|settings| {
			settings
				.channel_overrides
				.iter()
				.map(|over| over.channel_id)
				.collect()
		})
		.unwrap_or_default();
	state.guild(guild_id)?.channels.values().find_map(|channel| {
		(channel.kind == ChannelType::GuildText
			&& !overridden.contains(&channel.id)
			&& channel
				.parent_id
				.is_none_or(|parent| !overridden.contains(&parent)))
		.then_some(channel.id)
	})
}

async fn scene(running: &Running) -> Scene {
	running.connected().await;
	let scene = locate(&running.session().state().read(), running.fixtures());
	scene
		.guild_level(running, NotificationLevel::OnlyMentions, false)
		.await;
	scene
}

fn locate(state: &State, fixtures: &Fixtures) -> Scene {
	let me = state.current_user_id().expect("signed in");
	let (guild_id, channel_id, author) = state
		.guilds()
		.find_map(|guild| {
			let channel_id = plain_channel(state, guild.id)?;
			let author = fixtures
				.someone_in(channel_id.0)
				.map(|user| user.id.0)
				.filter(|author| *author != me.0 && !blocked(state, *author))?;
			Some((guild.id, channel_id, author))
		})
		.expect("a plain text channel with another member");
	let dm_overrides: Vec<Snowflake> = state
		.guild_settings(None)
		.map(|settings| {
			settings
				.channel_overrides
				.iter()
				.map(|over| over.channel_id)
				.collect()
		})
		.unwrap_or_default();
	let (dm_id, dm_author) = state
		.private_channels()
		.filter(|channel| channel.kind == ChannelType::Dm && !dm_overrides.contains(&channel.id))
		.find_map(|channel| {
			let author = channel.recipient_ids.first()?.0;
			(!blocked(state, author) && fixtures.users.contains_key(&author)).then_some((channel.id, author))
		})
		.expect("a direct message with someone not blocked");
	Scene {
		me,
		guild_id,
		channel_id,
		author,
		dm_id,
		dm_author,
	}
}

impl Scene {
	fn mention(&self, text: &str) -> String {
		format!("<@{}> {text}", self.me.0)
	}

	async fn guild_level(&self, running: &Running, level: NotificationLevel, suppress_everyone: bool) {
		running.session().send(Command::SetGuildNotifications {
			guild_id: Some(self.guild_id),
			level,
			suppress_everyone,
			suppress_roles: false,
			mobile_push: true,
		});
		let guild_id = self.guild_id;
		running
			.until("the guild notification settings to apply", move |state| {
				state.guild_settings(Some(guild_id)).is_some_and(|settings| {
					settings.message_notifications == Some(level)
						&& settings.suppress_everyone == suppress_everyone
				})
			})
			.await;
	}

	async fn post(&self, running: &Running, content: &str) {
		self.post_in(running, self.channel_id, self.author, content).await;
	}

	async fn post_dm(&self, running: &Running, content: &str) {
		self.post_in(running, self.dm_id, self.dm_author, content).await;
	}

	async fn post_in(&self, running: &Running, channel_id: Snowflake, author: u64, content: &str) {
		let id = running
			.mock()
			.post_message(channel_id.0, author, content)
			.expect("the mock posts the message");
		running
			.until("the posted message to arrive", move |state| {
				state
					.channel(channel_id)
					.is_some_and(|channel| channel.last_message_id >= Some(Snowflake(id)))
			})
			.await;
	}

	async fn only(&self, running: &Running, seen: &Seen) -> Vec<(AlertKind, String)> {
		let sentinel = format!("sentinel {}", seen.collect(running));
		self.post_dm(running, &sentinel).await;
		running
			.eventually("the sentinel alert", || {
				seen.collect(running);
				seen.contents().iter().any(|(_, content)| *content == sentinel)
			})
			.await;
		let mut contents = seen.contents();
		contents.retain(|(_, content)| !content.starts_with("sentinel"));
		seen.alerts.lock().clear();
		contents
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_messages_and_mentions_raise_alerts() {
	let running = Running::start_with(roomy("alerts-basic")).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	scene.post(&running, "nobody is mentioned here").await;
	scene.post(&running, &scene.mention("look at this")).await;
	scene.post_dm(&running, "a direct hello").await;
	assert_eq!(
		scene.only(&running, &seen).await,
		vec![
			(AlertKind::Mention, scene.mention("look at this")),
			(AlertKind::DirectMessage, "a direct hello".to_owned()),
		]
	);
	scene
		.guild_level(&running, NotificationLevel::AllMessages, false)
		.await;
	scene.post(&running, "everything counts now").await;
	assert_eq!(
		scene.only(&running, &seen).await,
		vec![(AlertKind::Message, "everything counts now".to_owned())]
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_muted_guild_stays_silent_even_for_mentions() {
	let running = Running::start_with(roomy("alerts-muted")).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	running.session().send(Command::MuteGuild {
		guild_id: scene.guild_id,
		until: None,
	});
	let guild_id = scene.guild_id;
	running
		.until("the guild to be muted", move |state| {
			state
				.guild_settings(Some(guild_id))
				.is_some_and(|settings| settings.muted)
		})
		.await;
	scene.post(&running, &scene.mention("are you there")).await;
	assert_eq!(scene.only(&running, &seen).await, Vec::new());
	running.session().send(Command::MuteChannel {
		channel_id: scene.dm_id,
		guild_id: None,
		until: None,
	});
	let dm_id = scene.dm_id;
	running
		.until("the direct message to be muted", move |state| {
			state.guild_settings(None).is_some_and(|settings| {
				settings
					.channel_overrides
					.iter()
					.any(|over| over.channel_id == dm_id && over.muted)
			})
		})
		.await;
	scene.post_dm(&running, "muted hello").await;
	tokio::time::sleep(SETTLE).await;
	seen.collect(&running);
	assert_eq!(seen.contents(), Vec::new());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn everyone_mentions_follow_the_suppress_setting() {
	let running = Running::start_with(roomy("alerts-everyone")).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	scene.post(&running, "@everyone first call").await;
	assert_eq!(
		scene.only(&running, &seen).await,
		vec![(AlertKind::Mention, "@everyone first call".to_owned())]
	);
	scene
		.guild_level(&running, NotificationLevel::OnlyMentions, true)
		.await;
	scene.post(&running, "@everyone second call").await;
	assert_eq!(scene.only(&running, &seen).await, Vec::new());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_channel_in_view_stays_silent_until_the_window_loses_focus() {
	let running = Running::start_with(roomy("alerts-viewing")).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	running.session().send(Command::SelectChannel {
		guild_id: Some(scene.guild_id),
		channel_id: scene.channel_id,
	});
	let channel_id = scene.channel_id;
	running
		.until("the channel to be selected", move |state| {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == channel_id)
		})
		.await;
	scene.post(&running, &scene.mention("while you watch")).await;
	assert_eq!(scene.only(&running, &seen).await, Vec::new());
	running.session().send(Command::SetViewing(false));
	scene.post(&running, &scene.mention("while you are away")).await;
	assert_eq!(
		scene.only(&running, &seen).await,
		vec![(AlertKind::Mention, scene.mention("while you are away"))]
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_plugin_hook_can_veto_an_alert() {
	let running = Running::start_with(roomy("alerts-veto").hooks(Arc::new(Veto))).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	scene.post_dm(&running, QUIET).await;
	scene.post_dm(&running, "this one may ring").await;
	assert_eq!(
		scene.only(&running, &seen).await,
		vec![(AlertKind::DirectMessage, "this one may ring".to_owned())]
	);
	running.stop().await;
}

struct SlowVeto {
	asked: AtomicUsize,
}

impl SessionHooks for SlowVeto {
	fn name(&self) -> &str {
		"test.slow-veto"
	}

	fn allow_notification(&self, message: &Message) -> Result<bool> {
		self.asked.fetch_add(1, Ordering::SeqCst);
		std::thread::sleep(Duration::from_millis(100));
		Ok(message.content != QUIET)
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_hook_hears_only_alerts_the_burst_limit_would_show() {
	let hook = Arc::new(SlowVeto {
		asked: AtomicUsize::new(0),
	});
	let running = Running::start_with(Options::new("alerts-reserve").hooks(hook.clone())).await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	for _ in 0..3 {
		scene.post_dm(&running, QUIET).await;
	}
	tokio::time::sleep(SETTLE).await;
	for step in 0..5 {
		scene.post_dm(&running, &format!("ring {step}")).await;
	}
	tokio::time::sleep(SETTLE).await;
	seen.collect(&running);
	let mut shown = seen.contents();
	shown.sort_by(|left, right| left.1.cmp(&right.1));
	assert_eq!(
		shown,
		(0..3)
			.map(|step| (AlertKind::DirectMessage, format!("ring {step}")))
			.collect::<Vec<_>>(),
		"vetoed alerts give their place back"
	);
	assert_eq!(
		hook.asked.load(Ordering::SeqCst),
		6,
		"alerts past the burst limit never reach the hook"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_burst_of_messages_raises_only_a_few_alerts() {
	let running = Running::start("alerts-burst").await;
	let scene = scene(&running).await;
	let seen = Seen::new();
	for step in 0..8 {
		scene.post_dm(&running, &format!("burst {step}")).await;
	}
	tokio::time::sleep(SETTLE).await;
	assert_eq!(seen.collect(&running), 3);
	running.stop().await;
}
