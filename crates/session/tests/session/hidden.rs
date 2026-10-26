use std::time::Duration;

use mockcord::Recording;
use protocol::{ChannelType, Opcode, Snowflake};
use session::Command;
use state::State;

use crate::harness::Running;

const QUIET: Duration = Duration::from_millis(800);

fn hidden_channel(state: &State) -> (Snowflake, Snowflake, Snowflake) {
	state
		.guilds()
		.find_map(|guild| {
			let hidden = guild
				.channels
				.values()
				.find(|channel| channel.kind == ChannelType::GuildText && state.is_hidden(channel.id))?;
			Some((guild.id, hidden.id, hidden.last_message_id?))
		})
		.expect("a text channel the user cannot view in the fixtures")
}

fn visible_channel(state: &State, guild_id: Snowflake) -> Snowflake {
	state
		.guild(guild_id)
		.and_then(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.kind == ChannelType::GuildText && state.can_view(channel.id))
		})
		.map(|channel| channel.id)
		.expect("a text channel the user can view")
}

fn naming(recording: &Recording, channel_id: Snowflake) -> Vec<String> {
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

fn reading(guild_id: Snowflake, channel_id: Snowflake, last: Snowflake) -> Vec<Command> {
	vec![
		Command::SelectChannel {
			guild_id: Some(guild_id),
			channel_id,
		},
		Command::LoadOlder { channel_id },
		Command::LoadNewer { channel_id },
		Command::LoadAround {
			channel_id,
			message_id: last,
		},
		Command::LoadPins { channel_id },
		Command::LoadApplicationCommands { channel_id },
		Command::LoadArchivedThreads {
			channel_id,
			more: false,
		},
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
		Command::MarkGuildRead { guild_id },
	]
}

async fn open(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the visible channel to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hidden_channel_is_never_fetched_subscribed_acked_or_typed_in() {
	let mut running = Running::start("hidden-channel").await;
	running.connected().await;
	let (guild_id, hidden, last) = hidden_channel(&running.session().state().read());
	let visible = visible_channel(&running.session().state().read(), guild_id);
	running.session().send(Command::SubscribeGuild { guild_id });
	for command in reading(guild_id, hidden, last) {
		running.session().send(command);
	}
	open(&running, guild_id, visible).await;
	tokio::time::sleep(QUIET).await;
	assert!(
		running.session().state().read().messages(hidden).is_none(),
		"no history of the hidden channel was loaded"
	);
	let traffic = running.mock().inspect(|recording| naming(recording, hidden));
	assert!(
		traffic.is_empty(),
		"nothing names the hidden channel: {traffic:?}"
	);
	let subscribed = running
		.mock()
		.inspect(|recording| recording.frames_with(Opcode::GuildSubscriptionsBulk).count());
	assert!(subscribed > 0, "the server itself is still subscribed");

	running.restart().await;
	running.connected().await;
	running.session().send(Command::SubscribeGuild { guild_id });
	for command in reading(guild_id, hidden, last) {
		running.session().send(command);
	}
	open(&running, guild_id, visible).await;
	tokio::time::sleep(QUIET).await;
	let traffic = running.mock().inspect(|recording| naming(recording, hidden));
	assert!(
		traffic.is_empty(),
		"nothing names the hidden channel after a restart: {traffic:?}"
	);
	running.stop().await;
}
