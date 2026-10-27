use std::cell::RefCell;
use std::time::Instant;

use bridge::{Connection, ConnectionWatch, Row, SidebarRow};
use mockcord::fixtures::CURRENT_USER_ID;
use protocol::{ChannelType, Snowflake};

use crate::harness::Running;

fn without_read_state(rows: &[SidebarRow], opened: Snowflake) -> Vec<SidebarRow> {
	rows.iter()
		.cloned()
		.map(|row| match row {
			SidebarRow::Channel(mut channel) if channel.id == opened => {
				channel.unread = false;
				channel.mentions = 0;
				SidebarRow::Channel(channel)
			}
			other => other,
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opening_a_channel_changes_only_the_message_pane() {
	let mut running = Running::start("channel-open-isolated").await;
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == bridge::ChannelKind::Text && !channel.thread)
		.map(|channel| channel.id)
		.expect("a text channel");
	let thread_id = running
		.server
		.control()
		.add_unjoined_thread(channel_id.0, "drive-by thread")
		.map(Snowflake)
		.expect("mockcord adds the thread");
	let watch = RefCell::new(ConnectionWatch::default());
	running
		.until_state("the session to be live", |state| {
			let mut watch = watch.borrow_mut();
			watch.observe(&state.session().connection, Instant::now());
			*watch.current() == Connection::Live
		})
		.await;
	let rows_before = running.view.sidebar.rows().to_vec();
	let rail_before = running.view.rail.items().to_vec();

	let command = running.open_channel(channel_id).expect("the channel opens");
	running.session().send(command);
	assert!(
		matches!(
			running.view.timeline.rows().first().map(|row| &row.row),
			Some(Row::Loading)
		),
		"the pane shows the skeleton until the first page"
	);
	let banners = RefCell::new(Vec::new());
	running
		.until_state("the page and the thread sync", |state| {
			let mut watch = watch.borrow_mut();
			watch.observe(&state.session().connection, Instant::now());
			banners.borrow_mut().push(watch.current().clone());
			let synced = state
				.guild(guild_id)
				.is_some_and(|guild| guild.threads.contains_key(&thread_id));
			let loaded = state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty());
			synced && loaded
		})
		.await;
	let fence = running
		.server
		.control()
		.post_message(channel_id.get(), CURRENT_USER_ID, "after the sync")
		.expect("mockcord posts the message");
	running
		.until("the view to catch up past the sync", |view| {
			view.timeline.messages().any(|entry| entry.id.get() == fence)
		})
		.await;

	assert!(
		banners.borrow().iter().all(|shown| *shown == Connection::Live),
		"no connection banner while a channel opens: {:?}",
		banners.borrow()
	);
	assert_eq!(
		without_read_state(running.view.sidebar.rows(), channel_id),
		without_read_state(&rows_before, channel_id),
		"the channel list is unchanged by a channel open, apart from the opened channel's read state"
	);
	assert!(
		running.view.sidebar.channel(thread_id).is_none(),
		"a thread the user has not joined stays out of the channel list"
	);
	assert_eq!(
		running.view.rail.items().len(),
		rail_before.len(),
		"the server rail keeps its items"
	);
	let state = running.session().state().clone();
	assert!(
		state
			.read()
			.channel(thread_id)
			.is_some_and(|thread| thread.kind == ChannelType::PublicThread),
		"the synced thread is still known to the threads popout"
	);
	running.stop().await;
}
