use std::collections::BTreeSet;

use bridge::{ChannelKind, WINDOW};
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

fn first_text_channel(running: &Running) -> Snowflake {
	running
		.view
		.sidebar
		.channels()
		.iter()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("the first guild has a text channel")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opening_a_channel_loads_its_history_and_appends_live_messages() {
	let mut running = Running::start("timeline").await;
	let channel_id = first_text_channel(&running);
	let command = running.open_channel(channel_id).expect("a select command");
	assert!(matches!(command, Command::SelectChannel { channel_id: id, .. } if id == channel_id));
	assert!(running.open_channel(channel_id).is_none());
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			!view.timeline.messages().is_empty()
		})
		.await;
	let loaded = running.view.timeline.messages().len();
	assert!(loaded <= WINDOW);

	let posted = running
		.server
		.post_message(channel_id, "spike live message")
		.expect("mockcord posts the message");
	running
		.until("the live message to appear", |view| {
			view.timeline
				.messages()
				.last()
				.is_some_and(|message| message.id == posted)
		})
		.await;
	let messages = running.view.timeline.messages();
	let last = messages.last().expect("a last message");
	assert_eq!(last.content, "spike live message");
	assert!(!last.author.is_empty());
	assert!(!last.timestamp.is_empty());
	let ids: BTreeSet<Snowflake> = messages.iter().map(|message| message.id).collect();
	assert_eq!(ids.len(), messages.len(), "no message is listed twice");
	assert!(
		messages.windows(2).all(|pair| pair[0].id < pair[1].id),
		"messages stay in id order"
	);

	let second = running
		.server
		.post_message(channel_id, "another live message")
		.expect("mockcord posts again");
	running
		.until("the second live message to appear", |view| {
			view.timeline
				.messages()
				.last()
				.is_some_and(|message| message.id == second)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_in_other_channels_leave_the_open_one_alone() {
	let mut running = Running::start("timeline-other").await;
	let channel_id = first_text_channel(&running);
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			!view.timeline.messages().is_empty()
		})
		.await;
	let before = running.view.timeline.messages().to_vec();
	let other = running
		.server
		.create_channel(guild_id, "elsewhere")
		.expect("mockcord creates a channel");
	running
		.until("the other channel to be listed", |view| {
			view.sidebar.channels().iter().any(|channel| channel.id == other)
		})
		.await;
	running
		.server
		.post_message(other, "not here")
		.expect("mockcord posts elsewhere");
	let marker = running
		.server
		.post_message(channel_id, "here")
		.expect("mockcord posts into the open channel");
	running
		.until("the open channel's message to appear", |view| {
			view.timeline
				.messages()
				.last()
				.is_some_and(|message| message.id == marker)
		})
		.await;
	let after = running.view.timeline.messages();
	assert!(after.iter().all(|message| message.content != "not here"));
	let ids = |messages: &[bridge::MessageEntry]| -> Vec<Snowflake> {
		messages.iter().map(|message| message.id).collect()
	};
	let kept = ids(&after[..after.len() - 1]);
	assert!(ids(&before).ends_with(&kept), "the earlier messages stay listed");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_busiest_channel_is_the_one_with_the_newest_message() {
	let mut running = Running::start("timeline-busiest").await;
	let guild = running.view.sidebar.guilds().last().expect("a guild").id;
	let channel_id = running
		.server
		.fixtures()
		.guild(guild.0)
		.and_then(|guild| guild.message_channel_ids.first().copied())
		.map(Snowflake)
		.expect("the guild has a message channel");
	let posted = running
		.server
		.post_message(channel_id, "newest anywhere")
		.expect("mockcord posts");
	let state = running.session().state().clone();
	running
		.until("the channel to know its newest message", |_| {
			state
				.read()
				.channel(channel_id)
				.is_some_and(|channel| channel.last_message_id == Some(posted))
		})
		.await;
	let busiest = bridge::busiest_channel(&state.read()).expect("a busiest channel");
	assert_eq!(busiest.channel_id, channel_id);
	assert_eq!(busiest.guild_id, Some(guild));
	running.stop().await;
}
