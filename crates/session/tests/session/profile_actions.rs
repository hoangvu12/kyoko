use std::time::Duration;

use protocol::{RelationshipKind, Snowflake};
use serde_json::json;
use session::Command;
use state::State;

use crate::harness::Running;

const RELATIONSHIPS: &str = "/users/@me/relationships";

fn stranger(running: &Running) -> Snowflake {
	let state = running.session().state().read();
	let me = state.current_user_id();
	running
		.fixtures()
		.users
		.values()
		.map(|user| user.id)
		.filter(|id| Some(*id) != me)
		.find(|id| state.relationships().all(|relationship| relationship.id != *id))
		.expect("a stranger in the fixtures")
}

fn friend(state: &State) -> Snowflake {
	state.friends()[0].user_id
}

fn kind(state: &State, user_id: Snowflake) -> Option<RelationshipKind> {
	state
		.relationships()
		.find(|relationship| relationship.id == user_id)
		.map(|relationship| relationship.kind)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignoring_flags_the_person_and_unignoring_restores_what_was_there() {
	let running = Running::start("profile-ignore").await;
	running.connected().await;
	let (stranger, friend) = (stranger(&running), friend(&running.session().state().read()));
	for user_id in [stranger, friend] {
		running.session().send(Command::IgnoreUser { user_id });
	}
	running
		.until("both to be ignored", |state| {
			state.is_ignored(stranger) && state.is_ignored(friend)
		})
		.await;
	assert_eq!(
		kind(&running.session().state().read(), friend),
		Some(RelationshipKind::Friend),
		"ignoring a friend keeps the friendship"
	);
	for user_id in [stranger, friend] {
		running.session().send(Command::UnignoreUser { user_id });
	}
	running
		.until("both to be unignored", |state| {
			!state.is_ignored(stranger) && !state.is_ignored(friend)
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(kind(&state, friend), Some(RelationshipKind::Friend));
		assert_eq!(
			kind(&state, stranger),
			None,
			"a stranger leaves no relationship behind"
		);
	}
	let recording = running.mock().recording();
	for user_id in [stranger, friend] {
		let path = format!("{RELATIONSHIPS}/{user_id}/ignore");
		assert_eq!(recording.requests_to("PUT", &path).count(), 1);
		assert_eq!(recording.requests_to("DELETE", &path).count(), 1);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inviting_to_a_server_sends_a_fresh_invite_in_the_dm_without_leaving_the_channel() {
	let running = Running::start("profile-invite").await;
	let channel_id = running.open_text_channel().await;
	let user_id = Snowflake(
		running.fixtures().guilds[0].members[1]
			.user
			.as_ref()
			.expect("a member")
			.id
			.0,
	);
	running
		.session()
		.send(Command::InviteToServer { user_id, channel_id });
	let body = running
		.until_wire("the invite link in the conversation", |recording| {
			recording
				.requests
				.iter()
				.filter(|request| request.method == "POST" && request.path.ends_with("/messages"))
				.filter_map(|request| request.json())
				.find(|body| {
					body["content"]
						.as_str()
						.is_some_and(|content| content.starts_with("https://discord.gg/"))
				})
		})
		.await;
	let recording = running.mock().recording();
	let invites = format!("/channels/{channel_id}/invites");
	let created: Vec<_> = recording.requests_to("POST", &invites).collect();
	assert_eq!(created.len(), 1);
	assert_eq!(
		created[0].json(),
		Some(json!({ "max_age": 604_800, "max_uses": 0, "temporary": false }))
	);
	let opened: Vec<_> = recording.requests_to("POST", "/users/@me/channels").collect();
	assert_eq!(
		opened[0].json(),
		Some(json!({ "recipients": [user_id.to_string()] }))
	);
	assert!(body["nonce"].is_string(), "sent through the outbox");
	tokio::time::sleep(Duration::from_millis(200)).await;
	let selected = running
		.session()
		.state()
		.read()
		.selected()
		.map(|open| open.channel_id);
	assert_eq!(selected, Some(channel_id), "the open channel stays open");
	running.stop().await;
}
