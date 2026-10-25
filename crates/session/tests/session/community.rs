use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use mockcord::{COMMUNITY_INVITE, Recording, Reply, RestRequest};
use protocol::Snowflake;
use serde_json::{Value, json};
use session::{Command, InviteSource, RestTuning, RiskyReason};
use state::State;

use crate::harness::{Options, Running};

const INVITE_PATH: &str = "/invites/mockcommunity";
const SETTLE: Duration = Duration::from_millis(400);
const NO_ARTIFICIAL_WAIT: Duration = Duration::from_millis(500);
const SEND_GAP: Duration = Duration::from_millis(300);
const GAP_SLACK: Duration = Duration::from_millis(20);

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

async fn rejected_since(running: &Running, before: u64) -> String {
	running
		.until("a rejection", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.id > before)
		})
		.await;
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map(|rejection| rejection.reason.clone())
		.unwrap_or_default()
}

fn context(request: &RestRequest) -> Option<Value> {
	let header = request.header("x-context-properties")?;
	serde_json::from_slice(&STANDARD.decode(header).ok()?).ok()
}

fn lock_reason(state: &State, channel_id: Snowflake) -> Option<RiskyReason> {
	state.channel_lock(channel_id, Utc::now()).map(|lock| lock.reason)
}

fn requests(recording: &Recording, method: &str, path: &str) -> Vec<RestRequest> {
	recording.requests_to(method, path).cloned().collect()
}

async fn resolve(running: &Running) -> (Snowflake, Snowflake) {
	running.connected().await;
	running.session().send(Command::ResolveInvite {
		code: COMMUNITY_INVITE.to_owned(),
	});
	running
		.until("the invite preview", |state| {
			state.invite_preview(COMMUNITY_INVITE).is_some()
		})
		.await;
	let state = running.session().state().read();
	let preview = state.invite_preview(COMMUNITY_INVITE).expect("the preview");
	(
		preview.guild_id.expect("the invite's server"),
		preview.channel_id.expect("the invite's channel"),
	)
}

async fn join(running: &Running) -> (Snowflake, Snowflake) {
	let (guild_id, channel_id) = resolve(running).await;
	running.session().send(Command::AcceptInvite {
		code: COMMUNITY_INVITE.to_owned(),
		from: InviteSource::AddServer,
	});
	running
		.until("the joined server", |state| state.guild(guild_id).is_some())
		.await;
	(guild_id, channel_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn joining_by_invite_sends_the_web_clients_context_and_session() {
	let running = Running::start("community-join").await;
	let (guild_id, channel_id) = join(&running).await;
	let recording = running.mock().recording();
	let lookup = requests(&recording, "GET", INVITE_PATH);
	assert_eq!(lookup.len(), 1);
	assert_eq!(
		lookup[0].query.as_deref(),
		Some("with_counts=true&with_expiration=true")
	);
	assert!(context(&lookup[0]).is_none());
	let accepted = requests(&recording, "POST", INVITE_PATH);
	assert_eq!(accepted.len(), 1);
	assert_eq!(
		context(&accepted[0]),
		Some(json!({
			"location": "Join Guild",
			"location_guild_id": guild_id.to_string(),
			"location_channel_id": channel_id.to_string(),
			"location_channel_type": 0,
		}))
	);
	let session_id = running.session().state().read().session().session_id.clone();
	assert!(!session_id.is_empty());
	assert_eq!(accepted[0].json(), Some(json!({ "session_id": session_id })));
	assert!(
		recording
			.requests
			.iter()
			.filter(|request| request.path != INVITE_PATH || request.method != "POST")
			.all(|request| request.header("x-context-properties").is_none())
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_on_join_stops_and_is_never_retried() {
	let running = Running::start("community-captcha").await;
	let (guild_id, _) = resolve(&running).await;
	running
		.mock()
		.respond_once("POST", INVITE_PATH, Reply::captcha_required());
	let before = rejections(&running);
	running.session().send(Command::AcceptInvite {
		code: COMMUNITY_INVITE.to_owned(),
		from: InviteSource::AddServer,
	});
	let reason = rejected_since(&running, before).await;
	assert!(reason.contains("official Discord client"), "{reason}");
	tokio::time::sleep(SETTLE).await;
	let recording = running.mock().recording();
	assert_eq!(requests(&recording, "POST", INVITE_PATH).len(), 1);
	assert!(requests(&recording, "GET", "/guilds/*/onboarding").is_empty());
	assert!(running.session().state().read().guild(guild_id).is_none());
	assert!(running.mock().community().is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onboarding_shows_at_once_and_submits_only_the_users_choices() {
	let running = Running::start("community-onboarding").await;
	let (guild_id, channel_id) = join(&running).await;
	running
		.until("the onboarding and the rules", |state| {
			state.onboarding(guild_id).is_some() && state.member_verification(guild_id).is_some()
		})
		.await;
	let recording = running.mock().recording();
	let accepted = requests(&recording, "POST", INVITE_PATH)[0].at;
	let shown = requests(&recording, "GET", &format!("/guilds/{guild_id}/onboarding"));
	assert_eq!(shown.len(), 1);
	assert!(shown[0].at.duration_since(accepted) < NO_ARTIFICIAL_WAIT);
	let rules = requests(
		&recording,
		"GET",
		&format!("/guilds/{guild_id}/member-verification"),
	);
	assert_eq!(
		rules[0].query.as_deref(),
		Some("with_guild=false&invite_code=mockcommunity")
	);
	let (prompts, required, optional) = {
		let state = running.session().state().read();
		let onboarding = state.onboarding(guild_id).expect("the onboarding");
		let prompts: Vec<Snowflake> = onboarding.prompts.iter().map(|prompt| prompt.id).collect();
		let required = onboarding.prompts[0].options[1].id;
		let optional = onboarding.prompts[1].options[0].id;
		(prompts, required, optional)
	};
	assert_eq!(prompts.len(), 2, "prompts outside onboarding are not shown");
	assert_eq!(
		lock_reason(&running.session().state().read(), channel_id),
		Some(RiskyReason::Screening)
	);
	tokio::time::sleep(SETTLE).await;
	let untouched = running.mock().recording();
	assert!(requests(&untouched, "PUT", "/guilds/*/onboarding-responses").is_empty());
	assert!(requests(&untouched, "PUT", "/guilds/*/requests/@me").is_empty());

	running
		.session()
		.send(Command::AcceptMemberVerification { guild_id });
	running
		.until("the rules to be accepted", |state| {
			lock_reason(state, channel_id) == Some(RiskyReason::Onboarding)
		})
		.await;
	let agreement = requests(
		&running.mock().recording(),
		"PUT",
		&format!("/guilds/{guild_id}/requests/@me"),
	);
	assert_eq!(agreement.len(), 1);
	let body = agreement[0].json().expect("a json body");
	assert_eq!(body["version"], json!("2026-09-01T12:00:00.000000+00:00"));
	assert_eq!(body["form_fields"][0]["response"], json!(true));
	assert_eq!(body["form_fields"][0]["field_type"], json!("TERMS"));

	let before = rejections(&running);
	running.session().send(Command::SubmitOnboarding {
		guild_id,
		chosen: vec![optional],
		prompts_seen: Vec::new(),
		options_seen: Vec::new(),
	});
	assert!(rejected_since(&running, before).await.contains("required"));
	assert!(
		requests(
			&running.mock().recording(),
			"PUT",
			"/guilds/*/onboarding-responses"
		)
		.is_empty()
	);

	let seen_at = 1_790_000_000_000;
	running.session().send(Command::SubmitOnboarding {
		guild_id,
		chosen: vec![required, optional, Snowflake(42)],
		prompts_seen: prompts.iter().map(|id| (*id, seen_at)).collect(),
		options_seen: vec![(required, seen_at), (optional, seen_at)],
	});
	running
		.until("the server to unlock", |state| {
			lock_reason(state, channel_id).is_none() && state.onboarding(guild_id).is_none()
		})
		.await;
	let submitted = requests(
		&running.mock().recording(),
		"PUT",
		&format!("/guilds/{guild_id}/onboarding-responses"),
	);
	assert_eq!(submitted.len(), 1);
	let mut prompts_seen = serde_json::Map::new();
	for id in &prompts {
		prompts_seen.insert(id.to_string(), json!(seen_at));
	}
	assert_eq!(
		submitted[0].json(),
		Some(json!({
			"onboarding_responses": [required.to_string(), optional.to_string()],
			"onboarding_prompts_seen": prompts_seen,
			"onboarding_responses_seen": {
				required.to_string(): seen_at,
				optional.to_string(): seen_at,
			},
		}))
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn creating_an_invite_sends_the_web_clients_body() {
	let running = Running::start("community-invite").await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::CreateInvite {
		channel_id,
		max_age: 604_800,
		max_uses: 0,
		temporary: false,
	});
	running
		.until("the created invite", |state| {
			state.invite_created(channel_id).is_some()
		})
		.await;
	let created = requests(
		&running.mock().recording(),
		"POST",
		&format!("/channels/{channel_id}/invites"),
	);
	assert_eq!(created.len(), 1);
	assert_eq!(
		created[0].json(),
		Some(json!({ "max_age": 604_800, "max_uses": 0, "temporary": false }))
	);
	assert!(context(&created[0]).is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forwards_are_paced_sends_and_respect_locks() {
	let tuning = RestTuning {
		send_gap: SEND_GAP,
		..RestTuning::default()
	};
	let running = Running::start_with(Options::new("community-forward").rest(tuning)).await;
	let source = running.open_text_channel().await;
	let (_, gated) = join(&running).await;
	let (guild_id, message_id, targets) = {
		let state = running.session().state().read();
		let guild_id = state.channel_guild(source).expect("a server channel");
		let message_id = state
			.messages(source)
			.and_then(|cache| cache.ordered.last().map(|message| message.id))
			.expect("a message to forward");
		let targets: Vec<Snowflake> = state
			.guilds()
			.flat_map(|guild| guild.channels.values())
			.filter(|channel| channel.kind == protocol::ChannelType::GuildText)
			.filter(|channel| channel.id != source && channel.id != gated)
			.filter(|channel| lock_reason(&state, channel.id).is_none())
			.map(|channel| channel.id)
			.take(3)
			.collect();
		(guild_id, message_id, targets)
	};
	assert_eq!(targets.len(), 3);

	let before = rejections(&running);
	running.session().send(Command::ForwardMessage {
		channel_id: source,
		message_id,
		to_channel_id: gated,
	});
	let reason = rejected_since(&running, before).await;
	assert_eq!(reason, RiskyReason::Screening.explanation());

	for target in &targets {
		running.session().send(Command::ForwardMessage {
			channel_id: source,
			message_id,
			to_channel_id: *target,
		});
	}
	let sent = running
		.until_wire("three forwards", |recording| {
			let found: Vec<RestRequest> = requests(recording, "POST", "/channels/*/messages")
				.into_iter()
				.filter(|request| {
					request
						.json()
						.is_some_and(|body| body["message_reference"]["type"] == json!(1))
				})
				.collect();
			(found.len() == targets.len()).then_some(found)
		})
		.await;
	let mut paths: Vec<String> = sent.iter().map(|request| request.path.clone()).collect();
	let mut expected: Vec<String> = targets
		.iter()
		.map(|target| format!("/channels/{target}/messages"))
		.collect();
	paths.sort();
	expected.sort();
	assert_eq!(paths, expected);
	let mut nonces = Vec::new();
	for request in &sent {
		let body = request.json().expect("a json body");
		assert_eq!(
			body["message_reference"],
			json!({
				"type": 1,
				"message_id": message_id.to_string(),
				"channel_id": source.to_string(),
				"guild_id": guild_id.to_string(),
			})
		);
		assert!(body.get("content").is_none());
		nonces.push(body["nonce"].as_str().expect("a nonce").to_owned());
	}
	nonces.sort();
	nonces.dedup();
	assert_eq!(nonces.len(), targets.len());
	let mut sent = sent;
	sent.sort_by_key(|request| request.at);
	for pair in sent.windows(2) {
		assert!(pair[1].at.duration_since(pair[0].at) + GAP_SLACK >= SEND_GAP);
	}
	assert!(
		requests(
			&running.mock().recording(),
			"POST",
			&format!("/channels/{gated}/messages")
		)
		.is_empty()
	);
	running.stop().await;
}
