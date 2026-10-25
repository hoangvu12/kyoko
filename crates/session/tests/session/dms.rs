use std::time::Duration;

use mockcord::{Recording, Reply};
use protocol::{ChannelType, Snowflake};
use session::Command;
use state::State;

use crate::harness::Running;

async fn started(name: &'static str) -> Running {
	let running = Running::start(name).await;
	running.connected().await;
	running
}

fn conversation(running: &Running, kind: ChannelType) -> Snowflake {
	let state = running.session().state().read();
	state
		.dm_rows()
		.into_iter()
		.filter_map(|row| state.channel(row.channel_id))
		.find(|channel| channel.kind == kind)
		.map(|channel| channel.id)
		.expect("a fixture conversation of that kind")
}

fn closes(recording: &Recording, channel_id: Snowflake) -> usize {
	recording
		.requests_to("DELETE", &format!("/channels/{channel_id}"))
		.count()
}

fn listed(state: &State, channel_id: Snowflake) -> bool {
	state.dm_rows().iter().any(|row| row.channel_id == channel_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_a_conversation_deletes_the_channel_once_and_drops_it() {
	let running = started("dms-close").await;
	for kind in [ChannelType::Dm, ChannelType::GroupDm] {
		let channel_id = conversation(&running, kind);
		running.session().send(Command::CloseDm { channel_id });
		running
			.until("the conversation to leave the list", |state| {
				!listed(state, channel_id)
			})
			.await;
		tokio::time::sleep(Duration::from_millis(300)).await;
		assert_eq!(closes(&running.mock().recording(), channel_id), 1);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_close_keeps_the_conversation_and_reports_it() {
	let running = started("dms-close-refused").await;
	let channel_id = conversation(&running, ChannelType::Dm);
	running.mock().respond_once(
		"DELETE",
		&format!("/channels/{channel_id}"),
		Reply::error(403, 50001, "Missing Access"),
	);
	running.session().send(Command::CloseDm { channel_id });
	running
		.until("the refusal to be reported", |state| {
			state.last_rejection().is_some()
		})
		.await;
	assert!(listed(&running.session().state().read(), channel_id));
	assert_eq!(closes(&running.mock().recording(), channel_id), 1);
	running.stop().await;
}
