use std::sync::Arc;
use std::time::Duration;

use mockcord::{
	BROKEN_COMMAND, IGNORE_COMMAND, InteractionTiming, MOCKBOT_ID, PING_COMMAND, ROLL_COMMAND, Reply,
	THINK_COMMAND, THOUGHT, WHISPER_COMMAND, WHISPERED,
};
use protocol::{ChannelType, CommandOptionType, InteractionOption, Message, Snowflake};
use serde_json::json;
use session::{Command, RestTuning};
use state::{CommandStage, State};

use crate::harness::{Options, Running};

const NO_RESPONSE: &str = "The application did not respond";
const QUIET: Duration = Duration::from_millis(600);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

fn unpaced() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn timing(followup: Duration, window: Duration, hold_ping: bool) -> InteractionTiming {
	InteractionTiming {
		followup,
		window,
		hold_ping,
	}
}

async fn start(name: &'static str) -> (Running, Snowflake) {
	let running = Running::start_with(Options::new(name).rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	running
		.session()
		.send(Command::LoadApplicationCommands { channel_id });
	running
		.until("the command index to load", |state| {
			state.command_index(channel_id).is_some()
		})
		.await;
	(running, channel_id)
}

fn run(running: &Running, channel_id: Snowflake, command: u64) {
	let options = if command == ROLL_COMMAND {
		vec![InteractionOption {
			kind: CommandOptionType::Integer,
			name: "sides".to_owned(),
			value: Some(json!(20)),
			options: Vec::new(),
			focused: None,
		}]
	} else {
		Vec::new()
	};
	running.session().send(Command::RunApplicationCommand {
		channel_id,
		command_id: Snowflake(command),
		options,
		files: Vec::new(),
	});
}

fn invoked(message: &Message, name: &str) -> bool {
	message
		.interaction_header()
		.is_some_and(|header| header.name == name)
}

fn stage(state: &State, channel_id: Snowflake, name: &str) -> Option<CommandStage> {
	state
		.messages(channel_id)?
		.pending
		.iter()
		.find(|pending| invoked(&pending.message, name))?
		.command
		.clone()
}

fn answer(state: &State, channel_id: Snowflake, name: &str) -> Option<Arc<Message>> {
	let cache = state.messages(channel_id)?;
	cache
		.ordered
		.iter()
		.chain(cache.ephemeral())
		.rev()
		.find(|message| invoked(message, name))
		.cloned()
}

fn ephemeral_with(state: &State, channel_id: Snowflake, content: &str) -> Option<Snowflake> {
	state
		.messages(channel_id)?
		.ephemeral()
		.iter()
		.find(|message| message.content == content)
		.map(|message| message.id)
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

fn other_text_channel(state: &State, channel_id: Snowflake) -> (Option<Snowflake>, Snowflake) {
	state
		.guilds()
		.find_map(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.kind == ChannelType::GuildText && channel.id != channel_id)
				.map(|channel| (Some(guild.id), channel.id))
		})
		.expect("a second text channel")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_command_shows_sending_until_discord_takes_it_and_the_reply_replaces_it() {
	let (running, channel_id) = start("command-replies-sending").await;
	running
		.mock()
		.set_interaction_timing(timing(Duration::from_secs(2), Duration::from_secs(3), true));
	run(&running, channel_id, PING_COMMAND);
	running
		.until("the pending command row", |state| {
			stage(state, channel_id, "ping") == Some(CommandStage::Sending)
		})
		.await;
	tokio::time::sleep(QUIET).await;
	{
		let state = running.session().state().read();
		assert_eq!(stage(&state, channel_id, "ping"), Some(CommandStage::Sending));
		let pending = state
			.messages(channel_id)
			.and_then(|cache| cache.pending.first().cloned())
			.expect("the pending row");
		assert_eq!(pending.message.author.id.0, MOCKBOT_ID);
		assert_eq!(pending.message.author.username, "Mockbot");
		let header = pending
			.message
			.interaction_header()
			.expect("an interaction header");
		assert_eq!(Some(header.user.id), state.current_user_id());
	}

	run(&running, channel_id, ROLL_COMMAND);
	running
		.until("the roll to be answered", |state| {
			answer(state, channel_id, "roll").is_some() && stage(state, channel_id, "roll").is_none()
		})
		.await;
	{
		let state = running.session().state().read();
		let reply = answer(&state, channel_id, "roll").expect("the reply");
		assert_eq!(reply.content, "\u{1f3b2} You rolled a 17");
		assert_eq!(reply.author.id.0, MOCKBOT_ID);
		assert_eq!(
			reply.interaction_header().map(|header| header.user.id),
			state.current_user_id()
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deferred_reply_thinks_until_the_follow_up_fills_it() {
	let (running, channel_id) = start("command-replies-deferred").await;
	running
		.mock()
		.set_interaction_timing(timing(Duration::from_millis(1500), Duration::from_secs(3), false));
	run(&running, channel_id, THINK_COMMAND);
	running
		.until("the deferred reply", |state| {
			answer(state, channel_id, "think").is_some_and(|message| message.is_loading())
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(
			stage(&state, channel_id, "think"),
			None,
			"the reply replaced the pending row"
		);
		let reply = answer(&state, channel_id, "think").expect("the reply");
		assert!(reply.content.is_empty());
		assert!(!reply.is_ephemeral());
	}
	running
		.until("the follow-up", |state| {
			answer(state, channel_id, "think")
				.is_some_and(|message| !message.is_loading() && message.content == THOUGHT)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ephemeral_reply_lives_only_in_memory_until_dismissed() {
	let (mut running, channel_id) = start("command-replies-ephemeral").await;
	run(&running, channel_id, WHISPER_COMMAND);
	running
		.until("the ephemeral reply", |state| {
			ephemeral_with(state, channel_id, WHISPERED).is_some()
		})
		.await;
	let whispered = {
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the channel cache");
		assert!(cache.pending.is_empty());
		assert!(!cache.ordered.iter().any(|message| message.content == WHISPERED));
		ephemeral_with(&state, channel_id, WHISPERED).expect("the ephemeral reply")
	};

	let (guild_id, other) = other_text_channel(&running.session().state().read(), channel_id);
	running.session().send(Command::SelectChannel {
		guild_id,
		channel_id: other,
	});
	running
		.until("the other channel", |state| {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == other)
		})
		.await;
	running
		.session()
		.send(Command::SelectChannel { guild_id, channel_id });
	running
		.until("back in the first channel", |state| {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == channel_id)
		})
		.await;
	assert_eq!(
		ephemeral_with(&running.session().state().read(), channel_id, WHISPERED),
		Some(whispered),
		"a channel switch keeps it, as on the web"
	);

	tokio::time::sleep(QUIET).await;
	let stored = running
		.store()
		.db
		.recent_messages(channel_id.get(), 1000)
		.expect("cached messages are readable");
	assert!(!stored.iter().any(|(id, _)| *id == whispered.get()));

	running.session().send(Command::DismissMessage {
		channel_id,
		message_id: whispered,
	});
	running
		.until("the dismissed reply to go", |state| {
			ephemeral_with(state, channel_id, WHISPERED).is_none()
		})
		.await;

	run(&running, channel_id, WHISPER_COMMAND);
	running
		.until("a second ephemeral reply", |state| {
			ephemeral_with(state, channel_id, WHISPERED).is_some()
		})
		.await;
	let session_id = running.session().state().read().session().session_id.clone();
	running.mock().close_gateway(4009);
	running
		.until("a new gateway session", |state| {
			!state.session().session_id.is_empty() && state.session().session_id != session_id
		})
		.await;
	assert_eq!(
		ephemeral_with(&running.session().state().read(), channel_id, WHISPERED),
		None,
		"a new gateway session drops ephemeral messages"
	);

	run(&running, channel_id, WHISPER_COMMAND);
	running
		.until("a third ephemeral reply", |state| {
			ephemeral_with(state, channel_id, WHISPERED).is_some()
		})
		.await;
	running.restart().await;
	running.connected().await;
	running.session().send(Command::SelectChannel {
		guild_id: running.session().state().read().channel_guild(channel_id),
		channel_id,
	});
	running
		.until("the channel history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the channel cache");
		assert!(cache.ephemeral().is_empty());
		assert!(!cache.ordered.iter().any(|message| message.content == WHISPERED));
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reply_that_fails_or_never_comes_says_the_application_did_not_respond() {
	let (running, channel_id) = start("command-replies-failed").await;
	running
		.mock()
		.set_interaction_timing(timing(Duration::from_secs(2), Duration::from_millis(1500), false));
	let before = rejections(&running);

	run(&running, channel_id, BROKEN_COMMAND);
	running
		.until("the failed reply", |state| {
			stage(state, channel_id, "broken") == Some(CommandStage::Failed(NO_RESPONSE.to_owned()))
		})
		.await;
	let nonce = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.pending.first()?.message.nonce.clone())
		.expect("the failed row has a nonce");
	running
		.session()
		.send(Command::DiscardMessage { channel_id, nonce });
	running
		.until("the dismissed failure to go", |state| {
			stage(state, channel_id, "broken").is_none()
		})
		.await;

	run(&running, channel_id, IGNORE_COMMAND);
	running
		.until("Discord to take the command", |state| {
			stage(state, channel_id, "ignore") == Some(CommandStage::Thinking)
		})
		.await;
	running
		.until("the application to time out", |state| {
			stage(state, channel_id, "ignore") == Some(CommandStage::Failed(NO_RESPONSE.to_owned()))
		})
		.await;

	running
		.mock()
		.respond_once("POST", "/interactions", Reply::captcha_required());
	run(&running, channel_id, BROKEN_COMMAND);
	running
		.until("the refused command", |state| {
			stage(state, channel_id, "broken")
				== Some(CommandStage::Failed(
					"/broken: complete this in the official Discord client".to_owned(),
				))
		})
		.await;

	running.mock().hold_requests("POST", "/interactions");
	run(&running, channel_id, PING_COMMAND);
	running
		.until("the held command to be sending", |state| {
			stage(state, channel_id, "ping") == Some(CommandStage::Sending)
		})
		.await;
	tokio::time::sleep(REQUEST_TIMEOUT / 2).await;
	assert_eq!(
		stage(&running.session().state().read(), channel_id, "ping"),
		Some(CommandStage::Sending)
	);
	running
		.until("the request to time out", |state| {
			stage(state, channel_id, "ping") == Some(CommandStage::Failed(NO_RESPONSE.to_owned()))
		})
		.await;
	running.mock().release_requests();
	assert_eq!(
		rejections(&running),
		before,
		"failures show in the row, not as a toast"
	);
	running.stop().await;
}
