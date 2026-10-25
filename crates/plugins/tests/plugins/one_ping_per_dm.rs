use std::time::Duration;

use parking_lot::Mutex;
use plugins::{EnableError, Limits, Permission, Status};
use protocol::{ChannelType, NotificationLevel, Snowflake};
use serde_json::json;
use session::{AlertPacing, Command};
use state::State;

use crate::harness::Options;
use crate::support::{Lab, approve, scratch, start_with, write_plugin};

const ID: &str = "one-ping-per-dm";
const SETTLE: Duration = Duration::from_millis(400);

struct Chat {
	channel_id: Snowflake,
	author: u64,
}

struct Scene {
	me: Snowflake,
	guild_id: Snowflake,
	server: Chat,
	direct: Chat,
	group: Chat,
}

struct Pings<'a> {
	lab: &'a Lab,
	seen: Mutex<Vec<String>>,
}

fn roomy(name: &'static str) -> Options {
	Options::new(name).alerts(AlertPacing {
		burst: 100,
		window: Duration::from_secs(1),
	})
}

async fn lab(name: &'static str) -> Lab {
	start_with(roomy(name), scratch(name), Limits::default(), true).await
}

fn blocked(state: &State, user_id: u64) -> bool {
	state
		.blocked()
		.iter()
		.any(|row| row.user_id == Snowflake(user_id))
}

fn private_chat(lab: &Lab, state: &State, kind: ChannelType) -> Chat {
	let overridden: Vec<Snowflake> = state
		.guild_settings(None)
		.map(|settings| {
			settings
				.channel_overrides
				.iter()
				.map(|over| over.channel_id)
				.collect()
		})
		.unwrap_or_default();
	state
		.private_channels()
		.filter(|channel| channel.kind == kind && !overridden.contains(&channel.id))
		.find_map(|channel| {
			let author = channel.recipient_ids.iter().map(|id| id.0).find(|author| {
				!blocked(state, *author) && lab.running.fixtures().users.contains_key(author)
			})?;
			Some(Chat {
				channel_id: channel.id,
				author,
			})
		})
		.expect("a private conversation with someone not blocked")
}

fn server_chat(lab: &Lab, state: &State, me: Snowflake) -> (Snowflake, Chat) {
	state
		.guilds()
		.find_map(|guild| {
			let overridden: Vec<Snowflake> = state
				.guild_settings(Some(guild.id))
				.map(|settings| {
					settings
						.channel_overrides
						.iter()
						.map(|over| over.channel_id)
						.collect()
				})
				.unwrap_or_default();
			guild.channels.values().find_map(|channel| {
				let plain = channel.kind == ChannelType::GuildText
					&& !overridden.contains(&channel.id)
					&& channel
						.parent_id
						.is_none_or(|parent| !overridden.contains(&parent));
				if !plain {
					return None;
				}
				let author = lab
					.running
					.fixtures()
					.someone_in(channel.id.0)
					.map(|user| user.id.0)
					.filter(|author| *author != me.0 && !blocked(state, *author))?;
				Some((
					guild.id,
					Chat {
						channel_id: channel.id,
						author,
					},
				))
			})
		})
		.expect("a plain text channel with another member")
}

async fn scene(lab: &Lab) -> Scene {
	lab.running.connected().await;
	let scene = {
		let state = lab.running.session().state().read();
		let me = state.current_user_id().expect("signed in");
		let (guild_id, server) = server_chat(lab, &state, me);
		Scene {
			me,
			guild_id,
			server,
			direct: private_chat(lab, &state, ChannelType::Dm),
			group: private_chat(lab, &state, ChannelType::GroupDm),
		}
	};
	lab.running.session().send(Command::SetGuildNotifications {
		guild_id: Some(scene.guild_id),
		level: NotificationLevel::OnlyMentions,
		suppress_everyone: false,
		suppress_roles: false,
		mobile_push: true,
	});
	let guild_id = scene.guild_id;
	lab.running
		.until("the guild to notify for mentions", move |state| {
			state.guild_settings(Some(guild_id)).is_some_and(|settings| {
				settings.message_notifications == Some(NotificationLevel::OnlyMentions)
			})
		})
		.await;
	scene
}

fn enable(lab: &Lab) {
	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let Err(EnableError::NeedsApproval(asked)) = lab.host.enable(ID) else {
		panic!("enabling asks for approval first");
	};
	let mut names: Vec<String> = asked.iter().map(Permission::to_string).collect();
	names.sort();
	assert_eq!(names, ["hook:notify", "read:dms", "read:read_state"]);
	lab.host.approve(ID, &asked).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));
}

impl Scene {
	fn mention(&self, text: &str) -> String {
		format!("<@{}> {text}", self.me.0)
	}
}

impl<'a> Pings<'a> {
	fn new(lab: &'a Lab) -> Self {
		Self {
			lab,
			seen: Mutex::new(Vec::new()),
		}
	}

	fn collect(&self) {
		let alerts = self.lab.running.session().take_alerts();
		self.seen
			.lock()
			.extend(alerts.into_iter().map(|alert| alert.message.content.clone()));
	}

	async fn post(&self, chat: &Chat, content: &str) -> Snowflake {
		let channel_id = chat.channel_id;
		let id = Snowflake(
			self.lab
				.running
				.mock()
				.post_message(channel_id.0, chat.author, content)
				.expect("the mock posts the message"),
		);
		self.lab
			.running
			.until("the posted message to arrive", move |state| {
				state
					.channel(channel_id)
					.is_some_and(|channel| channel.last_message_id >= Some(id))
			})
			.await;
		id
	}

	async fn rings(&self, chat: &Chat, content: &str) -> Snowflake {
		let id = self.post(chat, content).await;
		self.lab
			.running
			.eventually(&format!("a notification for {content:?}"), || {
				self.collect();
				self.seen.lock().iter().any(|seen| seen == content)
			})
			.await;
		tokio::time::sleep(SETTLE).await;
		self.collect();
		assert_eq!(
			std::mem::take(&mut *self.seen.lock()),
			vec![content.to_owned()],
			"exactly one notification"
		);
		id
	}

	async fn quiet(&self, chat: &Chat, content: &str) -> Snowflake {
		let id = self.post(chat, content).await;
		tokio::time::sleep(SETTLE).await;
		self.collect();
		assert_eq!(
			std::mem::take(&mut *self.seen.lock()),
			Vec::<String>::new(),
			"{content:?} stays silent"
		);
		id
	}

	async fn read(&self, chat: &Chat, message_id: Snowflake) {
		let channel_id = chat.channel_id;
		self.lab.running.session().send(Command::AckChannel {
			channel_id,
			message_id,
		});
		self.read_up_to(channel_id, message_id).await;
	}

	async fn read_up_to(&self, channel_id: Snowflake, message_id: Snowflake) {
		self.lab
			.running
			.until("the conversation to be read", move |state| {
				state
					.read_state(channel_id)
					.is_some_and(|read| read.last_message_id >= Some(message_id))
			})
			.await;
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_direct_messages_ping_once_until_read() {
	let lab = lab("one-ping").await;
	let scene = scene(&lab).await;
	let pings = Pings::new(&lab);
	enable(&lab);

	pings.rings(&scene.direct, "first hello").await;
	pings.quiet(&scene.direct, "second hello").await;
	let last = pings.quiet(&scene.direct, "third hello").await;
	{
		let state = lab.running.session().state().read();
		let read = state.read_state(scene.direct.channel_id);
		assert!(
			read.is_none_or(|read| read.last_message_id < Some(last)),
			"the silenced messages stay unread"
		);
	}

	pings.read(&scene.direct, last).await;
	pings.rings(&scene.direct, "after reading").await;
	let last = pings.quiet(&scene.direct, "still unread").await;

	lab.running.session().send(Command::SelectChannel {
		guild_id: None,
		channel_id: scene.direct.channel_id,
	});
	pings.read_up_to(scene.direct.channel_id, last).await;
	lab.running.session().send(Command::SetViewing(false));
	pings.rings(&scene.direct, "after viewing").await;
	pings.quiet(&scene.direct, "while away").await;

	pings.rings(&scene.server, &scene.mention("first mention")).await;
	pings.rings(&scene.server, &scene.mention("second mention")).await;

	lab.host.disable(ID);
	pings.rings(&scene.direct, "plugin off").await;
	pings.rings(&scene.direct, "plugin still off").await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_settings_choose_which_conversations_ping_once() {
	let lab = lab("one-ping-settings").await;
	let scene = scene(&lab).await;
	let pings = Pings::new(&lab);
	enable(&lab);

	lab.host
		.set_setting(ID, "channels", &json!("group"))
		.expect("a listed option");
	pings.rings(&scene.direct, "direct one").await;
	pings.rings(&scene.direct, "direct two").await;
	pings.rings(&scene.group, "group one").await;
	pings.quiet(&scene.group, "group two").await;

	lab.host
		.set_setting(ID, "channels", &json!("direct"))
		.expect("a listed option");
	pings.rings(&scene.group, "group three").await;
	pings.rings(&scene.group, "group four").await;
	pings.rings(&scene.direct, "direct three").await;
	pings.quiet(&scene.direct, "direct four").await;

	lab.host
		.set_setting(ID, "servers", &json!(true))
		.expect("a boolean");
	pings.rings(&scene.server, &scene.mention("server one")).await;
	let last = pings.quiet(&scene.server, &scene.mention("server two")).await;
	pings.read(&scene.server, last).await;
	pings.rings(&scene.server, &scene.mention("server three")).await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_notification_filter_lets_the_notification_through() {
	let name = "notify-fails-open";
	let root = scratch(name);
	write_plugin(
		&root,
		"broken-filter",
		&["hook:notify"],
		"",
		"return { allow_notification = function(message) error(\"broken\") end }",
	);
	write_plugin(
		&root,
		"slow-filter",
		&["hook:notify"],
		"",
		"return { allow_notification = function(message) while true do end end }",
	);
	let lab = start_with(roomy(name), root, Limits::default(), false).await;
	let scene = scene(&lab).await;
	let pings = Pings::new(&lab);
	approve(&lab.host, "broken-filter");
	approve(&lab.host, "slow-filter");
	pings.rings(&scene.direct, "despite a broken filter").await;
	pings.rings(&scene.direct, "and again").await;
	lab.stop().await;
}
