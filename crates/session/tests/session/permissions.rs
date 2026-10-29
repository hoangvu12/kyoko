use std::time::Duration;

use mockcord::Recording;
use protocol::{ChannelType, PermissionOverwrite, PermissionOverwriteKind, Permissions, Snowflake};
use session::{Command, RestTuning, UploadFile};
use state::State;

use crate::harness::{Options, Running};

const GUILDS: usize = 3;
const ADMIN_GUILD: usize = 0;
const MOD_GUILD: usize = 1;
const MUTED_GUILD: usize = 2;
const QUIET: Duration = Duration::from_millis(400);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

fn rejected_with(state: &State, before: u64, words: &str) -> bool {
	state
		.last_rejection()
		.is_some_and(|rejection| rejection.id > before && rejection.reason.contains(words))
}

fn count(recording: &Recording, method: &str, pattern: &str) -> usize {
	recording.requests_to(method, pattern).count()
}

fn text_channel(running: &Running, guild: usize) -> Snowflake {
	let fixtures = running.fixtures();
	fixtures.guilds[guild]
		.message_channel_ids
		.iter()
		.copied()
		.find(|id| {
			fixtures.channels.get(id).is_some_and(|channel| {
				channel.kind == ChannelType::GuildText && channel.name.as_deref() != Some("staff-only")
			})
		})
		.map(Snowflake)
		.expect("a text channel")
}

async fn start(name: &'static str) -> Running {
	let running = Running::start_with(Options::new(name).guilds(GUILDS).rest(tuning())).await;
	running.connected().await;
	running
}

#[tokio::test(flavor = "multi_thread")]
async fn ready_members_with_only_a_user_id_give_my_own_roles_and_permissions() {
	let running = start("permissions-ready-members").await;
	let me = running.fixtures().current_user.id;
	{
		let state = running.session().state().read();
		check_my_roles(&running, &state, me);
	}
	running.stop().await;
}

fn check_my_roles(running: &Running, state: &State, me: Snowflake) {
	for guild in &running.fixtures().guilds {
		let guild_id = Snowflake(guild.id);
		let roles = state
			.guild(guild_id)
			.and_then(|known| known.members.get(&me))
			.map(|member| member.roles.clone());
		assert_eq!(
			roles.as_ref(),
			Some(&guild.current_member().roles),
			"my roles in guild {guild_id} come from READY"
		);
	}
	let admin = Snowflake(running.fixtures().guilds[ADMIN_GUILD].id);
	assert_eq!(state.guild_permissions(admin, me), Permissions::all());
	let moderated = text_channel(running, MOD_GUILD);
	assert!(state.can_manage_messages(moderated), "the Mods role applies");
	let muted = text_channel(running, MUTED_GUILD);
	assert!(!state.can_send(muted), "the Muted role's overwrite applies");
	assert!(!state.can_react(muted));
}

#[tokio::test(flavor = "multi_thread")]
async fn sending_and_typing_without_send_messages_never_reach_rest() {
	let running = start("permissions-send").await;
	let channel_id = text_channel(&running, MUTED_GUILD);
	let before = rejections(&running);
	running.session().send(Command::Typing { channel_id });
	running.session().send(Command::SendMessage {
		channel_id,
		content: "hello".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	running
		.until("the message to be refused", |state| {
			rejected_with(state, before, "permission to send messages")
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	let messages = format!("/channels/{channel_id}/messages");
	let typing = format!("/channels/{channel_id}/typing");
	assert_eq!(count(&recording, "POST", &messages), 0, "no message request");
	assert_eq!(count(&recording, "POST", &typing), 0, "no typing request");
	assert!(
		running
			.session()
			.state()
			.read()
			.messages(channel_id)
			.is_none_or(|cache| cache.ordered.iter().all(|message| message.content != "hello")),
		"no pending message is shown"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn files_without_attach_files_never_reach_rest() {
	let running = start("permissions-attach").await;
	let channel_id = text_channel(&running, MOD_GUILD);
	let guild_id = Snowflake(running.fixtures().guilds[MOD_GUILD].id);
	let mut channel = running.fixtures().channels[&channel_id.get()].clone();
	let mut overwrites: Vec<PermissionOverwrite> = channel.permission_overwrites.iter().cloned().collect();
	overwrites.push(PermissionOverwrite {
		id: guild_id,
		kind: PermissionOverwriteKind::Role,
		allow: Permissions::empty(),
		deny: Permissions::ATTACH_FILES,
	});
	channel.permission_overwrites = protocol::LenientVec::from(overwrites);
	running.mock().dispatch(
		"CHANNEL_UPDATE",
		&serde_json::to_value(&channel).expect("channel json"),
	);
	running
		.until("Attach Files to be denied", |state| {
			state.can_send(channel_id) && !state.can_attach(channel_id)
		})
		.await;
	let before = rejections(&running);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "a file".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: vec![UploadFile::new("notes.txt", b"x".to_vec(), "text/plain")],
	});
	running
		.until("the upload to be refused", |state| {
			rejected_with(state, before, "Attach Files")
		})
		.await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: "words only".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	let messages = format!("/channels/{channel_id}/messages");
	running
		.until_wire("the text-only message", |recording| {
			recording
				.requests_to("POST", &messages)
				.find(|request| String::from_utf8_lossy(&request.body).contains("words only"))
				.map(drop)
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(
		count(&recording, "POST", &messages),
		1,
		"only the text reaches REST"
	);
	running.stop().await;
}
