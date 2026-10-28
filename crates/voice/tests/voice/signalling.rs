use std::time::Duration;

use mockcord::hello::{
	Capture, NameCase, TlsSummary, chrome_fixture, differences, missing_names, order_difference,
};
use mockcord::{VOICE_TOKEN, VoiceRecording};
use protocol::fingerprint::{ACCEPT_ENCODING, CHROME_MAJOR, WEB_ORIGIN, accept_language};
use serde_json::json;

use crate::harness::{CHANNEL_ID, Call, GUILD_ID, LOCALE, SESSION_ID, USER_ID, header};

const IDENTIFY: i64 = 0;
const SELECT_PROTOCOL: i64 = 1;
const HEARTBEAT: i64 = 3;
const HELLO: i64 = 8;
const UNOFFERED_EXTENSIONS: &str = "Sec-WebSocket-Extensions";
const WEB_HEARTBEAT_CAP: Duration = Duration::from_millis(5_000);
const SHORT_INTERVAL_MS: u64 = 300;

fn heartbeat_gaps(recording: &VoiceRecording) -> Vec<Duration> {
	let beats: Vec<_> = recording.frames_with(HEARTBEAT).map(|frame| frame.at).collect();
	beats.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_voice_upgrade_presents_chromes_websocket_client_hello_and_headers() {
	let call = Call::start(|_| {}).await;
	let recording = call
		.until("an upgrade on the voice socket", |recording| {
			recording.sockets.iter().any(|socket| socket.upgrade.is_some())
		})
		.await;
	let socket = recording
		.sockets
		.iter()
		.find(|socket| socket.upgrade.is_some())
		.expect("an upgraded socket");
	assert_eq!(socket.alpn.as_deref(), Some("http/1.1"));
	let expected = chrome_fixture(CHROME_MAJOR, "wss").expect("a recorded fixture for CHROME_MAJOR");
	let actual = Capture {
		client: "kyoko".to_owned(),
		kind: "wss".to_owned(),
		negotiated_alpn: None,
		tls: TlsSummary::of(socket.hello.as_ref().expect("a client hello")),
		http2: None,
		http1: None,
	};
	let found = differences(&expected, &actual);
	assert!(
		found.is_empty(),
		"the voice client hello differs from chrome {CHROME_MAJOR}: {found:?}"
	);
	let upgrade = socket.upgrade.as_ref().expect("the upgrade request");
	assert!(
		upgrade.request_line.starts_with("GET /?v=9 "),
		"{}",
		upgrade.request_line
	);
	let chrome = chrome_fixture(CHROME_MAJOR, "wss-cross-site")
		.expect("a recorded cross-site upgrade for CHROME_MAJOR")
		.http1
		.expect("the fixture recorded the upgrade");
	let headers = &upgrade.headers;
	let difference = order_difference(&chrome.headers, headers, NameCase::Exact);
	assert!(
		difference.is_empty(),
		"the upgrade differs from chrome's: {difference:?}"
	);
	assert_eq!(headers.first().map(|(name, _)| name.as_str()), Some("Host"));
	assert_eq!(
		missing_names(&chrome.headers, headers, NameCase::Exact),
		[UNOFFERED_EXTENSIONS]
	);
	assert_eq!(header(headers, "Cookie"), None);
	assert_eq!(header(headers, "Origin"), Some(WEB_ORIGIN));
	assert_eq!(
		header(headers, "Accept-Language"),
		Some(accept_language(LOCALE).as_str())
	);
	assert_eq!(header(headers, "Accept-Encoding"), Some(ACCEPT_ENCODING));
	for name in [
		"Pragma",
		"Cache-Control",
		"Upgrade",
		"Sec-WebSocket-Version",
		"User-Agent",
	] {
		assert_eq!(header(headers, name), header(&chrome.headers, name), "{name}");
	}
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identify_is_v9_with_the_channel_two_layers_and_dave_1() {
	let call = Call::start(|_| {}).await;
	let recording = call
		.until("select protocol", |recording| {
			recording.frames_with(SELECT_PROTOCOL).next().is_some()
		})
		.await;
	let identify = recording.frames_with(IDENTIFY).next().expect("an identify");
	assert_eq!(
		identify.keys(&["d"]),
		[
			"server_id",
			"channel_id",
			"user_id",
			"session_id",
			"token",
			"video",
			"streams",
			"max_dave_protocol_version"
		]
	);
	assert_eq!(
		identify.data,
		json!({
			"server_id": GUILD_ID.to_string(),
			"channel_id": CHANNEL_ID.to_string(),
			"user_id": USER_ID.to_string(),
			"session_id": SESSION_ID,
			"token": VOICE_TOKEN,
			"video": true,
			"streams": [
				{ "type": "video", "rid": "100", "quality": 100 },
				{ "type": "video", "rid": "50", "quality": 50 },
			],
			"max_dave_protocol_version": 1,
		})
	);
	let select = recording
		.frames_with(SELECT_PROTOCOL)
		.next()
		.expect("a select protocol");
	assert_eq!(
		select.keys(&["d"]),
		["protocol", "data", "codecs", "rtc_connection_id", "experiments"]
	);
	assert_eq!(select.data["protocol"], "udp");
	assert_eq!(select.keys(&["d", "data"]), ["address", "port", "mode"]);
	let connection_id = select.data["rtc_connection_id"]
		.as_str()
		.expect("an rtc connection id");
	assert_eq!(
		uuid_shape(connection_id),
		[8, 4, 4, 4, 12],
		"{connection_id} is a uuid"
	);
	assert_eq!(select.data["experiments"], json!(["fixed_keyframe_interval"]));
	assert_eq!(select.data["codecs"][0]["name"], "opus");
	assert_eq!(select.data["codecs"][0]["payload_type"], 120);
	call.stop().await;
}

fn uuid_shape(text: &str) -> Vec<usize> {
	text.split('-').map(str::len).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_heartbeat_follows_the_web_client_and_caps_at_five_seconds() {
	let call = Call::start(|_| {}).await;
	let recording = call
		.until("two heartbeats", |recording| {
			recording.frames_with(HEARTBEAT).count() >= 2
		})
		.await;
	let hello = recording.sent_with(HELLO).next().expect("a hello");
	assert_eq!(hello.data["heartbeat_interval"], 41_250);
	let first = recording.frames_with(HEARTBEAT).next().expect("a heartbeat");
	assert_eq!(first.keys(&["d"]), ["t", "seq_ack"]);
	assert_eq!(first.data["seq_ack"], -1);
	let after_hello = first.at - hello.at;
	assert!(
		after_hello >= WEB_HEARTBEAT_CAP - Duration::from_millis(100),
		"{after_hello:?}"
	);
	let gap = heartbeat_gaps(&recording)[0];
	assert!(gap >= WEB_HEARTBEAT_CAP - Duration::from_millis(100), "{gap:?}");
	assert!(gap < WEB_HEARTBEAT_CAP * 2, "{gap:?}");
	call.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shorter_heartbeat_interval_is_used_as_is() {
	let call = Call::start(|control| control.set_heartbeat_interval(SHORT_INTERVAL_MS)).await;
	let recording = call
		.until("four heartbeats", |recording| {
			recording.frames_with(HEARTBEAT).count() >= 4
		})
		.await;
	for gap in heartbeat_gaps(&recording) {
		assert!(gap >= Duration::from_millis(SHORT_INTERVAL_MS - 50), "{gap:?}");
		assert!(gap < WEB_HEARTBEAT_CAP, "{gap:?}");
	}
	call.stop().await;
}
