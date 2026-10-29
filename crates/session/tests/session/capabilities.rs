use std::sync::Arc;

use anyhow::Result;
use gateway::ADVERTISED_CAPABILITIES;
use mockcord::{Control, Fixtures, GatewayFrame};
use parking_lot::Mutex;
use protocol::capability::{
	AUTH_TOKEN_REFRESH, AUTO_CALL_CONNECT, AUTO_LOBBY_CONNECT, CHANNEL_OBFUSCATION, CLIENT_STATE_V2,
	DEBOUNCE_MESSAGE_REACTIONS, DEDUPE_USER_OBJECTS, LAZY_USER_NOTES, MULTIPLE_GUILD_EXPERIMENT_POPULATIONS,
	NO_AFFINE_USER_IDS, NON_CHANNEL_READ_STATES, PASSIVE_GUILD_UPDATE, PASSIVE_GUILD_UPDATE_V2,
	PRIORITIZED_READY_PAYLOAD, USER_SETTINGS_PROTO, VERSIONED_READ_STATES, VERSIONED_USER_GUILD_SETTINGS,
	has,
};
use protocol::{DispatchEvent, Opcode, Snowflake, StatusKind};
use serde_json::json;
use session::{Command, SessionHooks};
use state::State;

use crate::harness::{Options, Running};

const HANDLED: [u64; 9] = [
	LAZY_USER_NOTES,
	VERSIONED_READ_STATES,
	VERSIONED_USER_GUILD_SETTINGS,
	DEDUPE_USER_OBJECTS,
	PRIORITIZED_READY_PAYLOAD,
	MULTIPLE_GUILD_EXPERIMENT_POPULATIONS,
	NON_CHANNEL_READ_STATES,
	AUTH_TOKEN_REFRESH,
	AUTO_CALL_CONNECT,
];
const UNHANDLED: [u64; 8] = [
	NO_AFFINE_USER_IDS,
	USER_SETTINGS_PROTO,
	CLIENT_STATE_V2,
	PASSIVE_GUILD_UPDATE,
	DEBOUNCE_MESSAGE_REACTIONS,
	PASSIVE_GUILD_UPDATE_V2,
	CHANNEL_OBFUSCATION,
	AUTO_LOBBY_CONNECT,
];
const REFRESHED_TOKEN: &str = "mock-token-refreshed";
const RESUMABLE_CLOSE: u16 = 4000;
const REACTION: &str = "🧪";

fn refresh_token_on_ready(mock: &Control) {
	mock.transform_ready(|_, ready| ready["auth_token"] = json!(REFRESHED_TOKEN));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identify_advertises_only_handled_capabilities() {
	let running = Running::start("capabilities-identify").await;
	let capabilities = running
		.until_wire("the first IDENTIFY", |recording| {
			recording
				.frames_with(Opcode::Identify)
				.next()
				.map(|frame| frame.data["capabilities"].as_u64())
		})
		.await
		.expect("IDENTIFY carries a capabilities number");
	assert_eq!(capabilities, ADVERTISED_CAPABILITIES);
	for capability in HANDLED {
		assert!(
			has(capabilities, capability),
			"bit {capability:#x} is handled and advertised"
		);
	}
	for capability in UNHANDLED {
		assert!(
			!has(capabilities, capability),
			"bit {capability:#x} is not handled"
		);
	}
	running.stop().await;
}

fn assert_history_uses_refreshed_token(running: &Running, channel_id: Snowflake) {
	let history = format!("/channels/{channel_id}/messages");
	let history_tokens: Vec<bool> = running
		.mock()
		.recording()
		.requests_to("GET", &history)
		.map(|request| request.header("authorization") == Some(REFRESHED_TOKEN))
		.collect();
	assert!(!history_tokens.is_empty());
	assert!(
		history_tokens.iter().all(|refreshed| *refreshed),
		"history requests after READY carry the refreshed token"
	);
}

async fn resume_with_refreshed_token(running: &Running, channel_id: Snowflake) -> GatewayFrame {
	let first = running.identified_connection().await;
	running.mock().close_gateway(RESUMABLE_CLOSE);
	let resume = running
		.until_wire("a RESUME on a new connection", |recording| {
			recording
				.frames_with(Opcode::Resume)
				.find(|frame| frame.connection != first)
				.cloned()
		})
		.await;
	assert!(
		resume.data["token"].as_str() == Some(REFRESHED_TOKEN),
		"RESUME carries the refreshed token"
	);
	running
		.send_and_wait(channel_id, "after refresh", "after refresh")
		.await;
	resume
}

fn current_user_id(running: &Running) -> Snowflake {
	running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("a current user after READY")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refreshed_token_reaches_rest_the_gateway_and_the_store() {
	let running =
		Running::start_with(Options::new("capabilities-refresh").prepare(refresh_token_on_ready)).await;
	let channel_id = running.open_text_channel().await;
	assert_history_uses_refreshed_token(&running, channel_id);

	let user_id = current_user_id(&running);
	let store = running.store();
	running
		.eventually("the refreshed token to be stored", || {
			let stored = store.tokens.load().ok().flatten();
			let account = store.accounts.token_for(user_id).ok().flatten();
			stored.as_deref() == Some(REFRESHED_TOKEN) && account.as_deref() == Some(REFRESHED_TOKEN)
		})
		.await;

	let resume = resume_with_refreshed_token(&running, channel_id).await;
	running.mock().invalidate_session(false);
	let identify = running
		.until_wire("an IDENTIFY after the session ended", |recording| {
			recording
				.frames_with(Opcode::Identify)
				.find(|frame| frame.connection == resume.connection)
				.cloned()
		})
		.await;
	assert!(
		identify.data["token"].as_str() == Some(REFRESHED_TOKEN),
		"a later IDENTIFY carries the refreshed token"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unremembered_session_keeps_a_refreshed_token_in_memory() {
	let running = Running::start_with(
		Options::new("capabilities-refresh-memory")
			.remember_account(false)
			.prepare(refresh_token_on_ready),
	)
	.await;
	let channel_id = running.open_text_channel().await;
	assert_history_uses_refreshed_token(&running, channel_id);
	resume_with_refreshed_token(&running, channel_id).await;
	let store = running.store();
	assert!(store.tokens.load().ok().flatten().is_none());
	assert!(store.accounts.list().is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signing_out_after_a_refresh_leaves_no_token_behind() {
	let running =
		Running::start_with(Options::new("capabilities-refresh-sign-out").prepare(refresh_token_on_ready))
			.await;
	running.connected().await;
	let user_id = current_user_id(&running);
	running.session().send(Command::Logout);
	let store = running.store();
	running
		.eventually("sign-out to remove the account and its tokens", || {
			store.accounts.list().is_empty()
				&& store.tokens.load().ok().flatten().is_none()
				&& store.accounts.token_for(user_id).ok().flatten().is_none()
		})
		.await;
	running.stop().await;
}

fn assert_versioned_containers(state: &State, fixtures: &Fixtures) {
	for read_state in &fixtures.read_states {
		let stored = state.read_state(read_state.id).expect("fixture read state");
		assert_eq!(stored.last_message_id, read_state.last_message_id);
		assert_eq!(stored.mention_count, read_state.mention_count);
	}
	assert!(!fixtures.user_guild_settings.is_empty());
	for settings in &fixtures.user_guild_settings {
		let stored = state
			.guild_settings(settings.guild_id)
			.expect("fixture guild settings");
		assert_eq!(stored.muted, settings.muted);
		assert_eq!(stored.message_notifications, settings.message_notifications);
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn versioned_containers_fill_read_states_and_guild_settings() {
	let running = Running::start("capabilities-versioned").await;
	running.connected().await;
	assert_versioned_containers(&running.session().state().read(), running.fixtures());
	running.stop().await;
}

fn assert_only_channel_read_states(state: &State, fixtures: &Fixtures) {
	for guild in &fixtures.guilds {
		assert!(state.read_state(Snowflake(guild.id)).is_none());
	}
	assert!(!fixtures.read_states.is_empty());
	for read_state in &fixtures.read_states {
		assert!(state.read_state(read_state.id).is_some());
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_channel_read_states_stay_out_of_channel_unreads() {
	let running = Running::start("capabilities-non-channel").await;
	running.connected().await;
	assert_only_channel_read_states(&running.session().state().read(), running.fixtures());
	running.stop().await;
}

fn assert_users_resolved(state: &State, fixtures: &Fixtures) {
	let mut checked = 0;
	for channel_id in &fixtures.private_channel_ids {
		let channel = state
			.channel(Snowflake(*channel_id))
			.expect("fixture private channel");
		for recipient in &channel.recipient_ids {
			let user = state.user(*recipient).expect("recipient resolved from users");
			assert_eq!(
				Some(&user.username),
				fixtures.users.get(&recipient.0).map(|user| &user.username)
			);
			checked += 1;
		}
	}
	for relationship in &fixtures.relationships {
		assert!(state.user(relationship.id).is_some());
		checked += 1;
	}
	assert!(checked > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deduplicated_users_resolve_recipients_and_relationships() {
	let running = Running::start("capabilities-dedupe").await;
	running.connected().await;
	assert_users_resolved(&running.session().state().read(), running.fixtures());
	running.stop().await;
}

fn shown_presences(running: &Running) -> Vec<Snowflake> {
	running
		.fixtures()
		.guilds
		.iter()
		.flat_map(|guild| guild.presences.iter())
		.filter(|presence| !matches!(presence.status, StatusKind::Offline | StatusKind::Invisible))
		.filter_map(|presence| {
			presence
				.user_id
				.or_else(|| presence.user.as_ref().map(|user| user.id))
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prioritized_ready_delivers_presences_in_the_supplement() {
	let running = Running::start("capabilities-prioritized").await;
	let expected = shown_presences(&running);
	assert!(!expected.is_empty());
	running
		.until("presences from READY_SUPPLEMENTAL", |state: &State| {
			state.session().connected && expected.iter().all(|user_id| state.presence(*user_id).is_some())
		})
		.await;
	running.stop().await;
}

#[derive(Default)]
struct DispatchNames {
	seen: Mutex<Vec<String>>,
}

impl SessionHooks for DispatchNames {
	fn on_dispatch(&self, event: &DispatchEvent) -> Result<()> {
		let name = match event {
			DispatchEvent::MessageReactionAdd(_) => "MESSAGE_REACTION_ADD".to_owned(),
			DispatchEvent::Unknown { name, .. } => name.clone(),
			_ => return Ok(()),
		};
		self.seen.lock().push(name);
		Ok(())
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unadvertised_reaction_debounce_keeps_single_reaction_events() {
	let names = Arc::new(DispatchNames::default());
	let running = Running::start_with(Options::new("capabilities-reactions").hooks(names.clone())).await;
	let channel_id = running.open_text_channel().await;
	let message_id = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.ordered.last().map(|message| message.id))
		.expect("a message to react to");
	running.session().send(Command::AddReaction {
		channel_id,
		message_id,
		emoji: REACTION.to_owned(),
		burst: false,
	});
	running
		.until("the reaction applied to the message", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.ordered.iter().any(|message| {
					message.id == message_id
						&& message
							.reactions
							.iter()
							.any(|reaction| reaction.me && reaction.emoji.name.as_deref() == Some(REACTION))
				})
			})
		})
		.await;
	running
		.eventually("the hook to see MESSAGE_REACTION_ADD", || {
			names
				.seen
				.lock()
				.iter()
				.any(|name| name == "MESSAGE_REACTION_ADD")
		})
		.await;
	assert!(
		!names
			.seen
			.lock()
			.iter()
			.any(|name| name == "MESSAGE_REACTION_ADD_MANY")
	);
	running.stop().await;
}
