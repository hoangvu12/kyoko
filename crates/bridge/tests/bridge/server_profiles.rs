use std::sync::Arc;

use bridge::{Avatar, ProfileCard};
use chrono::Utc;
use protocol::{Member, Message, Snowflake, User};
use session::Command;

use crate::harness::Running;

const CDN: &str = "https://cdn.discordapp.com";

fn member_where(running: &Running, guild_id: Snowflake, wanted: impl Fn(&Member) -> bool) -> Member {
	running
		.server
		.fixtures()
		.guild(guild_id.0)
		.expect("the selected guild")
		.members
		.iter()
		.skip(1)
		.find(|member| wanted(member))
		.cloned()
		.expect("a member that fits")
}

fn user_of(member: &Member) -> User {
	member.user.clone().expect("a member user")
}

fn authored_by(user: &User, guild_id: Snowflake, member: Option<Member>) -> Arc<Message> {
	Arc::new(Message {
		id: Snowflake(user.id.0 + 1),
		channel_id: Snowflake(1),
		guild_id: Some(guild_id),
		author: user.clone(),
		member,
		content: "hello".to_owned(),
		..Message::default()
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_avatars_banners_and_bios_win_over_global_ones_and_animate_where_asked() {
	let mut running = Running::start("server-profiles").await;
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
	let moving = member_where(&running, guild_id, |member| {
		member
			.avatar
			.as_deref()
			.is_some_and(|hash| hash.starts_with("a_"))
	});
	let still = member_where(&running, guild_id, |member| {
		member
			.avatar
			.as_deref()
			.is_some_and(|hash| !hash.starts_with("a_"))
	});
	let bannered = member_where(&running, guild_id, |member| member.banner.is_some());
	let plain = member_where(&running, guild_id, |member| {
		member.avatar.is_none() && member.banner.is_none() && user_of(member).avatar.is_some()
	});
	let ids = [&moving, &still, &plain].map(|member| user_of(member).id);
	running
		.until_state("the members with their server looks", |state| {
			state
				.guild(guild_id)
				.is_some_and(|guild| ids.iter().all(|id| guild.members.contains_key(id)))
		})
		.await;
	{
		let state = running.session().state().read();
		let (moving_user, still_user, plain_user) = (user_of(&moving), user_of(&still), user_of(&plain));
		let moving_hash = moving.avatar.clone().expect("a server avatar");
		let server = Avatar::of(&state, Some(guild_id), &moving_user);
		assert!(server.is_animated());
		assert_eq!(
			server.request(CDN, 40., 1., true).url,
			format!(
				"{CDN}/guilds/{guild_id}/users/{}/avatars/{moving_hash}.webp?size=64&animated=true",
				moving_user.id
			)
		);
		assert_eq!(
			server.request(CDN, 40., 1., false).url,
			format!(
				"{CDN}/guilds/{guild_id}/users/{}/avatars/{moving_hash}.webp?size=64",
				moving_user.id
			)
		);
		let still_hash = still.avatar.clone().expect("a server avatar");
		let calm = Avatar::of(&state, Some(guild_id), &still_user);
		assert!(!calm.is_animated());
		assert!(calm.moving_request(CDN, 40., 1.).is_none());
		assert_eq!(
			calm.request(CDN, 40., 1., true).url,
			format!(
				"{CDN}/guilds/{guild_id}/users/{}/avatars/{still_hash}.webp?size=64",
				still_user.id
			)
		);
		let global = Avatar::of(&state, None, &moving_user);
		assert_eq!(global.guild_id, None);
		assert!(
			global
				.request(CDN, 40., 1., false)
				.url
				.starts_with(&format!("{CDN}/avatars/{}/", moving_user.id))
		);
		let fallback = Avatar::of(&state, Some(guild_id), &plain_user);
		assert_eq!(fallback.guild_id, None);
		assert_eq!(fallback.hash, plain_user.avatar);

		let entry =
			bridge::detached_entry(&state, Some(guild_id), &authored_by(&moving_user, guild_id, None));
		assert_eq!(entry.avatar, server);
		let stranger = User {
			id: Snowflake(9_000_000_001),
			username: "stranger".to_owned(),
			avatar: Some("a_globalhash".to_owned()),
			..User::default()
		};
		let sent_as = Member {
			avatar: Some("serverhash".to_owned()),
			nick: Some("Server Stranger".to_owned()),
			..Member::default()
		};
		let entry = bridge::detached_entry(
			&state,
			Some(guild_id),
			&authored_by(&stranger, guild_id, Some(sent_as)),
		);
		assert_eq!(entry.avatar.guild_id, Some(guild_id));
		assert_eq!(entry.avatar.hash.as_deref(), Some("serverhash"));
		assert_eq!(entry.author, "Server Stranger");
		let entry = bridge::detached_entry(&state, Some(guild_id), &authored_by(&stranger, guild_id, None));
		assert_eq!(entry.avatar, Avatar::of_user(&stranger));
		assert!(
			entry
				.avatar
				.request(CDN, 40., 1., true)
				.url
				.ends_with("a_globalhash.webp?size=64&animated=true")
		);
	}

	let bannered_user = user_of(&bannered);
	let banner_hash = bannered.banner.clone().expect("a server banner");
	running.session().send(Command::LoadProfile {
		user_id: bannered_user.id,
		guild_id: Some(guild_id),
	});
	running
		.until_state("the server profile", |state| {
			state.guild_profile(bannered_user.id, guild_id).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, bannered_user.id, Some(guild_id), Utc::now()).expect("a card");
		let banner = card.banner.clone().expect("the server banner");
		assert_eq!(banner.guild_id, Some(guild_id));
		assert!(banner.is_animated());
		assert_eq!(
			card.banner_request(CDN).expect("a banner request").url,
			format!(
				"{CDN}/guilds/{guild_id}/users/{}/banners/{banner_hash}.webp?size=1024&animated=true",
				bannered_user.id
			)
		);
		assert_eq!(
			card.bio.as_deref(),
			Some(format!("{}, server edition", bannered_user.username).as_str())
		);
		if let Some(nick) = bannered.nick.as_deref() {
			assert_eq!(card.name, nick);
		}
		let elsewhere = ProfileCard::of(&state, bannered_user.id, None, Utc::now()).expect("a global card");
		assert_eq!(
			elsewhere.bio.as_deref(),
			Some(format!("hi, i am {}", bannered_user.username).as_str())
		);
		assert!(
			elsewhere
				.banner
				.as_ref()
				.is_none_or(|banner| banner.guild_id.is_none())
		);
	}

	let moving_user = user_of(&moving);
	running.session().send(Command::LoadProfile {
		user_id: moving_user.id,
		guild_id: Some(guild_id),
	});
	running
		.until_state("the animated member's profile", |state| {
			state.guild_profile(moving_user.id, guild_id).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		let card = ProfileCard::of(&state, moving_user.id, Some(guild_id), Utc::now()).expect("a card");
		let request = card.avatar_request(CDN, 80., 1.).expect("an avatar request");
		assert!(request.url.starts_with(&format!(
			"{CDN}/guilds/{guild_id}/users/{}/avatars/a_",
			moving_user.id
		)));
		assert!(request.url.ends_with("animated=true"));
	}
	running.stop().await;
}
