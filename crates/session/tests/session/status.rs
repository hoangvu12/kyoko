use std::time::Duration;

use chrono::Utc;
use mockcord::{Recording, Reply};
use protocol::gateway::Opcode;
use protocol::{ProtoStatusSettings, Snowflake, StatusKind, decode_status_settings};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const SETTINGS: &str = "/users/@me/settings-proto/1";
const DELAY: Duration = Duration::from_millis(100);
const GAP: Duration = Duration::from_millis(600);

async fn start(name: &'static str) -> Running {
	let tuning = RestTuning {
		mutation_gap: Duration::ZERO,
		settings_delay: DELAY,
		settings_gap: GAP,
		..RestTuning::default()
	};
	let running = Running::start_with(Options::new(name).rest(tuning)).await;
	running.connected().await;
	running
}

fn writes(recording: &Recording) -> Vec<ProtoStatusSettings> {
	recording
		.requests_to("PATCH", SETTINGS)
		.filter_map(|request| decode_status_settings(request.json()?["settings"].as_str()?))
		.collect()
}

fn presences(recording: &Recording) -> Vec<Value> {
	recording
		.frames_with(Opcode::PresenceUpdate)
		.map(|frame| frame.data.clone())
		.collect()
}

async fn settle() {
	tokio::time::sleep(GAP * 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_status_change_shows_at_once_goes_out_as_presence_and_is_saved_in_the_settings_proto() {
	let running = start("status-change").await;
	running.session().send(Command::SetStatus(StatusKind::Dnd));
	running
		.until("the local status", |state: &State| {
			state.own_status() == StatusKind::Dnd
		})
		.await;
	let presence = running
		.until_wire("the presence update", |recording| presences(recording).pop())
		.await;
	assert_eq!(presence["status"], "dnd");
	assert_eq!(presence["since"], 0);
	assert_eq!(presence["afk"], false);
	let saved = running
		.until_wire("the settings write", |recording| writes(recording).pop())
		.await;
	assert_eq!(saved.status.as_deref(), Some("dnd"));
	assert_eq!(saved.custom_status, None);
	running.session().send(Command::SetStatus(StatusKind::Invisible));
	let saved = running
		.until_wire("the second write", |recording| {
			writes(recording).into_iter().nth(1)
		})
		.await;
	assert_eq!(saved.status.as_deref(), Some("invisible"));
	assert_eq!(
		presences(&running.mock().recording())
			.last()
			.map(|data| data["status"].clone()),
		Some(json!("invisible"))
	);
	settle().await;
	let recording = running.mock().recording();
	assert_eq!(writes(&recording).len(), 2);
	assert!(
		recording
			.requests_to("PATCH", "/users/@me/settings")
			.next()
			.is_none()
	);
	assert_eq!(
		running.session().state().read().own_status(),
		StatusKind::Invisible
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_status_write_puts_the_old_status_back_and_says_why() {
	let running = start("status-refused").await;
	running.mock().respond_once(
		"PATCH",
		SETTINGS,
		Reply::json(400, json!({ "code": 50035, "message": "Invalid Form Body" })),
	);
	running.session().send(Command::SetStatus(StatusKind::Idle));
	running
		.until_wire("the refused write", |recording| writes(recording).pop())
		.await;
	running
		.until("the old status back", |state: &State| {
			state.own_status() == StatusKind::Online
				&& state
					.last_rejection()
					.is_some_and(|rejection| rejection.reason.contains("status could not be saved"))
		})
		.await;
	let sent = running
		.until_wire("the presence put back", |recording| {
			let sent = presences(recording);
			(sent.len() >= 2).then_some(sent)
		})
		.await;
	assert_eq!(sent[0]["status"], "idle");
	assert_eq!(sent[sent.len() - 1]["status"], "online");
	settle().await;
	assert_eq!(writes(&running.mock().recording()).len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_custom_status_is_set_with_its_expiry_and_cleared() {
	let running = start("status-custom").await;
	let expires = Utc::now() + chrono::Duration::hours(1);
	running.session().send(Command::SetCustomStatus {
		text: Some("brb coffee".to_owned()),
		emoji: Some("\u{2615}".to_owned()),
		expires: Some(expires),
	});
	running
		.until("the local custom status", |state: &State| {
			state
				.own_custom_status(Utc::now())
				.is_some_and(|custom| custom.text.as_deref() == Some("brb coffee"))
		})
		.await;
	let saved = running
		.until_wire("the custom status write", |recording| writes(recording).pop())
		.await;
	let custom = saved.custom_status.expect("a custom status in the write");
	assert_eq!(custom.text, "brb coffee");
	assert_eq!(custom.emoji_name.as_deref(), Some("\u{2615}"));
	assert_eq!(custom.expires_at_ms, Some(expires.timestamp_millis() as u64));
	let presence = presences(&running.mock().recording())
		.pop()
		.expect("a presence update");
	assert_eq!(presence["activities"][0]["type"], 4);
	assert_eq!(presence["activities"][0]["state"], "brb coffee");
	assert_eq!(presence["activities"][0]["emoji"]["name"], "\u{2615}");

	running.session().send(Command::SetCustomStatus {
		text: None,
		emoji: None,
		expires: None,
	});
	let cleared = running
		.until_wire("the clearing write", |recording| {
			writes(recording).into_iter().nth(1)
		})
		.await;
	assert_eq!(cleared.custom_status, None);
	let presence = presences(&running.mock().recording())
		.pop()
		.expect("a presence update");
	assert_eq!(presence["activities"], json!([]));
	assert!(
		running
			.session()
			.state()
			.read()
			.own_custom_status(Utc::now())
			.is_none()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_custom_status_drops_from_presence_without_another_write() {
	let running = start("status-expiry").await;
	running.session().send(Command::SetCustomStatus {
		text: Some("short".to_owned()),
		emoji: None,
		expires: Some(Utc::now() + chrono::Duration::milliseconds(1500)),
	});
	running
		.until_wire("the custom status write", |recording| writes(recording).pop())
		.await;
	let presence = running
		.until_wire("the presence without the custom status", |recording| {
			presences(recording)
				.pop()
				.filter(|presence| presence["activities"] == json!([]))
		})
		.await;
	assert_eq!(presence["status"], "online");
	assert!(
		running
			.session()
			.state()
			.read()
			.own_custom_status(Utc::now())
			.is_none()
	);
	settle().await;
	assert_eq!(writes(&running.mock().recording()).len(), 1);
	running.stop().await;
}

fn profile_requests(recording: &Recording, user_id: Snowflake) -> usize {
	let path = format!("/users/{user_id}/profile");
	recording
		.requests
		.iter()
		.filter(|request| request.method == "GET" && request.path.ends_with(&path))
		.count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_profile_is_fetched_once_and_a_missing_one_fails_quietly() {
	let running = start("status-profiles").await;
	let me = running.fixtures().current_user.id;
	let other = running
		.fixtures()
		.users
		.keys()
		.copied()
		.map(Snowflake)
		.find(|id| *id != me)
		.expect("another user");
	let guild = Snowflake(running.fixtures().guilds[0].id);
	for _ in 0..3 {
		running.session().send(Command::LoadProfile {
			user_id: other,
			guild_id: Some(guild),
		});
	}
	running
		.until("the profile", |state: &State| state.profile(other).is_some())
		.await;
	running.session().send(Command::LoadProfile {
		user_id: other,
		guild_id: Some(guild),
	});
	let missing = Snowflake(4_242);
	running.session().send(Command::LoadProfile {
		user_id: missing,
		guild_id: None,
	});
	running
		.until_wire("the missing profile request", |recording| {
			(profile_requests(recording, missing) == 1).then_some(())
		})
		.await;
	running.session().send(Command::LoadProfile {
		user_id: missing,
		guild_id: None,
	});
	settle().await;
	let recording = running.mock().recording();
	assert_eq!(profile_requests(&recording, other), 1);
	assert_eq!(profile_requests(&recording, missing), 1);
	let request = recording
		.requests
		.iter()
		.find(|request| request.path.ends_with(&format!("/users/{other}/profile")))
		.expect("the profile request");
	let query = request.query.as_deref().unwrap_or_default();
	assert!(query.contains("with_mutual_guilds=true"));
	assert!(query.contains(&format!("guild_id={guild}")));
	let state = running.session().state().read();
	assert!(state.profile(missing).is_none());
	assert!(state.last_rejection().is_none());
	drop(state);
	running.stop().await;
}
