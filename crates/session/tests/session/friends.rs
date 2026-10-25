use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mockcord::{Recording, Reply, RestRequest};
use protocol::{RelationshipKind, Snowflake};
use serde_json::{Value, json};
use session::{Command, FriendSource, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const RELATIONSHIPS: &str = "/users/@me/relationships";
const ONE_RELATIONSHIP: &str = "/users/@me/relationships/*";
const SETTLE: Duration = Duration::from_millis(400);
const MUTATION_GAP: Duration = Duration::from_millis(200);
const GAP_SLACK: Duration = Duration::from_millis(20);

fn context(request: &RestRequest) -> Option<Value> {
	let header = request.header("x-context-properties")?;
	serde_json::from_slice(&STANDARD.decode(header).ok()?).ok()
}

fn requests(recording: &Recording, method: &str, path: &str) -> Vec<RestRequest> {
	recording.requests_to(method, path).cloned().collect()
}

fn kind_of(state: &State, user_id: Snowflake) -> Option<RelationshipKind> {
	let rows = [
		(RelationshipKind::Friend, state.friends()),
		(RelationshipKind::IncomingRequest, state.pending_incoming()),
		(RelationshipKind::OutgoingRequest, state.pending_outgoing()),
		(RelationshipKind::Blocked, state.blocked()),
	];
	rows.into_iter()
		.find(|(_, rows)| rows.iter().any(|row| row.user_id == user_id))
		.map(|(kind, _)| kind)
}

fn rejections(running: &Running) -> u64 {
	running
		.session()
		.state()
		.read()
		.last_rejection()
		.map_or(0, |rejection| rejection.id)
}

struct People {
	stranger: (Snowflake, String),
	other_stranger: (Snowflake, String),
	friend: Snowflake,
	incoming: Snowflake,
	blocked: Snowflake,
}

fn people(running: &Running) -> People {
	let state = running.session().state().read();
	let me = state.current_user_id();
	let related = |id: Snowflake| kind_of(&state, id).is_some();
	let mut strangers = running
		.fixtures()
		.users
		.values()
		.filter(|user| Some(user.id) != me && !related(user.id))
		.map(|user| (user.id, user.username.clone()));
	People {
		stranger: strangers.next().expect("a stranger in the fixtures"),
		other_stranger: strangers.next().expect("a second stranger in the fixtures"),
		friend: state.friends()[0].user_id,
		incoming: state.pending_incoming()[0].user_id,
		blocked: state.blocked()[0].user_id,
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adding_a_friend_by_username_sends_the_web_clients_request() {
	let running = Running::start("friends-add").await;
	running.connected().await;
	let people = people(&running);
	let (user_id, username) = people.stranger.clone();
	running.session().send(Command::AddFriend {
		username: username.clone(),
		from: FriendSource::AddFriend,
	});
	running
		.until("the outgoing request", |state| {
			kind_of(state, user_id) == Some(RelationshipKind::OutgoingRequest)
		})
		.await;
	let (other_id, other_name) = people.other_stranger;
	running.session().send(Command::AddFriend {
		username: other_name.clone(),
		from: FriendSource::Profile,
	});
	running
		.until("the second outgoing request", |state| {
			kind_of(state, other_id) == Some(RelationshipKind::OutgoingRequest)
		})
		.await;
	let sent = requests(&running.mock().recording(), "POST", RELATIONSHIPS);
	assert_eq!(sent.len(), 2);
	assert_eq!(
		sent[0].json(),
		Some(json!({ "username": username, "discriminator": null }))
	);
	assert_eq!(context(&sent[0]), Some(json!({ "location": "Add Friend" })));
	assert_eq!(
		sent[1].json(),
		Some(json!({ "username": other_name, "discriminator": null }))
	);
	assert!(
		context(&sent[1]).is_none(),
		"only the Add Friend tab names its location"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepting_removing_and_blocking_are_paced_relationship_requests() {
	let tuning = RestTuning {
		mutation_gap: MUTATION_GAP,
		..RestTuning::default()
	};
	let running = Running::start_with(Options::new("friends-actions").rest(tuning)).await;
	running.connected().await;
	let people = people(&running);
	let (stranger, _) = people.stranger;
	for command in [
		Command::AcceptFriend {
			user_id: people.incoming,
		},
		Command::RemoveRelationship {
			user_id: people.friend,
		},
		Command::BlockUser { user_id: stranger },
		Command::RemoveRelationship {
			user_id: people.blocked,
		},
	] {
		running.session().send(command);
	}
	running
		.until("every relationship to change", |state| {
			kind_of(state, people.incoming) == Some(RelationshipKind::Friend)
				&& kind_of(state, people.friend).is_none()
				&& kind_of(state, stranger) == Some(RelationshipKind::Blocked)
				&& kind_of(state, people.blocked).is_none()
		})
		.await;
	let recording = running.mock().recording();
	let path = |user_id: Snowflake| format!("{RELATIONSHIPS}/{user_id}");
	let accepted = requests(&recording, "PUT", &path(people.incoming));
	assert_eq!(accepted.len(), 1);
	assert_eq!(accepted[0].json(), Some(json!({})));
	let blocked = requests(&recording, "PUT", &path(stranger));
	assert_eq!(blocked.len(), 1);
	assert_eq!(blocked[0].json(), Some(json!({ "type": 2 })));
	for user_id in [people.friend, people.blocked] {
		let removed = requests(&recording, "DELETE", &path(user_id));
		assert_eq!(removed.len(), 1);
		assert!(removed[0].body.is_empty());
	}
	let mut arrivals: Vec<_> = recording
		.requests
		.iter()
		.filter(|request| request.is("PUT", ONE_RELATIONSHIP) || request.is("DELETE", ONE_RELATIONSHIP))
		.map(|request| request.at)
		.collect();
	assert_eq!(arrivals.len(), 4);
	assert!(
		recording
			.requests
			.iter()
			.all(|request| request.header("x-context-properties").is_none())
	);
	arrivals.sort();
	for pair in arrivals.windows(2) {
		let gap = pair[1].duration_since(pair[0]);
		assert!(
			gap + GAP_SLACK >= MUTATION_GAP,
			"relationship requests {gap:?} apart"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_on_a_friend_request_stops_and_is_never_retried() {
	let running = Running::start("friends-captcha").await;
	running.connected().await;
	let (user_id, username) = people(&running).stranger;
	running
		.mock()
		.respond_once("POST", RELATIONSHIPS, Reply::captcha_required());
	let before = rejections(&running);
	running.session().send(Command::AddFriend {
		username,
		from: FriendSource::AddFriend,
	});
	running
		.until("a rejection", |state| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.id > before)
		})
		.await;
	let reason = running
		.session()
		.state()
		.read()
		.last_rejection()
		.map(|rejection| rejection.reason.clone())
		.unwrap_or_default();
	assert!(reason.contains("official Discord client"), "{reason}");
	tokio::time::sleep(SETTLE).await;
	assert_eq!(
		requests(&running.mock().recording(), "POST", RELATIONSHIPS).len(),
		1
	);
	assert!(kind_of(&running.session().state().read(), user_id).is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relationship_events_move_people_between_the_lists() {
	let running = Running::start("friends-events").await;
	running.connected().await;
	let people = people(&running);
	let id = people.friend.to_string();
	running.mock().dispatch(
		"RELATIONSHIP_UPDATE",
		&json!({ "id": id, "type": 1, "nickname": "bestie" }),
	);
	running
		.until("the nickname", |state| {
			state
				.friends()
				.iter()
				.any(|row| row.user_id == people.friend && row.nickname.as_deref() == Some("bestie"))
		})
		.await;
	running.mock().dispatch(
		"RELATIONSHIP_UPDATE",
		&json!({ "id": "1", "type": "odd", "nickname": "nobody" }),
	);
	running
		.mock()
		.dispatch("RELATIONSHIP_UPDATE", &json!({ "id": id, "nickname": null }));
	running
		.until("the nickname to clear", |state| {
			state
				.friends()
				.iter()
				.any(|row| row.user_id == people.friend && row.nickname.is_none())
		})
		.await;
	running.mock().dispatch(
		"RELATIONSHIP_REMOVE",
		&json!({ "id": people.incoming.to_string(), "type": 3 }),
	);
	let (stranger, _) = people.stranger;
	let user = running.fixtures().users[&stranger.0].clone();
	running.mock().dispatch(
		"RELATIONSHIP_ADD",
		&json!({ "id": stranger.to_string(), "type": 3, "user": user }),
	);
	running
		.until("the request to move", |state| {
			kind_of(state, people.incoming).is_none()
				&& kind_of(state, stranger) == Some(RelationshipKind::IncomingRequest)
		})
		.await;
	running.stop().await;
}
