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
