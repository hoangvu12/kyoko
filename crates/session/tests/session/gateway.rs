use std::sync::Mutex;
use std::time::{Duration, Instant};

use mockcord::{GatewayFrame, HeartbeatAcks, Recording};
use protocol::{Opcode, Snowflake};
use serde_json::json;
use session::{Backoff, Command, GatewayTuning, SendBudget};
use state::{ConnectionStatus, DisconnectReason};

use crate::harness::{Options, Running};

const NORMAL_CLOSES: [u16; 2] = [1000, 1001];
const AUTHENTICATION_FAILED: u16 = 4004;
const ARRIVAL_SLACK: Duration = Duration::from_millis(50);
const TIMER_SLACK: Duration = Duration::from_millis(5);

const BURST_BUDGET: SendBudget = SendBudget {
	frames: 24,
	period: Duration::from_secs(1),
};
const BURST_HEARTBEAT_MS: u64 = 450;
const BURST_FRAMES: usize = 30;
const BURST_IDS_PER_FRAME: usize = 100;
const BURST_USER_BASE: u64 = 990_000_000_000_200_000;

const JITTER_HEARTBEAT_MS: u64 = 250;
const JITTER_CONNECTIONS: usize = 4;
const JITTER_CEILING: f64 = 0.9;

const ZOMBIE_HEARTBEAT_MS: u64 = 150;
const MIN_IDENTIFY_DELAY: Duration = Duration::from_secs(1);

const QUICK_BACKOFF: Backoff = Backoff {
	base: Duration::from_millis(10),
	cap: Duration::from_millis(40),
};
const FATAL_QUIET: Duration = Duration::from_millis(250);

const GROWING_BACKOFF: Backoff = Backoff {
	base: Duration::from_millis(40),
	cap: Duration::from_millis(160),
};
const REFUSALS: usize = 6;
const RESUMABLE_CLOSE: u16 = 4000;
const LOWEST_JITTER: f64 = 0.75;
const HIGHEST_JITTER: f64 = 1.25;
const FIRST_CAPPED_ATTEMPT: u32 = 5;

const LARGE_READY_BYTES: usize = 20 * 1024 * 1024;
const READY_MARKER: &str = "zlib marker";
const LATE_CHANNEL_ID: u64 = 990_000_000_000_000_007;
const SPLIT_PARTS: usize = 3;
const OVERSIZED_MESSAGE_BYTES: usize = 64 * 1024 * 1024 + 1;
const MESSAGE_TOO_BIG: u16 = 1009;

fn tuned(backoff: Backoff) -> GatewayTuning {
	GatewayTuning {
		backoff,
		..GatewayTuning::default()
	}
}

fn frames_on(recording: &Recording, connection: u64) -> Vec<GatewayFrame> {
	recording
		.frames
		.iter()
		.filter(|frame| frame.connection == connection)
		.cloned()
		.collect()
}

fn heartbeats_on(recording: &Recording, connection: u64) -> Vec<Instant> {
	recording
		.frames_with(Opcode::Heartbeat)
		.filter(|frame| frame.connection == connection)
		.map(|frame| frame.at)
		.collect()
}

fn opened_at(recording: &Recording, connection: u64) -> Instant {
	recording
		.connections
		.iter()
		.find(|opened| opened.id == connection)
		.expect("the connection was recorded")
		.at
}

async fn resume_after(running: &Running, previous: u64) -> GatewayFrame {
	running
		.until_wire("a RESUME on a new connection", |recording| {
			recording
				.frames_with(Opcode::Resume)
				.find(|frame| frame.connection > previous)
				.cloned()
		})
		.await
}

fn closed_abnormally(recording: &Recording, connection: u64) -> bool {
	recording.closes.iter().any(|close| {
		close.connection == connection && close.code.is_some_and(|code| !NORMAL_CLOSES.contains(&code))
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bulk_burst_stays_inside_the_send_window_while_heartbeats_flow() {
	let tuning = GatewayTuning {
		send_budget: BURST_BUDGET,
		..GatewayTuning::default()
	};
	let running = Running::start_with(
		Options::new("send-window")
			.heartbeat_ms(BURST_HEARTBEAT_MS)
			.gateway(tuning),
	)
	.await;
	running.connected().await;
	let connection = running.identified_connection().await;
	let guilds: Vec<Snowflake> = running
		.fixtures()
		.guilds
		.iter()
		.map(|guild| Snowflake(guild.id))
		.collect();
	let burst_began = Instant::now();
	for index in 0..BURST_FRAMES {
		let first = BURST_USER_BASE + (index * BURST_IDS_PER_FRAME) as u64;
		running.session().send(Command::RequestMembers {
			guild_id: guilds[index % guilds.len()],
			user_ids: (first..first + BURST_IDS_PER_FRAME as u64)
				.map(Snowflake)
				.collect(),
		});
	}
	let recording = running
		.until_wire("every bulk frame", |recording| {
			(recording.frames_with(Opcode::RequestGuildMembers).count() >= BURST_FRAMES)
				.then(|| recording.clone())
		})
		.await;
	assert_eq!(recording.connections.len(), 1);
	assert!(recording.closes.is_empty());
	let frames = frames_on(&recording, connection);
	let is_bulk = |frame: &GatewayFrame| frame.opcode() == Opcode::RequestGuildMembers;
	let first_bulk = frames.iter().position(is_bulk).expect("a bulk frame");
	let last_bulk = frames.iter().rposition(is_bulk).expect("a bulk frame");
	let sent_during_burst = &frames[first_bulk..];
	assert!(sent_during_burst.len() > BURST_BUDGET.frames as usize);
	for (index, frame) in sent_during_burst.iter().enumerate() {
		let full_windows = (index / BURST_BUDGET.frames as usize) as u32;
		assert!(
			frame.at.duration_since(burst_began) >= BURST_BUDGET.period * full_windows,
			"frame {index} of the burst broke the send window"
		);
	}
	assert!(frames[last_bulk].at.duration_since(burst_began) >= BURST_BUDGET.period);
	let beats_during_burst = frames[first_bulk..last_bulk]
		.iter()
		.filter(|frame| frame.opcode() == Opcode::Heartbeat)
		.count();
	assert!(beats_during_burst >= 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_first_heartbeat_is_jittered_and_later_beats_keep_the_interval() {
	let running =
		Running::start_with(Options::new("heartbeat-jitter").heartbeat_ms(JITTER_HEARTBEAT_MS)).await;
	running.connected().await;
	let interval = Duration::from_millis(JITTER_HEARTBEAT_MS);
	let mut connection = running.identified_connection().await;
	let mut offsets = Vec::new();
	for round in 0..JITTER_CONNECTIONS {
		let recording = running
			.until_wire("a heartbeat on the newest connection", |recording| {
				(!heartbeats_on(recording, connection).is_empty()).then(|| recording.clone())
			})
			.await;
		offsets
			.push(heartbeats_on(&recording, connection)[0].duration_since(opened_at(&recording, connection)));
		if round + 1 == JITTER_CONNECTIONS {
			break;
		}
		running.mock().request_reconnect();
		connection = resume_after(&running, connection).await.connection;
	}
	assert!(
		offsets
			.iter()
			.any(|offset| *offset < interval.mul_f64(JITTER_CEILING)),
		"no first heartbeat came early: {offsets:?}"
	);
	let beats = running
		.until_wire("two more heartbeats on the last connection", |recording| {
			let beats = heartbeats_on(recording, connection);
			(beats.len() >= 3).then_some(beats)
		})
		.await;
	for pair in beats.windows(2) {
		assert!(pair[1].duration_since(pair[0]) >= interval - ARRIVAL_SLACK);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_heartbeat_ack_closes_and_resumes_on_a_new_connection() {
	let running = Running::start_with(Options::new("zombie").heartbeat_ms(ZOMBIE_HEARTBEAT_MS)).await;
	running.connected().await;
	let first = running.identified_connection().await;
	running.mock().set_heartbeat_acks(HeartbeatAcks::Withheld);
	let resume = resume_after(&running, first).await;
	running.mock().set_heartbeat_acks(HeartbeatAcks::Prompt);
	assert!(resume.data["session_id"].is_string());
	let recording = running.mock().recording();
	assert!(closed_abnormally(&recording, first));
	let last_beat = *heartbeats_on(&recording, first)
		.last()
		.expect("heartbeats on the zombie connection");
	let close = recording
		.closes
		.iter()
		.find(|close| close.connection == first)
		.expect("the client closed the zombie connection");
	assert!(close.at.duration_since(last_beat) >= Duration::from_millis(ZOMBIE_HEARTBEAT_MS) - ARRIVAL_SLACK);
	assert_eq!(recording.frames_with(Opcode::Identify).count(), 1);
	running
		.until("the resumed session", |state| state.session().connected)
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_non_resumable_invalid_session_identifies_again_after_a_pause() {
	let running = Running::start("invalid-session").await;
	running.connected().await;
	let first = running.identified_connection().await;
	let old_session = running.session().state().read().session().session_id.clone();
	let invalidated = Instant::now();
	running.mock().invalidate_session(false);
	let identify = running
		.until_wire("a second IDENTIFY", |recording| {
			recording.frames_with(Opcode::Identify).nth(1).cloned()
		})
		.await;
	assert_eq!(identify.connection, first);
	assert!(identify.at.duration_since(invalidated) >= MIN_IDENTIFY_DELAY);
	running
		.until("a fresh session", |state| {
			state.session().connected && state.session().session_id != old_session
		})
		.await;
	assert_eq!(running.mock().recording().frames_with(Opcode::Resume).count(), 0);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resumable_invalid_session_resumes() {
	let running = Running::start("invalid-resumable").await;
	running.connected().await;
	let first = running.identified_connection().await;
	running.mock().invalidate_session(true);
	let resume = resume_after(&running, first).await;
	assert!(resume.data["seq"].as_u64().is_some_and(|seq| seq > 0));
	let recording = running.mock().recording();
	assert_eq!(recording.frames_with(Opcode::Identify).count(), 1);
	assert!(closed_abnormally(&recording, first));
	running
		.until("the resumed session", |state| state.session().connected)
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reconnect_request_resumes_on_a_new_connection() {
	let running = Running::start("reconnect-op").await;
	running.connected().await;
	let first = running.identified_connection().await;
	running.mock().request_reconnect();
	resume_after(&running, first).await;
	let recording = running.mock().recording();
	assert_eq!(recording.frames_with(Opcode::Identify).count(), 1);
	assert!(closed_abnormally(&recording, first));
	running
		.until("the resumed session", |state| state.session().connected)
		.await;
	running.stop().await;
}

async fn stops_on_fatal_close(name: &'static str, code: u16) {
	let running = Running::start_with(Options::new(name).gateway(tuned(QUICK_BACKOFF))).await;
	running.connected().await;
	let connections = running.mock().recording().connections.len();
	running.mock().close_gateway(code);
	running
		.until("a stop reason", |state| {
			matches!(state.session().connection, ConnectionStatus::Disconnected(_))
		})
		.await;
	tokio::time::sleep(FATAL_QUIET).await;
	let status = running.session().state().read().session().connection.clone();
	let ConnectionStatus::Disconnected(reason) = status else {
		panic!("the session left the stopped state: {status:?}");
	};
	if code == AUTHENTICATION_FAILED {
		assert_eq!(reason, DisconnectReason::AuthenticationFailed);
	} else {
		assert!(
			matches!(&reason, DisconnectReason::Rejected { code: Some(rejected), message }
				if *rejected == code && !message.is_empty()),
			"unexpected reason {reason:?}"
		);
	}
	assert!(!running.session().state().read().session().connected);
	assert_eq!(running.mock().recording().connections.len(), connections);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4004_stops_as_an_authentication_failure() {
	stops_on_fatal_close("fatal-4004", 4004).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4010_stops_reconnecting() {
	stops_on_fatal_close("fatal-4010", 4010).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4011_stops_reconnecting() {
	stops_on_fatal_close("fatal-4011", 4011).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4012_stops_reconnecting() {
	stops_on_fatal_close("fatal-4012", 4012).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4013_stops_reconnecting() {
	stops_on_fatal_close("fatal-4013", 4013).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4014_stops_reconnecting() {
	stops_on_fatal_close("fatal-4014", 4014).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4015_stops_reconnecting() {
	stops_on_fatal_close("fatal-4015", 4015).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4016_stops_reconnecting() {
	stops_on_fatal_close("fatal-4016", 4016).await;
}

fn shortest_backoff(failures: u32) -> Duration {
	let uncapped = GROWING_BACKOFF
		.base
		.mul_f64(LOWEST_JITTER * 2f64.powi(failures as i32 - 1));
	uncapped.min(GROWING_BACKOFF.cap)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_drops_back_off_longer_up_to_the_cap_then_reset() {
	let running = Running::start_with(Options::new("backoff").gateway(tuned(GROWING_BACKOFF))).await;
	running.connected().await;
	let before = running.mock().recording().connections.len();
	let observed: Mutex<Vec<(u32, Duration)>> = Mutex::new(Vec::new());
	running.mock().close_next_connections(RESUMABLE_CLOSE, REFUSALS);
	let dropped = Instant::now();
	running.mock().close_gateway(RESUMABLE_CLOSE);
	running
		.until("the session to resume after the refusals", |state| {
			if let ConnectionStatus::Reconnecting { attempt, retry_in } = state.session().connection {
				let mut observed = observed.lock().expect("observations");
				if observed.last() != Some(&(attempt, retry_in)) {
					observed.push((attempt, retry_in));
				}
			}
			state.session().connected && running.mock().recording().connections.len() > before + REFUSALS
		})
		.await;
	let recording = running.mock().recording();
	let opened: Vec<Instant> = recording.connections[before..]
		.iter()
		.map(|connection| connection.at)
		.collect();
	assert_eq!(opened.len(), REFUSALS + 1);
	assert!(opened[0].duration_since(dropped) >= shortest_backoff(1) - TIMER_SLACK);
	for (index, pair) in opened.windows(2).enumerate() {
		let failures = index as u32 + 2;
		assert!(
			pair[1].duration_since(pair[0]) >= shortest_backoff(failures) - TIMER_SLACK,
			"retry {failures} came too soon"
		);
	}
	let observed = observed.into_inner().expect("observations");
	assert!(
		observed
			.iter()
			.any(|(attempt, _)| *attempt >= FIRST_CAPPED_ATTEMPT),
		"never saw a capped attempt: {observed:?}"
	);
	assert!(
		observed
			.iter()
			.all(|(_, retry_in)| *retry_in <= GROWING_BACKOFF.cap)
	);
	let last = recording.connections[recording.connections.len() - 1].id;
	assert!(
		recording
			.frames_with(Opcode::Resume)
			.any(|frame| frame.connection == last)
	);
	running.mock().close_gateway(RESUMABLE_CLOSE);
	running
		.until("a fresh first retry", |state| {
			matches!(
				state.session().connection,
				ConnectionStatus::Reconnecting { attempt: 2, retry_in }
					if retry_in <= GROWING_BACKOFF.base.mul_f64(HIGHEST_JITTER)
			)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_large_ready_arrives_over_zlib_stream() {
	let options = Options::new("zlib-large-ready").prepare(|mock| {
		mock.transform_ready(|_, ready| {
			ready["user"]["global_name"] = json!(READY_MARKER);
			ready["kyoko_padding"] = json!("kyoko ".repeat(LARGE_READY_BYTES / 6));
		});
	});
	let running = Running::start_with(options).await;
	running
		.until("the large READY", |state| {
			state.session().connected
				&& state
					.current_user()
					.is_some_and(|user| user.global_name.as_deref() == Some(READY_MARKER))
		})
		.await;
	let connection = running.identified_connection().await;
	let recording = running.mock().recording();
	let query = recording
		.connections
		.iter()
		.find(|opened| opened.id == connection)
		.and_then(|opened| opened.query.clone())
		.expect("the connection query was recorded");
	assert!(query.contains("compress=zlib-stream"));
	let guild_id = running.fixtures().guilds[0].id;
	running.mock().dispatch(
		"CHANNEL_CREATE",
		&json!({
			"id": LATE_CHANNEL_ID.to_string(),
			"type": 0,
			"guild_id": guild_id.to_string(),
			"name": "after-the-large-ready",
			"position": 99,
			"permission_overwrites": [],
		}),
	);
	running
		.until("a dispatch after the large READY", |state| {
			state
				.guild(Snowflake(guild_id))
				.is_some_and(|guild| guild.channels.contains_key(&Snowflake(LATE_CHANNEL_ID)))
		})
		.await;
	assert_eq!(running.mock().recording().connections.len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zlib_stream_messages_split_across_websocket_messages_still_inflate() {
	let options = Options::new("zlib-split").prepare(|mock| {
		mock.split_gateway_messages(SPLIT_PARTS);
		mock.transform_ready(|_, ready| {
			ready["user"]["global_name"] = json!(READY_MARKER);
		});
	});
	let running = Running::start_with(options).await;
	running
		.until("a READY split across messages", |state| {
			state.session().connected
				&& state
					.current_user()
					.is_some_and(|user| user.global_name.as_deref() == Some(READY_MARKER))
		})
		.await;
	let guild_id = running.fixtures().guilds[0].id;
	running.mock().dispatch(
		"CHANNEL_CREATE",
		&json!({
			"id": LATE_CHANNEL_ID.to_string(),
			"type": 0,
			"guild_id": guild_id.to_string(),
			"name": "split-across-messages",
			"position": 99,
			"permission_overwrites": [],
		}),
	);
	running
		.until("a split dispatch after READY", |state| {
			state
				.guild(Snowflake(guild_id))
				.is_some_and(|guild| guild.channels.contains_key(&Snowflake(LATE_CHANNEL_ID)))
		})
		.await;
	assert_eq!(running.mock().recording().connections.len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_websocket_message_over_the_cap_stops_the_session_as_too_big() {
	let running = Running::start("gateway-oversized-message").await;
	running.connected().await;
	running
		.mock()
		.send_uncompressed_gateway_text(" ".repeat(OVERSIZED_MESSAGE_BYTES));
	running
		.until("a stop reason", |state| {
			matches!(state.session().connection, ConnectionStatus::Disconnected(_))
		})
		.await;
	let status = running.session().state().read().session().connection.clone();
	assert!(
		matches!(
			&status,
			ConnectionStatus::Disconnected(DisconnectReason::Rejected {
				code: Some(MESSAGE_TOO_BIG),
				..
			})
		),
		"unexpected status {status:?}"
	);
	tokio::time::sleep(FATAL_QUIET).await;
	assert_eq!(running.mock().recording().connections.len(), 1);
	running.stop().await;
}
