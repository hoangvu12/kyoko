use std::time::{Duration, Instant};

use mockcord::{Control, GatewayFrame, Recording};
use protocol::{ChannelType, MessageType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::{Command, VoiceSetup};
use state::{State, VoiceStatus};

use crate::harness::{Options, Running};
use crate::voice::{self, SETTLE, Voice, manual_setup};

const RING_TIMEOUT: Duration = Duration::from_millis(600);
const NO_LOOP_WINDOW: Duration = Duration::from_millis(800);
const OWN_ID: u64 = mockcord::fixtures::CURRENT_USER_ID;

struct Private {
	channel_id: Snowflake,
	recipients: Vec<Snowflake>,
}

fn private_channels(running: &Running) -> Vec<Private> {
	let state = running.session().state().read();
	running
		.fixtures()
		.private_channel_ids
		.iter()
		.filter_map(|id| state.channel(Snowflake(*id)))
		.map(|channel| Private {
			channel_id: channel.id,
			recipients: channel.recipient_ids.clone(),
		})
		.collect()
}

fn direct(running: &Running, index: usize) -> Private {
	private_channels(running)
		.into_iter()
		.filter(|channel| channel.recipients.len() == 1)
		.nth(index)
		.expect("a direct message in the fixtures")
}

fn group_of_three(running: &Running) -> Private {
	let state = running.session().state().read();
	let found = running
		.fixtures()
		.private_channel_ids
		.iter()
		.filter_map(|id| state.channel(Snowflake(*id)))
		.find(|channel| channel.kind == ChannelType::GroupDm && channel.recipient_ids.len() == 2)
		.map(|channel| Private {
			channel_id: channel.id,
			recipients: channel.recipient_ids.clone(),
		});
	found.expect("a group of three in the fixtures")
}

fn incoming(state: &State, channel_id: Snowflake) -> bool {
	state.incoming_calls().any(|call| call.channel_id == channel_id)
}

fn ring_requests(recording: &Recording, channel_id: Snowflake, path: &str) -> Vec<Value> {
	recording
		.requests_to("POST", &format!("/channels/{channel_id}/call/{path}"))
		.map(|request| request.json().unwrap_or(Value::Null))
		.collect()
}

fn op_4_count(recording: &Recording, since: Instant) -> usize {
	recording
		.frames_with(Opcode::VoiceStateUpdate)
		.filter(|frame| frame.at > since)
		.count()
}

fn is_dm_op_4(frame: &GatewayFrame, channel_id: Option<Snowflake>) -> bool {
	frame.data["guild_id"].is_null()
		&& match channel_id {
			Some(channel_id) => frame.data["channel_id"] == json!(channel_id.to_string()),
			None => frame.data["channel_id"].is_null(),
		}
}

async fn start(name: &'static str) -> Voice {
	let voice = voice::start(name).await;
	voice.running.session().set_voice_setup(VoiceSetup {
		ring_timeout: RING_TIMEOUT,
		..manual_setup(&voice.running)
	});
	voice
}

async fn rung(voice: &Voice, channel_id: Snowflake) {
	voice
		.running
		.until("the call to ring", |state| incoming(state, channel_id))
		.await;
}

async fn not_rung(voice: &Voice, channel_id: Snowflake, what: &str) {
	voice
		.running
		.until(what, |state| !incoming(state, channel_id))
		.await;
}

fn mock(voice: &Voice) -> &Control {
	voice.running.mock()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starting_a_dm_call_rings_once_connects_and_ends_when_everyone_leaves() {
	let voice = start("calls-start").await;
	let dm = direct(&voice.running, 0);
	let friend = dm.recipients[0];
	voice.running.session().send(Command::SelectChannel {
		guild_id: None,
		channel_id: dm.channel_id,
	});
	let since = voice.command(Command::StartCall {
		channel_id: dm.channel_id,
		video: false,
	});
	let join = voice
		.op_4(since, "op 4 into the DM without a guild", |frame| {
			is_dm_op_4(frame, Some(dm.channel_id))
		})
		.await;
	assert_eq!(join.data["self_video"], json!(false));
	voice.connected_to(dm.channel_id).await;
	let identify = voice
		.voice_until("the voice identify", |recording| {
			recording.frames_with(0).next().is_some()
		})
		.await;
	let identify = identify.frames_with(0).last().expect("an identify");
	assert_eq!(identify.data["server_id"], json!(dm.channel_id.to_string()));
	assert_eq!(identify.data["channel_id"], json!(dm.channel_id.to_string()));
	let rings = voice
		.running
		.until_wire("the ring", |recording| {
			let rings = ring_requests(recording, dm.channel_id, "ring");
			(!rings.is_empty()).then_some(rings)
		})
		.await;
	assert_eq!(rings, vec![json!({ "recipients": null })]);
	voice
		.running
		.until("the friend to ring", |state| {
			state.call(dm.channel_id).is_some_and(|call| call.rings(friend))
		})
		.await;
	assert!(
		!incoming(&voice.running.session().state().read(), dm.channel_id),
		"your own call never rings you"
	);
	mock(&voice).call_join_as(dm.channel_id.get(), friend.get());
	voice
		.running
		.until("the friend to answer", |state| {
			state
				.call(dm.channel_id)
				.is_some_and(|call| call.has(friend) && !call.rings(friend))
				&& state
					.voice()
					.is_some_and(|session| session.peers.contains(&friend))
		})
		.await;
	mock(&voice).call_leave_as(dm.channel_id.get(), friend.get());
	voice
		.running
		.until("the friend to hang up", |state| {
			state.call(dm.channel_id).is_some_and(|call| !call.has(friend))
		})
		.await;
	tokio::time::sleep(NO_LOOP_WINDOW).await;
	let still_connected = voice
		.running
		.session()
		.state()
		.read()
		.voice()
		.is_some_and(|session| session.status == VoiceStatus::Connected);
	assert!(still_connected, "the other side leaving keeps you in the call");
	assert_eq!(
		ring_requests(&mock(&voice).recording(), dm.channel_id, "ring").len(),
		1,
		"the call rings once and never again"
	);
	let left = voice.command(Command::LeaveVoice);
	voice
		.op_4(left, "op 4 out of the call", |frame| is_dm_op_4(frame, None))
		.await;
	voice
		.running
		.until("the call to end", |state| state.call(dm.channel_id).is_none())
		.await;
	let message_id = mock(&voice).call(dm.channel_id.get()).map(|call| call.message_id);
	assert!(message_id.is_none(), "mockcord ended the call");
	voice
		.running
		.until("the call message to say how long it lasted", |state| {
			state.messages(dm.channel_id).is_some_and(|cache| {
				cache.ordered.iter().any(|message| {
					message.kind == MessageType::Call
						&& message
							.call
							.as_ref()
							.is_some_and(|call| call.ended_timestamp.is_some())
				})
			})
		})
		.await;
	voice.running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn declining_a_group_call_stops_ringing_for_you_only_and_never_joins() {
	let voice = start("calls-decline").await;
	let group = group_of_three(&voice.running);
	let (caller, other) = (group.recipients[0], group.recipients[1]);
	let since = Instant::now();
	assert!(mock(&voice).ring_me(group.channel_id.get(), caller.get()));
	rung(&voice, group.channel_id).await;
	voice.command(Command::DeclineCall {
		channel_id: group.channel_id,
	});
	not_rung(&voice, group.channel_id, "the popup to go").await;
	let stops = voice
		.running
		.until_wire("stop ringing", |recording| {
			let stops = ring_requests(recording, group.channel_id, "stop-ringing");
			(!stops.is_empty()).then_some(stops)
		})
		.await;
	assert_eq!(stops, vec![json!({ "recipients": null })]);
	voice
		.running
		.eventually("mockcord to stop ringing you", || {
			mock(&voice)
				.call(group.channel_id.get())
				.is_some_and(|call| !call.ringing.contains(&OWN_ID) && call.ringing.contains(&other.get()))
		})
		.await;
	voice
		.running
		.until("the others still ring", |state| {
			state.call(group.channel_id).is_some_and(|call| call.rings(other))
		})
		.await;
	tokio::time::sleep(NO_LOOP_WINDOW).await;
	let recording = mock(&voice).recording();
	assert_eq!(op_4_count(&recording, since), 0, "declining never joins");
	assert!(ring_requests(&recording, group.channel_id, "ring").is_empty());
	assert!(voice.running.session().state().read().voice().is_none());
	voice.running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ring_stops_when_answered_elsewhere_when_the_caller_hangs_up_and_after_a_while() {
	let voice = start("calls-ring-stops").await;
	let since = Instant::now();
	let elsewhere = direct(&voice.running, 0);
	assert!(mock(&voice).ring_me(elsewhere.channel_id.get(), elsewhere.recipients[0].get()));
	rung(&voice, elsewhere.channel_id).await;
	mock(&voice).answer_elsewhere(elsewhere.channel_id.get());
	not_rung(
		&voice,
		elsewhere.channel_id,
		"answering elsewhere to stop the ring",
	)
	.await;

	let hung_up = direct(&voice.running, 1);
	let caller = hung_up.recipients[0];
	assert!(mock(&voice).ring_me(hung_up.channel_id.get(), caller.get()));
	rung(&voice, hung_up.channel_id).await;
	mock(&voice).call_leave_as(hung_up.channel_id.get(), caller.get());
	voice
		.running
		.until("the caller hanging up to end the call", |state| {
			state.call(hung_up.channel_id).is_none()
		})
		.await;

	let ignored = direct(&voice.running, 2);
	assert!(mock(&voice).ring_me(ignored.channel_id.get(), ignored.recipients[0].get()));
	rung(&voice, ignored.channel_id).await;
	let rang = Instant::now();
	not_rung(&voice, ignored.channel_id, "the ring to time out").await;
	assert!(
		rang.elapsed() >= RING_TIMEOUT - SETTLE,
		"the ring lasts its timeout"
	);
	assert!(
		voice
			.running
			.session()
			.state()
			.read()
			.call(ignored.channel_id)
			.is_some(),
		"the call itself goes on"
	);
	tokio::time::sleep(NO_LOOP_WINDOW).await;
	let recording = mock(&voice).recording();
	assert_eq!(op_4_count(&recording, since), 0, "a ring never joins by itself");
	for channel in [&elsewhere, &hung_up, &ignored] {
		assert!(ring_requests(&recording, channel.channel_id, "stop-ringing").is_empty());
		assert!(ring_requests(&recording, channel.channel_id, "ring").is_empty());
	}
	voice.running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepting_a_group_call_with_video_joins_without_ringing_and_turns_the_camera_on() {
	let voice = start("calls-accept").await;
	let group = group_of_three(&voice.running);
	let (caller, other) = (group.recipients[0], group.recipients[1]);
	assert!(mock(&voice).ring_me(group.channel_id.get(), caller.get()));
	rung(&voice, group.channel_id).await;
	let since = voice.command(Command::StartCall {
		channel_id: group.channel_id,
		video: true,
	});
	not_rung(&voice, group.channel_id, "accepting to close the popup").await;
	voice
		.op_4(since, "op 4 into the group", |frame| {
			is_dm_op_4(frame, Some(group.channel_id))
		})
		.await;
	voice.connected_to(group.channel_id).await;
	voice
		.running
		.until("the camera to come on", |state| {
			state.voice().is_some_and(|session| session.camera_on)
		})
		.await;
	voice
		.op_4(since, "op 4 with the camera on", |frame| {
			is_dm_op_4(frame, Some(group.channel_id)) && frame.data["self_video"] == json!(true)
		})
		.await;
	mock(&voice).call_join_as(group.channel_id.get(), other.get());
	voice
		.running
		.until("all three in the call", |state| {
			state.call(group.channel_id).is_some_and(|call| {
				let mut people: Vec<Snowflake> = call.participants().collect();
				people.sort_unstable();
				let mut wanted = vec![caller, other, Snowflake(OWN_ID)];
				wanted.sort_unstable();
				people == wanted && call.ringing.is_empty()
			})
		})
		.await;
	tokio::time::sleep(SETTLE).await;
	assert!(
		ring_requests(&mock(&voice).recording(), group.channel_id, "ring").is_empty(),
		"accepting a call never rings anyone"
	);
	voice.running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_call_ringing_before_you_connect_arrives_after_ready() {
	let running = Running::start_with(Options::new("calls-auto-connect").prepare(|mock| {
		mock.seed_first_ring();
	}))
	.await;
	running.connected().await;
	let dm = running.fixtures().private_channel_ids[0];
	running
		.until("the earlier call to ring", |state| incoming(state, Snowflake(dm)))
		.await;
	running.stop().await;
}
