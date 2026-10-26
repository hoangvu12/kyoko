use bridge::{DmPanel, DmStatus, active_card, since_label};
use chrono::{TimeDelta, Utc};
use protocol::Snowflake;
use serde_json::json;
use session::Command;

use crate::harness::Running;

const FIXTURE_DMS: usize = 5;

async fn listed(name: &str) -> Running {
	let mut running = Running::start(name).await;
	running
		.until("the direct messages to be listed", |view| {
			view.dms.entries().len() == FIXTURE_DMS
		})
		.await;
	running
}

fn profile_requests(running: &Running, user_id: Snowflake) -> usize {
	let path = format!("/users/{user_id}/profile");
	running
		.server
		.control()
		.recording()
		.requests
		.iter()
		.filter(|request| request.method == "GET" && request.path.ends_with(&path))
		.count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dm_shows_its_recipients_profile_and_the_mutuals_only_once_loaded() {
	let mut running = listed("dm-panel-profile").await;
	let (channel_id, recipient) = running
		.view
		.dms
		.entries()
		.iter()
		.find_map(|entry| entry.recipient.map(|user_id| (entry.id, user_id)))
		.expect("a one to one conversation");
	{
		let state = running.session().state().read();
		let DmPanel::Profile(profile) = DmPanel::of(&state, channel_id, Utc::now()) else {
			panic!("a one to one conversation shows its recipient's profile");
		};
		assert_eq!(profile.card.id, recipient);
		assert_eq!(profile.card.name, state.display_name(None, recipient));
		assert_eq!(profile.since, since_label(recipient.timestamp().date_naive()));
		assert!(profile.mutual_servers_text().is_none());
		assert!(profile.mutual_friends_text().is_none());
		assert!(profile.card.bio.is_none());
	}
	for _ in 0..2 {
		running.session().send(Command::LoadProfile {
			user_id: recipient,
			guild_id: None,
		});
	}
	running
		.until_state("the profile to load", |state| state.profile(recipient).is_some())
		.await;
	running.session().send(Command::LoadProfile {
		user_id: recipient,
		guild_id: None,
	});
	{
		let state = running.session().state().read();
		let DmPanel::Profile(profile) = DmPanel::of(&state, channel_id, Utc::now()) else {
			panic!("the profile is still shown");
		};
		let username = state.user(recipient).expect("the recipient").username.clone();
		assert_eq!(
			profile.card.bio.as_deref(),
			Some(format!("hi, i am {username}").as_str())
		);
		let servers = profile.mutual_servers.expect("the mutual servers are known");
		assert_eq!(
			profile.mutual_servers_text(),
			Some(format!("Mutual Servers — {servers}"))
		);
		assert!(profile.mutual_friends_text().is_some());
	}
	assert_eq!(profile_requests(&running, recipient), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_group_dm_lists_every_member_with_the_owner_marked() {
	let running = listed("dm-panel-group").await;
	let group = running
		.view
		.dms
		.entries()
		.iter()
		.find(|entry| entry.name == "weekend crew")
		.map(|entry| entry.id)
		.expect("the named group");
	{
		let state = running.session().state().read();
		let me = state.current_user_id().expect("signed in");
		let channel = state.channel(group).expect("the group channel");
		let DmPanel::Members(members) = DmPanel::of(&state, group, Utc::now()) else {
			panic!("a group shows its members");
		};
		assert_eq!(members.len(), channel.recipient_ids.len() + 1);
		assert!(members.iter().any(|member| member.id == me));
		let owners: Vec<Snowflake> = members
			.iter()
			.filter(|member| member.owner)
			.map(|member| member.id)
			.collect();
		assert_eq!(owners, channel.owner_id.into_iter().collect::<Vec<_>>());
		let names: Vec<String> = members.iter().map(|member| member.name.to_lowercase()).collect();
		let mut sorted = names.clone();
		sorted.sort();
		assert_eq!(names, sorted);
		for member in members.iter().filter(|member| member.id != me) {
			assert_eq!(member.status, DmStatus::of(&state, member.id));
		}
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_now_shows_a_friends_game_with_how_long_they_have_played() {
	let mut running = Running::start("dm-panel-active").await;
	running
		.until("the friends", |view| view.friends.everyone().next().is_some())
		.await;
	let friend = running
		.view
		.friends
		.everyone()
		.next()
		.map(|entry| entry.id)
		.expect("a friend");
	let started = Utc::now() - TimeDelta::minutes(65);
	running.server.control().dispatch(
		"PRESENCE_UPDATE",
		&json!({
			"user": { "id": friend.to_string() },
			"status": "online",
			"client_status": { "desktop": "online" },
			"activities": [
				{ "name": "Custom Status", "type": 4, "state": "brb" },
				{
					"name": "Outer Wilds",
					"type": 0,
					"details": "Exploring Brittle Hollow",
					"state": "Loop 14",
					"timestamps": { "start": started.timestamp_millis() },
				},
			],
		}),
	);
	running
		.until_state("the game to show", |state| {
			active_card(state, friend, Utc::now()).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		let card = active_card(&state, friend, Utc::now()).expect("the card");
		assert_eq!(card.title, "Outer Wilds — 1h");
		assert_eq!(card.details.as_deref(), Some("Exploring Brittle Hollow"));
		assert_eq!(card.state.as_deref(), Some("Loop 14"));
	}
	running.stop().await;
}
