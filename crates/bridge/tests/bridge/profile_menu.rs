use bridge::{ProfileCard, ProfileMenuItem, ProfileTheme, profile_link, profile_menu};
use chrono::Utc;
use protocol::{Permissions, Snowflake};
use session::Command;

use crate::harness::Running;

async fn opened(name: &str) -> (Running, Snowflake) {
	let mut running = Running::start(name).await;
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
	(running, guild_id)
}

fn someone(running: &Running, wanted: impl Fn(u64) -> bool) -> Snowflake {
	let me = running.server.fixtures().current_user.id;
	running
		.server
		.fixtures()
		.guilds
		.iter()
		.flat_map(|guild| guild.members.iter())
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.find(|id| *id != me && wanted(id.0))
		.expect("a fixture member that fits")
}

async fn load(running: &mut Running, user_id: Snowflake, guild_id: Option<Snowflake>) {
	running.session().send(Command::LoadProfile { user_id, guild_id });
	running
		.until_state("the profile", |state| match guild_id {
			Some(guild_id) => state.guild_profile(user_id, guild_id).is_some(),
			None => state.profile(user_id).is_some(),
		})
		.await;
}

fn theme_of(colors: Option<[u32; 2]>) -> Option<ProfileTheme> {
	colors.map(|[primary, accent]| ProfileTheme { primary, accent })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn theme_colours_and_the_effect_come_from_the_profile_and_the_server_part_wins() {
	let (mut running, guild_id) = opened("profile-theme").await;
	let themed = someone(&running, |id| ((id >> 22) / 1000).is_multiple_of(4));
	let plain = someone(&running, |id| !((id >> 22) / 1000).is_multiple_of(2));
	load(&mut running, themed, None).await;
	load(&mut running, plain, None).await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, themed, None, Utc::now()).expect("a card");
		let profile = state.profile(themed).expect("the profile");
		assert!(profile.theme_colors.is_some());
		assert_eq!(card.theme, theme_of(profile.theme_colors));
		assert!(card.effect, "the profile effect is known");
		let card = ProfileCard::of(&state, plain, None, Utc::now()).expect("a card");
		assert_eq!(card.theme, None);
		assert!(!card.effect);
	}
	let styled = running
		.server
		.fixtures()
		.guild(guild_id.0)
		.expect("the open guild")
		.members
		.iter()
		.skip(1)
		.find(|member| member.banner.is_some())
		.and_then(|member| member.user.as_ref().map(|user| user.id))
		.expect("a member with a server look");
	load(&mut running, styled, None).await;
	load(&mut running, styled, Some(guild_id)).await;
	{
		let state = running.session().state().read();
		let server = state
			.guild_profile(styled, guild_id)
			.and_then(|server| server.theme_colors);
		assert!(server.is_some(), "the server profile has its own theme");
		let card = ProfileCard::of(&state, styled, Some(guild_id), Utc::now()).expect("a card");
		assert_eq!(card.theme, theme_of(server));
		let global = ProfileCard::of(&state, styled, None, Utc::now()).expect("a card");
		assert_eq!(
			global.theme,
			theme_of(state.profile(styled).and_then(|profile| profile.theme_colors))
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_profile_menu_has_discords_items_and_ignore_turns_into_unignore() {
	let (mut running, _) = opened("profile-menu").await;
	let me = running.server.fixtures().current_user.id;
	let other = {
		let state = running.session().state().read();
		let related = |id: Snowflake| state.relationships().any(|relationship| relationship.id == id);
		someone(&running, |id| !related(Snowflake(id)))
	};
	load(&mut running, other, None).await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, other, None, Utc::now()).expect("a card");
		let groups = profile_menu(&state, &card, false);
		let keys: Vec<Vec<&str>> = groups
			.iter()
			.map(|group| group.iter().map(ProfileMenuItem::key).collect())
			.collect();
		assert_eq!(keys[1], ["ignore", "block", "report"]);
		assert_eq!(keys[2], ["copy-id"]);
		assert_eq!(&keys[0][..2], ["full-profile", "share"]);
		if let Some(ProfileMenuItem::InviteToServer(targets)) = groups[0].get(2) {
			assert!(!targets.is_empty());
			for target in targets {
				let guild = state.guild(target.guild_id).expect("one of my servers");
				assert!(!guild.members.contains_key(&other));
				assert!(!card.mutual_guilds.iter().any(|guild| guild.id == target.guild_id));
				assert!(
					state
						.my_permissions(target.channel_id)
						.contains(Permissions::CREATE_INSTANT_INVITE)
				);
			}
		}
		let full = profile_menu(&state, &card, true);
		assert_eq!(
			full[0][0],
			ProfileMenuItem::ShareProfile,
			"the full profile is already open"
		);
		assert_eq!(profile_link(other), format!("https://discord.com/users/{other}"));
		let mine = ProfileCard::of(&state, me, None, Utc::now()).expect("my card");
		let keys: Vec<Vec<&str>> = profile_menu(&state, &mine, false)
			.iter()
			.map(|group| group.iter().map(ProfileMenuItem::key).collect())
			.collect();
		assert_eq!(keys, [vec!["full-profile", "share"], vec!["copy-id"]]);
	}
	running.session().send(Command::IgnoreUser { user_id: other });
	running
		.until_state("the person to be ignored", |state| state.is_ignored(other))
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, other, None, Utc::now()).expect("a card");
		assert!(card.ignored);
		assert_eq!(
			profile_menu(&state, &card, false)[1][0],
			ProfileMenuItem::Unignore
		);
	}
	running.stop().await;
}
