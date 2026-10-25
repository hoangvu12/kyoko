use protocol::{ChannelType, Snowflake};
use serde_json::json;
use session::{CacheLimits, Command, Viewport};
use state::{ChannelRow, State};

use crate::harness::{Options, Running};

const WINDOW: usize = 60;
const CEILING: usize = 110;
const PAGE: usize = 50;

fn tiny() -> CacheLimits {
	CacheLimits {
		messages_per_channel: WINDOW,
		history_ceiling: CEILING,
		warm_channels: 3,
		..CacheLimits::default()
	}
}

fn text_channels(state: &State) -> Vec<(Snowflake, Snowflake)> {
	state
		.guilds()
		.flat_map(|guild| {
			guild
				.channels
				.values()
				.filter(|channel| channel.kind == ChannelType::GuildText)
				.map(move |channel| (guild.id, channel.id))
		})
		.collect()
}

fn cached(state: &State, channel_id: Snowflake) -> usize {
	state.messages(channel_id).map_or(0, |cache| cache.ordered.len())
}

fn ids(state: &State, channel_id: Snowflake) -> Vec<Snowflake> {
	state
		.messages(channel_id)
		.map(|cache| cache.ordered.iter().map(|message| message.id).collect())
		.unwrap_or_default()
}

fn channel_row(state: &State, guild_id: Snowflake, channel_id: Snowflake) -> Option<ChannelRow> {
	state
		.guild_channel_tree(guild_id)
		.into_iter()
		.flat_map(|node| node.category.into_iter().chain(node.children))
		.find(|row| row.id == channel_id)
}

async fn open(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the channel history to load", |state| {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == channel_id)
				&& cached(state, channel_id) > 0
		})
		.await;
}

async fn load_older(running: &Running, channel_id: Snowflake, oldest_before: Snowflake) {
	running.session().send(Command::LoadOlder { channel_id });
	running
		.until("an older page to load", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.oldest_id())
				.is_some_and(|oldest| oldest < oldest_before)
		})
		.await;
}

fn oldest(running: &Running, channel_id: Snowflake) -> Snowflake {
	ids(&running.session().state().read(), channel_id)[0]
}

fn mention_from_someone_else(running: &Running, channel_id: Snowflake) -> serde_json::Value {
	let fixtures = running.fixtures();
	let me = fixtures.current_user.id;
	let newest_fixture = fixtures
		.messages
		.values()
		.flat_map(|messages| messages.keys().copied())
		.max()
		.expect("fixture messages");
	let mut message = fixtures.messages[&channel_id.0]
		.values()
		.rev()
		.find(|message| message["author"]["id"] != json!(me.to_string()))
		.cloned()
		.expect("a fixture message by someone else");
	message["id"] = json!((newest_fixture + 1).to_string());
	message["content"] = json!("are you there?");
	message["mentions"] = json!([fixtures.current_user]);
	message["nonce"] = serde_json::Value::Null;
	message
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_the_warm_channels_keep_message_bodies() {
	let running = Running::start_with(Options::new("caches-warm").caches(tiny())).await;
	running.connected().await;
	let channels = text_channels(&running.session().state().read());
	assert!(channels.len() > 4, "the fixtures have enough text channels");
	let visited = &channels[..5];
	for &(guild_id, channel_id) in visited {
		open(&running, guild_id, channel_id).await;
	}
	{
		let state = running.session().state().read();
		let warm: Vec<Snowflake> = visited[2..].iter().map(|(_, channel_id)| *channel_id).collect();
		assert_eq!(state.warm_channels(), &warm[..]);
		for &(_, channel_id) in &visited[..2] {
			assert!(
				state.messages(channel_id).is_none(),
				"a cold channel keeps no bodies"
			);
			assert!(
				state
					.channel(channel_id)
					.is_some_and(|channel| channel.last_message_id.is_some())
			);
		}
		for channel_id in &warm {
			assert!(cached(&state, *channel_id) > 0);
		}
	}

	let (cold_guild, cold_channel) = visited[0];
	let mention = mention_from_someone_else(&running, cold_channel);
	let mention_id = Snowflake(mention["id"].as_str().expect("id").parse().expect("snowflake"));
	running.mock().dispatch("MESSAGE_CREATE", &mention);
	running
		.until("the cold channel to show the unread mention", |state| {
			channel_row(state, cold_guild, cold_channel).is_some_and(|row| row.unread && row.mentions == 1)
		})
		.await;
	{
		let state = running.session().state().read();
		assert!(state.messages(cold_channel).is_none());
		assert_eq!(
			state
				.channel(cold_channel)
				.and_then(|channel| channel.last_message_id),
			Some(mention_id)
		);
		let summary = state.guild_summary(cold_guild).expect("the guild summary");
		assert!(summary.unread);
		assert!(summary.mentions >= 1);
	}

	open(&running, cold_guild, cold_channel).await;
	{
		let state = running.session().state().read();
		assert!(state.is_warm(cold_channel));
		assert!(state.messages(visited[2].1).is_none());
		assert_eq!(state.warm_channels().len(), 3);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reading_back_widens_the_window_until_the_user_returns_to_the_bottom() {
	let running = Running::start_with(Options::new("caches-widen").caches(tiny())).await;
	let channel_id = running.open_text_channel().await;
	assert_eq!(cached(&running.session().state().read(), channel_id), PAGE);
	load_older(&running, channel_id, oldest(&running, channel_id)).await;
	let newest = {
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the open channel");
		assert_eq!(cache.ordered.len(), 2 * PAGE);
		assert_eq!(cache.window(state.limits()), 2 * PAGE);
		assert!(!cache.is_detached());
		cache.newest_id().expect("a newest message")
	};
	let visible = ids(&running.session().state().read(), channel_id)[2 * PAGE - 20];
	running.session().send(Command::SetViewport {
		channel_id,
		viewport: Viewport {
			first: visible,
			last: newest,
			at_bottom: true,
		},
	});
	running
		.until("the window to shrink back at the bottom", |state| {
			cached(state, channel_id) == WINDOW
		})
		.await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the open channel");
		assert_eq!(cache.newest_id(), Some(newest));
		assert!(cache.is_resident(visible, newest));
		assert_eq!(cache.window(state.limits()), WINDOW);
		assert!(!cache.reached_start);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_window_stops_at_the_ceiling_and_shrinks_when_the_channel_is_left() {
	let running = Running::start_with(Options::new("caches-ceiling").caches(tiny())).await;
	let channel_id = running.open_text_channel().await;
	let newest = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.newest_id())
		.expect("a newest message");
	load_older(&running, channel_id, oldest(&running, channel_id)).await;
	load_older(&running, channel_id, oldest(&running, channel_id)).await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the open channel");
		assert_eq!(cache.ordered.len(), CEILING);
		assert!(
			cache.is_detached(),
			"the newest messages made room for older history"
		);
		assert!(cache.newest_id() < Some(newest));
	}

	running.session().send(Command::LoadNewer { channel_id });
	running
		.until("newer pages to reattach the window", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.is_detached() && cache.newest_id() == Some(newest))
		})
		.await;
	assert_eq!(cached(&running.session().state().read(), channel_id), CEILING);

	let (guild_id, other) = text_channels(&running.session().state().read())
		.into_iter()
		.find(|(_, id)| *id != channel_id)
		.expect("another text channel");
	open(&running, guild_id, other).await;
	{
		let state = running.session().state().read();
		let cache = state
			.messages(channel_id)
			.expect("the previous channel stays warm");
		assert_eq!(cache.ordered.len(), WINDOW);
		assert_eq!(cache.newest_id(), Some(newest));
		assert_eq!(cache.viewport(), None);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_visible_range_is_never_evicted() {
	let ceiling = 2 * PAGE + 3;
	let limits = CacheLimits {
		history_ceiling: ceiling,
		..tiny()
	};
	let running = Running::start_with(Options::new("caches-visible").caches(limits)).await;
	let channel_id = running.open_text_channel().await;
	load_older(&running, channel_id, oldest(&running, channel_id)).await;
	let shown = ids(&running.session().state().read(), channel_id);
	let (first, last) = (shown[0], shown[4]);
	running.session().send(Command::SetViewport {
		channel_id,
		viewport: Viewport {
			first,
			last,
			at_bottom: false,
		},
	});
	running
		.eventually("the viewport to reach state", || {
			running
				.session()
				.state()
				.read()
				.messages(channel_id)
				.is_some_and(|cache| cache.viewport().is_some())
		})
		.await;

	for index in 0..ceiling - 2 * PAGE {
		let text = format!("live {index}");
		running.send_and_wait(channel_id, &text, &text).await;
	}
	assert_eq!(cached(&running.session().state().read(), channel_id), ceiling);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "one too many".to_owned(),
		reply_to: None,
		files: Vec::new(),
	});
	running
		.until(
			"the window to detach instead of dropping the visible range",
			|state| {
				state
					.messages(channel_id)
					.is_some_and(|cache| cache.pending.is_empty() && cache.is_detached())
			},
		)
		.await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the open channel");
		assert_eq!(cache.ordered.len(), ceiling);
		assert_eq!(cache.oldest_id(), Some(first));
		assert!(cache.is_resident(first, last));
	}

	load_older(&running, channel_id, first).await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the open channel");
		assert_eq!(cache.ordered.len(), ceiling);
		assert!(cache.is_resident(first, last));
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn member_user_and_profile_caches_stay_capped() {
	let limits = CacheLimits {
		members: 70,
		users: 75,
		profiles: 2,
		warm_channels: 1,
		..tiny()
	};
	let running = Running::start_with(Options::new("caches-people").caches(limits)).await;
	running.connected().await;
	let me = running.fixtures().current_user.id;
	let mut per_guild: Vec<(Snowflake, Snowflake)> = Vec::new();
	for (guild_id, channel_id) in text_channels(&running.session().state().read()) {
		if !per_guild.iter().any(|(seen, _)| *seen == guild_id) {
			per_guild.push((guild_id, channel_id));
		}
	}
	assert!(per_guild.len() > 1);
	for &(guild_id, channel_id) in &per_guild {
		open(&running, guild_id, channel_id).await;
		running
			.until("the member list to arrive", |state| {
				state
					.guild(guild_id)
					.is_some_and(|guild| !guild.member_list.slots.is_empty())
			})
			.await;
	}
	let profiled: Vec<u64> = running
		.fixtures()
		.users
		.keys()
		.copied()
		.filter(|id| Snowflake(*id) != me)
		.take(4)
		.collect();
	for &user_id in &profiled {
		running.session().send(Command::LoadProfile {
			user_id: Snowflake(user_id),
			guild_id: None,
		});
		running
			.until("the profile to load", |state| {
				state.profile(Snowflake(user_id)).is_some()
			})
			.await;
	}

	{
		let state = running.session().state().read();
		let total: usize = state.guilds().map(|guild| guild.members.len()).sum();
		assert!(total <= limits.members, "{total} members cached");
		let users = running
			.fixtures()
			.users
			.keys()
			.filter(|id| state.user(Snowflake(**id)).is_some())
			.count();
		assert!(users <= limits.users, "{users} users cached");
		let profiles = profiled
			.iter()
			.filter(|id| state.profile(Snowflake(**id)).is_some())
			.count();
		assert_eq!(profiles, limits.profiles);
		assert!(state.profile(Snowflake(profiled[3])).is_some());

		let (open_guild, open_channel) = *per_guild.last().expect("an open guild");
		let guild = state.guild(open_guild).expect("the open guild");
		for slot in &guild.member_list.slots {
			if let state::MemberSlot::Member(user_id) = slot {
				assert!(guild.members.contains_key(user_id), "a listed member was evicted");
				assert!(state.user(*user_id).is_some(), "a listed user was evicted");
			}
		}
		for message in &state.messages(open_channel).expect("the open channel").ordered {
			assert!(
				state.user(message.author.id).is_some(),
				"a visible author was evicted"
			);
		}
		assert!(state.user(me).is_some());
		for &(guild_id, _) in &per_guild {
			assert!(
				state
					.guild(guild_id)
					.is_some_and(|guild| guild.members.contains_key(&me))
			);
		}
		for relationship in state.relationships() {
			assert!(state.user(relationship.id).is_some(), "a friend was evicted");
		}
		for channel in state.private_channels() {
			for recipient in &channel.recipient_ids {
				assert!(state.user(*recipient).is_some(), "a DM recipient was evicted");
			}
		}
	}
	running.stop().await;
}
