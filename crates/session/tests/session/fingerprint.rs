use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mockcord::{
	API_CODE_VERSION, DEVICE_COOKIE, GatewayFrame, MockConfig, Recording, RestRequest, device_cookie_value,
	installation_id,
};
use parking_lot::Mutex;
use protocol::fingerprint::{
	API_VERSION, CHROME_MAJOR, Clock, HEARTBEAT_SESSION_LIFETIME, LAUNCH_SIGNATURE_MOD_BITS,
};
use protocol::{Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;

use crate::harness::{LOCALE, Options, Running};

const IDENTIFY_FIELDS: [&str; 6] = [
	"token",
	"properties",
	"compress",
	"presence",
	"capabilities",
	"client_state",
];
const PRESENCE_FIELDS: [&str; 4] = ["activities", "status", "since", "afk"];
const CHROME_156_BRANDS: &str =
	"\"Not:A-Brand\";v=\"8\", \"Chromium\";v=\"156\", \"Google Chrome\";v=\"156\"";
const NEWER_API_CODE_VERSION: i64 = 7;
const PROPERTY_FIELDS: [&str; 19] = [
	"os",
	"browser",
	"device",
	"system_locale",
	"browser_user_agent",
	"browser_version",
	"os_version",
	"referrer",
	"referring_domain",
	"referrer_current",
	"referring_domain_current",
	"release_channel",
	"client_build_number",
	"client_event_source",
	"has_client_mods",
	"client_launch_id",
	"launch_signature",
	"client_app_state",
	"client_heartbeat_session_id",
];
const ROTATED_INSTALLATION: &str = "rotated-installation.0000";
const MOCK_SEED: u64 = 1;

fn super_properties(request: &RestRequest) -> Value {
	let encoded = request
		.header("x-super-properties")
		.expect("every request carries super properties");
	let decoded = STANDARD.decode(encoded).expect("super properties are base64");
	serde_json::from_slice(&decoded).expect("super properties are json")
}

fn identify_properties(recording: &Recording) -> Vec<Value> {
	recording
		.frames_with(Opcode::Identify)
		.map(|frame| frame.data["properties"].clone())
		.collect()
}

fn identifies(recording: &Recording) -> Vec<GatewayFrame> {
	recording.frames_with(Opcode::Identify).cloned().collect()
}

async fn nth_identify(running: &Running, index: usize) -> GatewayFrame {
	running
		.until_wire("the expected IDENTIFY", |recording| {
			identifies(recording).get(index).cloned()
		})
		.await
}

fn without_installation_id(properties: &Value) -> Value {
	let mut shared = properties.clone();
	if let Some(fields) = shared.as_object_mut() {
		fields.remove("installation_id");
	}
	shared
}

fn manual_clock() -> (Clock, Arc<Mutex<Duration>>) {
	let start = Instant::now();
	let elapsed = Arc::new(Mutex::new(Duration::ZERO));
	let reading = elapsed.clone();
	(Clock::new(move || start + *reading.lock()), elapsed)
}

async fn message_request(running: &Running, channel_id: Snowflake, content: &str) -> RestRequest {
	running.send_and_wait(channel_id, content, content).await;
	let path = format!("/channels/{channel_id}/messages");
	running
		.mock()
		.recording()
		.requests_to("POST", &path)
		.find(|request| request.json().is_some_and(|body| body["content"] == content))
		.cloned()
		.expect("the message request was recorded")
}

fn device_cookie(request: &RestRequest) -> Option<String> {
	request.header("cookie")?.split(';').find_map(|pair| {
		pair.trim()
			.strip_prefix(&format!("{DEVICE_COOKIE}="))
			.map(str::to_owned)
	})
}

fn history_request(recording: &Recording, channel_id: Snowflake) -> Option<RestRequest> {
	recording
		.requests_to("GET", &format!("/channels/{channel_id}/messages"))
		.next()
		.cloned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identify_and_rest_present_one_web_client_identity() {
	let running = Running::start("fingerprint-identity").await;
	running.open_text_channel().await;
	let recording = running.mock().recording();
	let frames = identifies(&recording);
	assert_eq!(frames.len(), 1);
	let properties = &frames[0].data["properties"];
	assert_eq!(
		frames[0].keys(&["properties"]),
		PROPERTY_FIELDS,
		"the web client's property order, no installation id before one is known"
	);
	let major = CHROME_MAJOR.to_string();
	let user_agent = properties["browser_user_agent"].as_str().expect("user agent");
	assert!(user_agent.starts_with("Mozilla/5.0 ("));
	assert!(user_agent.ends_with(&format!("Chrome/{major}.0.0.0 Safari/537.36")));
	assert_eq!(properties["browser_version"], format!("{major}.0.0.0"));
	assert_eq!(properties["browser"], "Chrome");
	assert_eq!(properties["system_locale"], LOCALE);
	assert_eq!(properties["release_channel"], "stable");
	assert_eq!(properties["has_client_mods"], false);
	assert!(properties["client_event_source"].is_null());
	assert_eq!(
		properties["client_build_number"],
		MockConfig::default().build_number,
		"the build number scraped from the app assets"
	);
	assert_eq!(properties["client_launch_id"].as_str().map(str::len), Some(36));
	assert_eq!(
		properties["client_heartbeat_session_id"].as_str().map(str::len),
		Some(36)
	);
	let connection = recording.connections.first().expect("a gateway connection");
	assert_eq!(connection.header("user-agent"), Some(user_agent));
	assert!(!recording.requests.is_empty());
	for request in &recording.requests {
		assert_eq!(
			&super_properties(request),
			properties,
			"{} {}",
			request.method,
			request.path
		);
		assert_eq!(request.header("user-agent"), Some(user_agent));
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_and_gateway_use_api_v9() {
	let ready_version = Arc::new(Mutex::new(None));
	let seen = ready_version.clone();
	let running = Running::start_with(Options::new("fingerprint-v9").prepare(move |mock| {
		mock.transform_ready(move |_, ready| *seen.lock() = ready["v"].as_i64());
	}))
	.await;
	running.open_text_channel().await;
	let recording = running.mock().recording();
	assert!(!recording.requests.is_empty());
	assert!(
		recording
			.requests
			.iter()
			.all(|request| request.version == API_VERSION)
	);
	let query = recording.connections[0].query.as_deref().unwrap_or_default();
	assert!(query.split('&').any(|pair| pair == "v=9"), "{query}");
	assert_eq!(*ready_version.lock(), Some(i64::from(API_VERSION)));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_requests_carry_the_web_client_header_set() {
	let running = Running::start("fingerprint-headers").await;
	let channel_id = running.open_text_channel().await;
	let (guild_id, dm_id) = {
		let state = running.session().state().read();
		let guild_id = state
			.guilds()
			.find(|guild| guild.channels.contains_key(&channel_id))
			.map(|guild| guild.id)
			.expect("the channel's guild");
		let dm_id = state
			.private_channels()
			.next()
			.map(|channel| channel.id)
			.expect("a private channel in the fixtures");
		(guild_id, dm_id)
	};
	let history = history_request(&running.mock().recording(), channel_id).expect("history was loaded");
	assert_eq!(
		history.header("referer"),
		Some(format!("https://discord.com/channels/{guild_id}/{channel_id}").as_str())
	);
	running.session().send(Command::SelectChannel {
		guild_id: None,
		channel_id: dm_id,
	});
	let dm_history = running
		.until_wire("the direct message history request", |recording| {
			history_request(recording, dm_id)
		})
		.await;
	assert_eq!(
		dm_history.header("referer"),
		Some(format!("https://discord.com/channels/@me/{dm_id}").as_str())
	);
	let recording = running.mock().recording();
	for request in &recording.requests {
		let what = format!("{} {}", request.method, request.path);
		assert!(
			request.header("authorization") == Some(running.token()),
			"{what} carries the token"
		);
		assert_eq!(request.header("x-discord-locale"), Some(LOCALE), "{what}");
		assert_eq!(
			request.header("accept-language"),
			Some("en-US,en;q=0.9"),
			"{what}"
		);
		assert!(
			request
				.header("x-discord-timezone")
				.is_some_and(|zone| !zone.is_empty()),
			"{what} carries the timezone"
		);
		assert_eq!(
			request.header("sec-ch-ua"),
			Some(CHROME_156_BRANDS),
			"{what}: Chrome's brand list for major {CHROME_MAJOR}"
		);
		assert!(request.header("referer").is_some(), "{what}");
		if request.method != "GET" {
			assert_eq!(request.header("origin"), Some("https://discord.com"), "{what}");
		}
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cookies_and_installation_id_survive_a_restart() {
	let mut running = Running::start("fingerprint-restart").await;
	let channel_id = running.open_text_channel().await;
	running
		.send_and_wait(channel_id, "after the first response", "after the first response")
		.await;
	let expected_installation = installation_id(MOCK_SEED);
	let expected_cookie = device_cookie_value(MOCK_SEED);
	running
		.until_wire("a request echoing the cookie and installation id", |recording| {
			recording
				.requests
				.iter()
				.find(|request| {
					device_cookie(request).as_deref() == Some(expected_cookie.as_str())
						&& request.header("x-installation-id") == Some(expected_installation.as_str())
				})
				.cloned()
		})
		.await;
	let first_launch = identify_properties(&running.mock().recording())[0]["client_launch_id"].clone();
	let before_restart = running.mock().recording().requests.len();
	running.mock().transform_ready(|_, ready| {
		ready["apex_experiments"]["installation"] = Value::from(ROTATED_INSTALLATION);
	});
	running.restart().await;
	let channel_id = running.open_text_channel().await;
	let channel_path = format!("/channels/{channel_id}/**");
	let recording = running
		.until_wire("a channel request from the restarted session", |recording| {
			let reached = recording
				.requests
				.iter()
				.skip(before_restart)
				.any(|request| request.is("*", &channel_path));
			reached.then(|| recording.clone())
		})
		.await;
	let after: Vec<_> = recording.requests[before_restart..].iter().collect();
	assert!(!after.is_empty());
	for request in after {
		let what = format!("{} {}", request.method, request.path);
		assert_eq!(
			request.header("x-installation-id"),
			Some(expected_installation.as_str()),
			"{what}"
		);
		assert_eq!(device_cookie(request), Some(expected_cookie.clone()), "{what}");
	}
	let identifies = identify_properties(&recording);
	assert_eq!(identifies.len(), 2);
	assert_ne!(identifies[1]["client_launch_id"], first_launch);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identify_follows_the_web_client_shape() {
	let running = Running::start("fingerprint-identify-shape").await;
	let identify = nth_identify(&running, 0).await;
	assert_eq!(identify.keys(&[]), IDENTIFY_FIELDS);
	assert!(identify.data.get("large_threshold").is_none());
	assert_eq!(identify.data["compress"], false);
	assert_eq!(identify.keys(&["presence"]), PRESENCE_FIELDS);
	assert_eq!(
		identify.data["presence"],
		json!({ "activities": [], "status": "unknown", "since": 0, "afk": false })
	);
	assert_eq!(
		identify.data["client_state"],
		json!({ "guild_versions": {}, "api_code_version": 0 })
	);
	let signature = identify.data["properties"]["launch_signature"]
		.as_str()
		.expect("a launch signature");
	assert_eq!(signature.len(), 36);
	let bits = u128::from_str_radix(&signature.replace('-', ""), 16).expect("the launch signature is a uuid");
	assert_eq!(bits & LAUNCH_SIGNATURE_MOD_BITS, 0);
	assert_eq!(identify.data["properties"]["has_client_mods"], false);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identify_echoes_the_installation_id_and_api_code_version() {
	let mut running = Running::start("fingerprint-identify-echo").await;
	running.connected().await;
	let expected_installation = installation_id(MOCK_SEED);
	running
		.eventually("the installation id to be adopted", || {
			running.store().installation_id().as_deref() == Some(expected_installation.as_str())
		})
		.await;
	running.mock().close_gateway(4009);
	let reidentify = nth_identify(&running, 1).await;
	assert_eq!(
		reidentify.data["client_state"]["api_code_version"],
		API_CODE_VERSION
	);
	assert_eq!(
		reidentify.data["properties"]["installation_id"],
		expected_installation.as_str()
	);
	let mut fields = PROPERTY_FIELDS.to_vec();
	fields.push("installation_id");
	assert_eq!(reidentify.keys(&["properties"]), fields);
	running.connected().await;

	running
		.restart_after(|mock| {
			mock.transform_ready(|_, ready| {
				ready["api_code_version"] = Value::from(NEWER_API_CODE_VERSION);
			});
		})
		.await;
	let next_launch = nth_identify(&running, 2).await;
	assert_eq!(
		next_launch.data["client_state"]["api_code_version"], API_CODE_VERSION,
		"the last READY's value survives the restart"
	);
	assert_eq!(
		next_launch.data["properties"]["installation_id"],
		expected_installation.as_str()
	);
	let channel_id = running.open_text_channel().await;
	let request = message_request(&running, channel_id, "after the restart").await;
	let header = super_properties(&request);
	assert!(header.get("installation_id").is_none());
	assert_eq!(header, without_installation_id(&next_launch.data["properties"]));
	assert_eq!(
		request.header("x-installation-id"),
		Some(expected_installation.as_str())
	);

	running.restart().await;
	let third_launch = nth_identify(&running, 3).await;
	assert_eq!(
		third_launch.data["client_state"]["api_code_version"],
		NEWER_API_CODE_VERSION
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_session_id_rotates_every_thirty_minutes() {
	let (clock, elapsed) = manual_clock();
	let running = Running::start_with(Options::new("fingerprint-heartbeat").clock(clock)).await;
	let channel_id = running.open_text_channel().await;
	let first = nth_identify(&running, 0).await.data["properties"].clone();
	let first_session = first["client_heartbeat_session_id"].clone();

	*elapsed.lock() = HEARTBEAT_SESSION_LIFETIME - Duration::from_secs(1);
	let before = super_properties(&message_request(&running, channel_id, "just before").await);
	assert_eq!(before, first, "the same identity until the session expires");

	*elapsed.lock() = HEARTBEAT_SESSION_LIFETIME;
	let after = super_properties(&message_request(&running, channel_id, "just after").await);
	let second_session = after["client_heartbeat_session_id"].clone();
	assert_ne!(second_session, first_session);
	assert_eq!(second_session.as_str().map(str::len), Some(36));
	assert_eq!(after["client_launch_id"], first["client_launch_id"]);
	assert_eq!(after["launch_signature"], first["launch_signature"]);

	running.mock().close_gateway(4009);
	let reidentify = nth_identify(&running, 1).await;
	assert_eq!(
		reidentify.data["properties"]["client_heartbeat_session_id"], second_session,
		"gateway and REST share the rotated id"
	);
	assert_eq!(without_installation_id(&reidentify.data["properties"]), after);

	*elapsed.lock() = HEARTBEAT_SESSION_LIFETIME * 2 + Duration::from_secs(1);
	let third = super_properties(&message_request(&running, channel_id, "an hour later").await);
	assert_ne!(third["client_heartbeat_session_id"], second_session);
	assert_ne!(third["client_heartbeat_session_id"], first_session);
	running.stop().await;
}
