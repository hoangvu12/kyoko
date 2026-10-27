use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use mockcord::RestRequest;
use plugins::{Limits, Permission, Slot, Status};
use protocol::{ChannelType, Permissions, Snowflake};
use serde_json::json;
use session::RestTuning;
use state::State;

use crate::harness::Options;
use crate::support::{Lab, scratch, start_with};

const ID: &str = "read-all-notifications";
const ACK_BULK: &str = "/read-states/ack-bulk";
const ACK_BULK_LIMIT: usize = 100;
const BULK_GAP: Duration = Duration::from_millis(400);
const CHANNELS_PER_GUILD: usize = 70;
const QUIET: Duration = Duration::from_millis(600);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		bulk_ack_gap: BULK_GAP,
		..RestTuning::default()
	}
}

fn unread(state: &State) -> BTreeMap<Snowflake, Snowflake> {
	state
		.guilds()
		.flat_map(|guild| guild.channels.values())
		.filter(|channel| {
			state
				.my_permissions(channel.id)
				.contains(Permissions::VIEW_CHANNEL)
		})
		.chain(state.private_channels())
		.filter(|channel| channel.kind != ChannelType::GuildCategory)
		.filter_map(|channel| {
			let read_state = state.read_state(channel.id)?;
			let newest = channel.last_message_id?;
			let behind = read_state.last_message_id < Some(newest);
			(behind || read_state.mention_count.unwrap_or(0) > 0).then_some((channel.id, newest))
		})
		.collect()
}

fn bulk_acks(lab: &Lab) -> Vec<RestRequest> {
	lab.running
		.mock()
		.recording()
		.requests_to("POST", ACK_BULK)
		.cloned()
		.collect()
}

fn click(lab: &Lab, key: &str) {
	let context = [
		("source".to_owned(), "slot".to_owned()),
		("slot".to_owned(), "rail.buttons".to_owned()),
	];
	lab.host
		.gesture(ID, key, &context)
		.expect("the click reaches the plugin");
}

fn entries(request: &RestRequest) -> Vec<serde_json::Value> {
	request.json().expect("a json body")["read_states"]
		.as_array()
		.expect("read states")
		.clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_button_marks_everything_read_in_paced_bulk_acks_of_at_most_a_hundred() {
	let options = Options::new("read-all")
		.channels(CHANNELS_PER_GUILD)
		.rest(tuning());
	let lab = start_with(options, scratch("read-all"), Limits::default(), true).await;
	lab.running.connected().await;
	let expected = unread(&lab.running.session().state().read());
	assert!(
		expected.len() > ACK_BULK_LIMIT,
		"the fixtures stage more unread channels than one request takes"
	);

	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let declared: BTreeSet<Permission> = lab
		.host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == ID)
		.expect("the plugin is bundled")
		.permissions
		.into_iter()
		.collect();
	assert_eq!(
		declared,
		BTreeSet::from([
			Permission::parse("action:mark_read"),
			Permission::Ui(Slot::RailButtons),
		])
	);
	let declared: Vec<Permission> = declared.into_iter().collect();
	lab.host.approve(ID, &declared).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));

	click(&lab, "something_else");
	tokio::time::sleep(QUIET).await;
	assert!(
		bulk_acks(&lab).is_empty(),
		"running, or a click on another key, marks nothing read"
	);

	click(&lab, "read_all");
	lab.running
		.until("every badge to clear", |state| {
			state
				.guild_summaries()
				.iter()
				.all(|guild| !guild.unread && guild.mentions == 0)
				&& state.dm_rows().iter().all(|dm| !dm.unread && dm.mentions == 0)
		})
		.await;
	let requests = lab
		.running
		.until_wire("bulk acks for every unread channel", |recording| {
			let requests: Vec<RestRequest> = recording.requests_to("POST", ACK_BULK).cloned().collect();
			let acked: usize = requests.iter().map(|request| entries(request).len()).sum();
			(acked >= expected.len()).then_some(requests)
		})
		.await;
	assert_eq!(
		requests.len(),
		expected.len().div_ceil(ACK_BULK_LIMIT),
		"as few requests as the limit allows"
	);
	let mut acked = BTreeMap::new();
	for request in &requests {
		let read_states = entries(request);
		assert!(read_states.len() <= ACK_BULK_LIMIT);
		for entry in read_states {
			let channel_id: u64 = entry["channel_id"]
				.as_str()
				.and_then(|id| id.parse().ok())
				.expect("a channel id");
			let message_id: u64 = entry["message_id"]
				.as_str()
				.and_then(|id| id.parse().ok())
				.expect("a message id");
			assert_eq!(entry["read_state_type"], json!(0));
			assert!(
				acked
					.insert(Snowflake(channel_id), Snowflake(message_id))
					.is_none(),
				"each channel is acked once"
			);
		}
	}
	assert_eq!(
		acked, expected,
		"exactly the unread channels, each up to its newest message"
	);
	lab.running.assert_released_apart(&requests, BULK_GAP, "bulk ack");
	assert_eq!(
		lab.running
			.mock()
			.recording()
			.requests_to("POST", "/channels/*/messages/*/ack")
			.count(),
		0,
		"never one ack per channel"
	);

	click(&lab, "read_all");
	tokio::time::sleep(QUIET).await;
	assert_eq!(
		bulk_acks(&lab).len(),
		requests.len(),
		"with nothing unread a second click sends nothing"
	);
	lab.stop().await;
}
