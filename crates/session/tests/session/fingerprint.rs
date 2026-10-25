use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mockcord::{DEVICE_COOKIE, MockConfig, Recording, RestRequest, device_cookie_value, installation_id};
use parking_lot::Mutex;
use protocol::fingerprint::{API_VERSION, CHROME_MAJOR};
use protocol::{Opcode, Snowflake};
use serde_json::Value;
use session::Command;

use crate::harness::{LOCALE, Options, Running};

const PROPERTY_FIELDS: [&str; 18] = [
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
	let identifies = identify_properties(&recording);
	assert_eq!(identifies.len(), 1);
	let properties = &identifies[0];
	let mut fields: Vec<&str> = properties
		.as_object()
		.expect("properties are an object")
		.keys()
		.map(String::as_str)
		.collect();
	fields.sort_unstable();
	let mut expected = PROPERTY_FIELDS.to_vec();
	expected.sort_unstable();
	assert_eq!(fields, expected);
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
		assert!(
			request
				.header("sec-ch-ua")
				.is_some_and(|brands| brands.contains(&format!("\"Google Chrome\";v=\"{CHROME_MAJOR}\""))),
			"{what}"
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
