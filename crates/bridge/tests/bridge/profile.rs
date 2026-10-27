use bridge::{DmStatus, PanelUser, ProfileCard, StatusChoice};
use chrono::Utc;
use protocol::{ActivityKind, Snowflake, StatusKind};
use session::Command;
use state::State;

use crate::harness::Running;

fn playing_user(state: &State, me: Snowflake, candidates: &[Snowflake]) -> Option<Snowflake> {
	candidates.iter().copied().find(|id| {
		*id != me
			&& state.presence(*id).is_some_and(|presence| {
				presence
					.activities
					.iter()
					.any(|activity| activity.kind == ActivityKind::Playing)
			})
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_profile_card_shows_presence_activities_and_the_loaded_profile() {
	let mut running = Running::start("profile-card").await;
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	let me = running.server.fixtures().current_user.id;
	let users: Vec<Snowflake> = running
		.server
		.fixtures()
		.users
		.keys()
		.copied()
		.map(Snowflake)
		.collect();
	running
		.until_state("someone playing a game", |state| {
			playing_user(state, me, &users).is_some()
		})
		.await;
	let user_id = {
		let state = running.session().state().read();
		playing_user(&state, me, &users).expect("a user playing a game")
	};
	{
		let state = running.session().state().read();
		let card =
			ProfileCard::of(&state, user_id, Some(guild_id), Utc::now()).expect("a card for a known user");
		assert!(!card.loaded);
		assert_eq!(card.status, DmStatus::of(&state, user_id));
		assert_ne!(card.status, DmStatus::Offline);
		let playing = card
			.activities
			.iter()
			.find(|line| line.heading == "Playing")
			.expect("the game shows on the card");
		assert!(!playing.name.is_empty());
	}
	running.session().send(Command::LoadProfile {
		user_id,
		guild_id: Some(guild_id),
	});
	running
		.until_state("the profile to load", |state| state.profile(user_id).is_some())
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, user_id, Some(guild_id), Utc::now()).expect("a card");
		let username = state.user(user_id).expect("the user").username.clone();
		assert!(card.loaded);
		assert_eq!(card.bio.as_deref(), Some(format!("hi, i am {username}").as_str()));
		assert!(!card.mutual_guilds.is_empty());
		assert!(
			card.mutual_guilds
				.iter()
				.all(|guild| state.guild(guild.id).is_some())
		);
		assert!(
			card.avatar_request("https://cdn.discordapp.com", 80., 1.)
				.is_some()
		);
		assert!(!card.mine);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_panel_and_own_card_follow_a_status_and_custom_status_change() {
	let mut running = Running::start("profile-panel").await;
	let me = running.server.fixtures().current_user.id;
	{
		let state = running.session().state().read();
		let panel = PanelUser::of(&state, Utc::now()).expect("the signed in user");
		assert_eq!(panel.status, StatusChoice::Online);
		assert_eq!(panel.subtitle(), panel.username);
	}
	running.session().send(Command::SetStatus(StatusKind::Dnd));
	running.session().send(Command::SetCustomStatus {
		text: Some("heads down".to_owned()),
		emoji: Some("\u{1f3a7}".to_owned()),
		expires: None,
	});
	running
		.until_state("the new status in the panel", |state| {
			PanelUser::of(state, Utc::now())
				.is_some_and(|panel| panel.status == StatusChoice::Dnd && panel.subtitle() == "heads down")
		})
		.await;
	{
		let state = running.session().state().read();
		let panel = PanelUser::of(&state, Utc::now()).expect("the panel");
		assert_eq!(panel.custom_status.glyph(), Some("\u{1f3a7}"));
		assert_eq!(panel.status.dot(), DmStatus::Dnd);
		let card = ProfileCard::of(&state, me, None, Utc::now()).expect("my own card");
		assert!(card.mine);
		assert_eq!(card.status, DmStatus::Dnd);
		assert_eq!(card.custom_status.text.as_deref(), Some("heads down"));
	}
	running.session().send(Command::SetStatus(StatusKind::Invisible));
	running
		.until_state("invisible in the panel", |state| {
			PanelUser::of(state, Utc::now()).is_some_and(|panel| panel.status == StatusChoice::Invisible)
		})
		.await;
	{
		let state = running.session().state().read();
		let panel = PanelUser::of(&state, Utc::now()).expect("the panel");
		assert_eq!(panel.status.dot(), DmStatus::Offline);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_loaded_profile_shows_its_guild_tag_and_connections_and_missing_parts_stay_empty() {
	let mut running = Running::start("profile-parts").await;
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	let me = running.server.fixtures().current_user.id;
	let fixture = running
		.server
		.fixtures()
		.guilds
		.iter()
		.find(|guild| guild.id == guild_id.0)
		.expect("the selected guild in the fixtures");
	let mut members: Vec<Snowflake> = running
		.server
		.fixtures()
		.users
		.keys()
		.copied()
		.filter(|id| *id != me.0 && fixture.member(*id).is_some())
		.map(Snowflake)
		.collect();
	members.sort();
	{
		let known = members.clone();
		running
			.until_state("the members to be known", move |state| {
				known.iter().filter(|id| state.user(**id).is_some()).count() > 3
			})
			.await;
	}
	let members: Vec<Snowflake> = {
		let state = running.session().state().read();
		members
			.into_iter()
			.filter(|id| state.user(*id).is_some())
			.collect()
	};
	let tagged = *members
		.iter()
		.find(|id| id.0.is_multiple_of(3))
		.expect("a member with a guild tag");
	let plain = *members
		.iter()
		.find(|id| !id.0.is_multiple_of(3))
		.expect("a member without a guild tag");
	for user_id in [tagged, plain] {
		running.session().send(Command::LoadProfile {
			user_id,
			guild_id: Some(guild_id),
		});
	}
	running
		.until_state("both profiles to load", |state| {
			state.profile(tagged).is_some() && state.profile(plain).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		let guild_name = state.guild(guild_id).expect("the guild").name.clone();
		let card = ProfileCard::of(&state, tagged, Some(guild_id), Utc::now()).expect("the tagged card");
		let tag = card.tag.as_ref().expect("the guild tag shows");
		assert_eq!(tag.text, "RUST");
		assert!(tag.badge_request().is_some());
		assert!(
			card.connections
				.iter()
				.any(|connection| connection.kind == "github" && connection.verified)
		);
		assert_eq!(card.guild_name.as_deref(), Some(guild_name.as_str()));
		let bare = ProfileCard::of(&state, plain, Some(guild_id), Utc::now()).expect("the plain card");
		assert!(bare.tag.is_none());
		assert_eq!(bare.badges.is_empty(), !plain.0.is_multiple_of(2));
		assert!(bare.banner.is_none() && bare.banner_request("https://cdn.discordapp.com").is_none());
		assert!(bare.pronouns.is_none());
		assert!(bare.accent.is_none());
		let without_guild =
			ProfileCard::of(&state, plain, None, Utc::now()).expect("the card outside a guild");
		assert!(without_guild.guild_name.is_none());
		assert!(without_guild.roles.is_empty());
	}
	running.stop().await;
}

fn member_with_roles(state: &State, guild_id: Snowflake, me: Snowflake) -> Option<Snowflake> {
	let guild = state.guild(guild_id)?;
	let mut ids: Vec<Snowflake> = guild
		.members
		.iter()
		.filter(|(id, member)| **id != me && !member.roles.is_empty() && state.user(**id).is_some())
		.map(|(id, _)| *id)
		.filter(|id| state.profile(*id).is_none())
		.collect();
	ids.sort_unstable();
	ids.first().copied()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_opened_profile_shows_the_member_at_once_and_fills_in_when_it_arrives() {
	let mut running = Running::start("profile-fills-in").await;
	let open = {
		let state = running.session().state().clone();
		bridge::first_channel(&state.read()).expect("a channel to open")
	};
	let guild_id = open.guild_id.expect("the first channel is in a guild");
	if let Some(command) = running.select_guild(guild_id) {
		running.session().send(command);
	}
	let command = running.open_channel(open.channel_id).expect("the channel opens");
	running.session().send(command);
	let me = running.server.fixtures().current_user.id;
	running
		.until_state("a member with roles", |state| {
			member_with_roles(state, guild_id, me).is_some()
		})
		.await;
	let user_id = {
		let state = running.session().state().read();
		member_with_roles(&state, guild_id, me).expect("a member with roles")
	};
	running
		.server
		.control()
		.hold_requests("GET", &format!("/users/{user_id}/profile"));
	running.session().send(Command::LoadProfile {
		user_id,
		guild_id: Some(guild_id),
	});
	running
		.until_state("the profile to be requested", |state| {
			state.profile_loading(user_id)
		})
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, user_id, Some(guild_id), Utc::now()).expect("a card at once");
		assert!(card.loading);
		assert!(!card.loaded);
		assert_eq!(card.name, state.display_name(Some(guild_id), user_id));
		assert!(!card.roles.is_empty());
		assert!(card.bio.is_none());
		assert!(card.connections.is_empty());
		assert!(
			card.avatar_request("https://cdn.discordapp.com", 80., 1.)
				.is_some()
		);
	}
	running.server.control().release_requests();
	running
		.until_state("the profile to arrive", |state| {
			state.profile(user_id).is_some() && !state.profile_loading(user_id)
		})
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, user_id, Some(guild_id), Utc::now()).expect("a card");
		assert!(card.loaded);
		assert!(!card.loading);
		assert!(card.bio.is_some());
		assert!(!card.roles.is_empty());
	}
	running.stop().await;
}
