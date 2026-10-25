use std::collections::BTreeSet;

use bridge::{ChannelKind, Row, RowKey};
use protocol::Snowflake;
use session::Command;

use crate::harness::{Running, View};

fn text_channels(running: &Running) -> Vec<Snowflake> {
	running
		.view
		.sidebar
		.channels()
		.iter()
		.filter(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.collect()
}

fn first_text_channel(running: &Running) -> Snowflake {
	text_channels(running)
		.first()
		.copied()
		.expect("the first guild has a text channel")
}

fn message_ids(view: &View) -> Vec<Snowflake> {
	view.timeline.messages().map(|message| message.id).collect()
}

fn has_messages(view: &View) -> bool {
	view.timeline.messages().next().is_some()
}

fn assert_ordered_and_unique(view: &View) {
	let ids = message_ids(view);
	let unique: BTreeSet<Snowflake> = ids.iter().copied().collect();
	assert_eq!(unique.len(), ids.len(), "no message is listed twice");
	assert!(
		ids.windows(2).all(|pair| pair[0] < pair[1]),
		"messages stay in id order"
	);
	let keys: Vec<&RowKey> = view.timeline.rows().iter().map(|row| &row.key).collect();
	let distinct: std::collections::HashSet<&RowKey> = keys.iter().copied().collect();
	assert_eq!(distinct.len(), keys.len(), "row keys are unique");
}

async fn open_loaded(running: &mut Running, channel_id: Snowflake) {
	let command = running.open_channel(channel_id).expect("a select command");
	assert!(matches!(command, Command::SelectChannel { channel_id: id, .. } if id == channel_id));
	let delta = running
		.view
		.timeline
		.take_delta()
		.expect("selecting publishes a delta");
	assert!(delta.reset, "a newly opened channel starts from a reset");
	running.session().send(command);
	running.until("the channel history to load", has_messages).await;
	running.view.timeline.take_delta();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_messages_are_appended_to_the_open_channel() {
	let mut running = Running::start("timeline").await;
	let channel_id = first_text_channel(&running);
	open_loaded(&mut running, channel_id).await;
	assert!(running.open_channel(channel_id).is_none());
	let before = message_ids(&running.view);

	let posted = running
		.server
		.post_message(channel_id, "a live message")
		.expect("mockcord posts the message");
	running
		.until("the live message to appear", |view| {
			view.timeline
				.messages()
				.next_back()
				.is_some_and(|message| message.id == posted)
		})
		.await;
	let delta = running
		.view
		.timeline
		.take_delta()
		.expect("the append is published");
	assert!(!delta.reset);
	assert_eq!(delta.prepended, 0);
	assert!(
		delta.appended >= 1,
		"the new row is reported as appended: {delta:?}"
	);
	assert!(delta.removed.is_empty());
	let last = running
		.view
		.timeline
		.messages()
		.next_back()
		.expect("a last message");
	assert_eq!(last.content, "a live message");
	assert!(!last.author.is_empty());
	assert_eq!(&message_ids(&running.view)[..before.len()], &before[..]);
	assert_ordered_and_unique(&running.view);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn older_history_arrives_as_a_prepend_batch() {
	let mut running = Running::start("timeline-older").await;
	let channel_id = first_text_channel(&running);
	open_loaded(&mut running, channel_id).await;
	assert!(
		running.view.timeline.has_older(),
		"the fixture channel has older pages"
	);
	let before = message_ids(&running.view);
	let oldest = before[0];

	running.session().send(Command::LoadOlder { channel_id });
	running
		.until("an older page to load", |view| {
			view.timeline
				.messages()
				.next()
				.is_some_and(|message| message.id < oldest)
		})
		.await;
	let delta = running.view.timeline.take_delta().expect("the page is published");
	assert!(!delta.reset);
	assert_eq!(delta.appended, 0);
	assert!(delta.removed.iter().all(|key| !matches!(key, RowKey::Message(_))));
	let after = message_ids(&running.view);
	let added = after.len() - before.len();
	assert!(added > 0);
	assert!(
		delta.prepended >= added,
		"every older message is part of the prepended batch: {delta:?}"
	);
	assert_eq!(&after[added..], &before[..], "the earlier rows keep their order");
	assert!(matches!(
		running.view.timeline.rows()[0].row,
		Row::Loading | Row::Start
	));
	assert_ordered_and_unique(&running.view);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edits_and_deletes_update_rows_in_place() {
	let mut running = Running::start("timeline-edits").await;
	let channel_id = first_text_channel(&running);
	open_loaded(&mut running, channel_id).await;
	let ids = message_ids(&running.view);
	let edited = ids[ids.len() - 2];
	let deleted = ids[ids.len() - 3];
	let control = running.server.control();

	assert!(control.edit_message(channel_id.0, edited.0, "changed words"));
	running
		.until("the edit to show", |view| {
			view.timeline
				.messages()
				.any(|message| message.id == edited && message.content == "changed words" && message.edited)
		})
		.await;
	let delta = running.view.timeline.take_delta().expect("the edit is published");
	assert!(!delta.reset);
	assert_eq!((delta.prepended, delta.appended), (0, 0));
	assert_eq!(delta.updated, vec![RowKey::Message(edited)]);
	assert!(delta.removed.is_empty());

	assert!(control.delete_message(channel_id.0, deleted.0));
	running
		.until("the deleted message to go", |view| {
			view.timeline.messages().all(|message| message.id != deleted)
		})
		.await;
	let delta = running
		.view
		.timeline
		.take_delta()
		.expect("the delete is published");
	assert!(!delta.reset);
	assert!(delta.removed.contains(&RowKey::Message(deleted)));
	assert!(delta.inserted.is_empty());
	assert_eq!(message_ids(&running.view).len(), ids.len() - 1);
	assert_ordered_and_unique(&running.view);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_channels_resets_the_rows() {
	let mut running = Running::start("timeline-switch").await;
	let channels = text_channels(&running);
	let (first, second) = (channels[0], channels[1]);
	open_loaded(&mut running, first).await;
	let command = running.open_channel(second).expect("a select command");
	let delta = running
		.view
		.timeline
		.take_delta()
		.expect("the switch is published");
	assert!(delta.reset);
	running.session().send(command);
	running
		.until("the second channel to load", |view| {
			view.timeline.open().map(|open| open.channel_id) == Some(second)
				&& view
					.timeline
					.messages()
					.next()
					.is_some_and(|message| message.message.channel_id == second)
		})
		.await;
	assert!(
		running
			.view
			.timeline
			.messages()
			.all(|message| message.message.channel_id == second)
	);
	assert_ordered_and_unique(&running.view);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_in_other_channels_leave_the_open_one_alone() {
	let mut running = Running::start("timeline-other").await;
	let channel_id = first_text_channel(&running);
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	open_loaded(&mut running, channel_id).await;
	let before = message_ids(&running.view);
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
				.next_back()
				.is_some_and(|message| message.id == marker)
		})
		.await;
	assert!(
		running
			.view
			.timeline
			.messages()
			.all(|message| message.content != "not here")
	);
	let after = message_ids(&running.view);
	assert_eq!(
		&after[..after.len() - 1],
		&before[..],
		"the earlier messages stay listed"
	);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_first_channel_is_the_top_text_channel_of_the_top_guild() {
	let running = Running::start("timeline-first").await;
	let guild = running.view.sidebar.guilds().first().expect("a guild").id;
	assert_eq!(running.view.sidebar.selected(), Some(guild));
	let expected = running
		.view
		.sidebar
		.channels()
		.iter()
		.find(|channel| matches!(channel.kind, ChannelKind::Text | ChannelKind::Announcement))
		.map(|channel| channel.id)
		.expect("the top guild has a text channel");
	let first = bridge::first_channel(&running.session().state().read()).expect("a first channel");
	assert_eq!(first.guild_id, Some(guild));
	assert_eq!(first.channel_id, expected);
	running.stop().await;
}
