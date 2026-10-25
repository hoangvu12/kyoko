use std::time::Duration;

use mockcord::fixtures::CURRENT_USER_ID;
use protocol::{ChannelType, Snowflake};
use serde_json::json;
use session::{CacheLimits, Command, RestTuning, Retention};
use state::{MessageRow, MessageVersion, MessageView, ReplyTarget, State};
use store::Store;

use crate::harness::{Options, Running};
use crate::reads::someone_else;

const PRUNE_INTERVAL: Duration = Duration::from_millis(50);

fn quick_deletes() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		delete_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn kept_deleted_ids(store: &Store, channel_id: Snowflake) -> Vec<u64> {
	let mut ids: Vec<u64> = store
		.db
		.kept_deleted(channel_id.get(), 1000)
		.expect("kept deleted messages are readable")
		.into_iter()
		.map(|row| row.id)
		.collect();
	ids.sort_unstable();
	ids
}

fn kept_versions(store: &Store, channel_id: Snowflake) -> Vec<String> {
	store
		.db
		.kept_versions(channel_id.get(), 1000)
		.expect("kept versions are readable")
		.into_iter()
		.filter_map(|row| serde_json::from_slice::<MessageVersion>(&row.json).ok())
		.map(|version| version.content)
		.collect()
}

fn cached_ids(store: &Store, channel_id: Snowflake) -> Vec<u64> {
	store
		.db
		.recent_messages(channel_id.get(), 1000)
		.expect("cached messages are readable")
		.into_iter()
		.map(|(id, _)| id)
		.collect()
}

fn view_of(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<MessageView> {
	state
		.channel_rows(channel_id)
		.into_iter()
		.find_map(|row| match row {
			MessageRow::Message(view) if view.message.id == message_id => Some(*view),
			_ => None,
		})
}

fn has_unread_marker(state: &State, channel_id: Snowflake) -> bool {
	state
		.channel_rows(channel_id)
		.iter()
		.any(|row| matches!(row, MessageRow::UnreadMarker))
}

fn in_window(state: &State, channel_id: Snowflake, message_id: Snowflake) -> bool {
	state
		.messages(channel_id)
		.is_some_and(|cache| cache.by_id.contains_key(&message_id))
}

fn kept_flagged(state: &State, channel_id: Snowflake, message_id: Snowflake) -> bool {
	state.deleted(channel_id, message_id).is_some() && in_window(state, channel_id, message_id)
}

fn history(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Vec<String> {
	state
		.edit_history(channel_id, message_id)
		.iter()
		.map(|version| version.content.clone())
		.collect()
}

fn content_of(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<String> {
	state
		.messages(channel_id)?
		.by_id
		.get(&message_id)
		.map(|message| message.content.clone())
}

async fn post(running: &Running, channel_id: Snowflake, author: u64, content: &str) -> Snowflake {
	let id = Snowflake(
		running
			.mock()
			.post_message(channel_id.get(), author, content)
			.expect("mockcord posts the message"),
	);
	running
		.until("the posted message to arrive", |state| {
			in_window(state, channel_id, id)
		})
		.await;
	id
}

async fn edit(running: &Running, channel_id: Snowflake, message_id: Snowflake, content: &str) {
	assert!(
		running
			.mock()
			.edit_message(channel_id.get(), message_id.get(), content)
	);
	running
		.until("the edit to arrive", |state| {
			content_of(state, channel_id, message_id).as_deref() == Some(content)
		})
		.await;
}

async fn delete_elsewhere(running: &Running, channel_id: Snowflake, message_id: Snowflake) {
	assert!(running.mock().delete_message(channel_id.get(), message_id.get()));
	running
		.until("the deleted message to be kept", |state| {
			kept_flagged(state, channel_id, message_id)
		})
		.await;
}

fn dispatch_reply(running: &Running, channel_id: Snowflake, target: Snowflake) -> Snowflake {
	let guild_id = running.session().state().read().channel_guild(channel_id);
	let id = Snowflake(target.get() + 1);
	running.mock().dispatch(
		"MESSAGE_CREATE",
		&json!({
			"id": id.to_string(),
			"channel_id": channel_id.to_string(),
			"guild_id": guild_id.map(|id| id.to_string()),
			"author": running.fixtures().current_user,
			"content": "a reply",
			"type": 19,
			"timestamp": "2026-01-01T00:00:00Z",
			"mentions": [],
			"message_reference": {
				"message_id": target.to_string(),
				"channel_id": channel_id.to_string(),
			},
		}),
	);
	id
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

async fn select(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until("the channel history to load", |state| {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == channel_id)
				&& state
					.messages(channel_id)
					.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deleted_message_stays_in_place_flagged_and_survives_a_restart() {
	let mut running = Running::start("kept-delete").await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SetViewing(false));
	let author = someone_else(&running);
	let before = post(&running, channel_id, author, "before").await;
	let doomed = post(&running, channel_id, author, "doomed").await;
	running
		.until("the new messages to be unread", |state| {
			has_unread_marker(state, channel_id)
		})
		.await;

	delete_elsewhere(&running, channel_id, doomed).await;
	{
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the channel is cached");
		let position = cache.ordered.iter().position(|message| message.id == doomed);
		assert_eq!(
			position,
			Some(cache.ordered.len() - 1),
			"the deleted message keeps its place"
		);
		assert!(
			view_of(&state, channel_id, doomed).is_none(),
			"hidden until the message log asks for it"
		);
		assert!(view_of(&state, channel_id, before).is_some());
	}

	running.session().send(Command::ShowDeletedMessages(true));
	running
		.until("the deleted message to show", |state| {
			view_of(state, channel_id, doomed).is_some_and(|view| view.deleted.is_some())
		})
		.await;
	assert!(running.session().state().read().shows_deleted_messages());

	assert!(running.mock().delete_message(channel_id.get(), before.get()));
	running
		.until("the unread marker to go with the deleted messages", |state| {
			state.deleted(channel_id, before).is_some() && !has_unread_marker(state, channel_id)
		})
		.await;

	running
		.eventually("the deleted messages to reach the store", || {
			kept_deleted_ids(running.store(), channel_id) == vec![before.get(), doomed.get()]
				&& !cached_ids(running.store(), channel_id).contains(&doomed.get())
		})
		.await;

	running.restart().await;
	let reopened = running.open_text_channel().await;
	assert_eq!(reopened, channel_id);
	running
		.until(
			"the kept messages to be restored in place and rendered",
			|state| {
				kept_flagged(state, channel_id, doomed)
					&& kept_flagged(state, channel_id, before)
					&& state
						.messages(channel_id)
						.is_some_and(|cache| cache.rendered.contains_key(&doomed))
			},
		)
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(content_of(&state, channel_id, doomed).as_deref(), Some("doomed"));
		assert!(
			!state.shows_deleted_messages(),
			"showing deleted messages is the plugin's choice each run"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bulk_delete_keeps_every_message() {
	let running = Running::start("kept-bulk").await;
	let channel_id = running.open_text_channel().await;
	let author = someone_else(&running);
	let mut ids = Vec::new();
	for index in 0..3 {
		ids.push(post(&running, channel_id, author, &format!("bulk {index}")).await);
	}
	let raw: Vec<u64> = ids.iter().map(|id| id.get()).collect();
	assert!(running.mock().bulk_delete(channel_id.get(), &raw));
	running
		.until("every bulk-deleted message to be kept", |state| {
			ids.iter().all(|id| kept_flagged(state, channel_id, *id))
		})
		.await;
	running
		.eventually("the bulk delete to reach the store", || {
			kept_deleted_ids(running.store(), channel_id) == raw
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edits_keep_earlier_versions_in_order_within_the_cap() {
	let limits = CacheLimits {
		kept_versions: 2,
		..CacheLimits::default()
	};
	let retention = Retention {
		kept_versions_per_message: 2,
		interval: PRUNE_INTERVAL,
		..Retention::default()
	};
	let mut running =
		Running::start_with(Options::new("kept-edits").caches(limits).retention(retention)).await;
	let channel_id = running.open_text_channel().await;
	let message_id = post(&running, channel_id, someone_else(&running), "first").await;

	edit(&running, channel_id, message_id, "second").await;
	running
		.until("the first version to be kept", |state| {
			history(state, channel_id, message_id) == ["first"]
		})
		.await;
	edit(&running, channel_id, message_id, "third").await;
	edit(&running, channel_id, message_id, "fourth").await;
	running
		.until("the history to hold the newest two versions", |state| {
			history(state, channel_id, message_id) == ["second", "third"]
		})
		.await;
	{
		let state = running.session().state().read();
		let view = view_of(&state, channel_id, message_id).expect("the edited message is shown");
		assert_eq!(view.message.content, "fourth");
		assert!(view.deleted.is_none());
		assert_eq!(view.versions.len(), 2);
		assert!(view.versions.iter().all(|version| version.replaced_at.is_some()));
		assert!(view.versions[1].replaced_at >= view.versions[0].replaced_at);
	}

	running
		.eventually("the store to keep the newest two versions", || {
			kept_versions(running.store(), channel_id) == ["second", "third"]
				&& cached_ids(running.store(), channel_id).contains(&message_id.get())
		})
		.await;
	running.restart().await;
	running.open_text_channel().await;
	running
		.until("the history to be restored", |state| {
			history(state, channel_id, message_id) == ["second", "third"]
				&& content_of(state, channel_id, message_id).as_deref() == Some("fourth")
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kept_messages_are_capped_per_channel_and_by_age() {
	let limits = CacheLimits {
		kept_deleted: 2,
		..CacheLimits::default()
	};
	let retention = Retention {
		kept_deleted_per_channel: 2,
		interval: PRUNE_INTERVAL,
		..Retention::default()
	};
	let mut running =
		Running::start_with(Options::new("kept-caps").caches(limits).retention(retention)).await;
	let channel_id = running.open_text_channel().await;
	let author = someone_else(&running);
	let mut ids = Vec::new();
	for index in 0..3 {
		ids.push(post(&running, channel_id, author, &format!("capped {index}")).await);
	}
	let edited = ids[2];
	edit(&running, channel_id, edited, "capped 2, edited").await;
	for id in &ids {
		assert!(running.mock().delete_message(channel_id.get(), id.get()));
	}
	running
		.until("only the newest two deletes to be kept", |state| {
			!in_window(state, channel_id, ids[0])
				&& state.deleted(channel_id, ids[0]).is_none()
				&& kept_flagged(state, channel_id, ids[1])
				&& kept_flagged(state, channel_id, ids[2])
		})
		.await;
	running
		.eventually("the store to keep the newest two deletes", || {
			kept_deleted_ids(running.store(), channel_id) == vec![ids[1].get(), ids[2].get()]
				&& kept_versions(running.store(), channel_id) == ["capped 2"]
		})
		.await;

	running
		.restart_with_retention(Retention {
			kept_max_age: Duration::from_millis(1),
			..Retention::default()
		})
		.await;
	running
		.eventually("old kept messages to be pruned", || {
			kept_deleted_ids(running.store(), channel_id).is_empty()
				&& kept_versions(running.store(), channel_id).is_empty()
		})
		.await;
	running.open_text_channel().await;
	{
		let state = running.session().state().read();
		assert!(ids.iter().all(|id| state.deleted(channel_id, *id).is_none()));
		assert!(ids.iter().all(|id| !in_window(&state, channel_id, *id)));
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deletes_and_edits_in_a_cold_channel_reach_the_store() {
	let limits = CacheLimits {
		warm_channels: 1,
		..CacheLimits::default()
	};
	let running = Running::start_with(Options::new("kept-cold").caches(limits)).await;
	running.connected().await;
	let channels = text_channels(&running.session().state().read());
	let [(guild_a, cold), (guild_b, open), ..] = channels[..] else {
		panic!("two text channels in the fixtures");
	};
	select(&running, guild_a, cold).await;
	let (victim, edited, original) = {
		let state = running.session().state().read();
		let cache = state.messages(cold).expect("the channel is cached");
		let [.., earlier, newest] = &cache.ordered[..] else {
			panic!("two cached messages");
		};
		(newest.id, earlier.id, earlier.content.clone())
	};
	running
		.eventually("the history to reach the store", || {
			let cached = cached_ids(running.store(), cold);
			cached.contains(&victim.get()) && cached.contains(&edited.get())
		})
		.await;
	select(&running, guild_b, open).await;
	running
		.until("the first channel to go cold", |state| {
			state.messages(cold).is_none()
		})
		.await;

	assert!(running.mock().delete_message(cold.get(), victim.get()));
	assert!(
		running
			.mock()
			.edit_message(cold.get(), edited.get(), "edited while cold")
	);
	running
		.eventually("the stored messages to be marked deleted and edited", || {
			kept_deleted_ids(running.store(), cold) == vec![victim.get()]
				&& !cached_ids(running.store(), cold).contains(&victim.get())
				&& kept_versions(running.store(), cold) == [original.clone()]
		})
		.await;
	let stored_oldest = cached_ids(running.store(), cold)
		.into_iter()
		.min()
		.map(Snowflake)
		.expect("the cold channel has cached rows");
	select(&running, guild_a, cold).await;
	running
		.until("the fresh page to keep the deleted message in place", |state| {
			state
				.messages(cold)
				.and_then(|cache| cache.oldest_id())
				.is_some_and(|oldest| oldest < stored_oldest)
				&& kept_flagged(state, cold, victim)
				&& history(state, cold, edited) == [original.clone()]
				&& content_of(state, cold, edited).as_deref() == Some("edited while cold")
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn own_deletes_from_kyoko_are_not_kept() {
	let running = Running::start_with(Options::new("kept-own").rest(quick_deletes())).await;
	let channel_id = running.open_text_channel().await;
	let mine = post(&running, channel_id, CURRENT_USER_ID, "regret").await;
	let elsewhere = post(&running, channel_id, CURRENT_USER_ID, "deleted on my phone").await;

	running.session().send(Command::DeleteMessage {
		channel_id,
		message_id: mine,
	});
	running
		.until("the own delete to remove the message", |state| {
			!in_window(state, channel_id, mine)
		})
		.await;
	delete_elsewhere(&running, channel_id, elsewhere).await;
	assert!(
		running
			.session()
			.state()
			.read()
			.deleted(channel_id, mine)
			.is_none()
	);
	running
		.eventually("only the delete from elsewhere to be stored", || {
			kept_deleted_ids(running.store(), channel_id) == vec![elsewhere.get()]
				&& !cached_ids(running.store(), channel_id).contains(&mine.get())
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reply_to_a_kept_deleted_message_resolves_to_the_kept_original() {
	let running = Running::start("kept-reply").await;
	let channel_id = running.open_text_channel().await;
	let original = post(&running, channel_id, someone_else(&running), "the original").await;
	let reply = dispatch_reply(&running, channel_id, original);
	running
		.until("the reply to resolve", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&reply))
				.and_then(|message| state.reply_target(message))
				.is_some_and(|target| matches!(target, ReplyTarget::Message(found) if found.id == original))
		})
		.await;

	delete_elsewhere(&running, channel_id, original).await;
	let target_of = |state: &State| {
		let message = state.messages(channel_id)?.by_id.get(&reply)?;
		Some(match state.reply_target(message)? {
			ReplyTarget::Deleted(found) => format!("deleted: {}", found.content),
			ReplyTarget::Message(found) => found.content.clone(),
			ReplyTarget::Unavailable => "unavailable".to_owned(),
			ReplyTarget::Loading => "loading".to_owned(),
		})
	};
	assert_eq!(
		target_of(&running.session().state().read()).as_deref(),
		Some("unavailable"),
		"without the message log the original reads as deleted"
	);
	running.session().send(Command::ShowDeletedMessages(true));
	running
		.until("the reply to show the kept original", |state| {
			target_of(state).as_deref() == Some("deleted: the original")
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signing_out_leaves_no_kept_messages() {
	let running = Running::start("kept-sign-out").await;
	let channel_id = running.open_text_channel().await;
	let author = someone_else(&running);
	let edited = post(&running, channel_id, author, "draft").await;
	edit(&running, channel_id, edited, "final").await;
	let deleted = post(&running, channel_id, author, "gone").await;
	delete_elsewhere(&running, channel_id, deleted).await;
	let store = running.store();
	running
		.eventually("the kept messages to reach the store", || {
			kept_deleted_ids(store, channel_id) == vec![deleted.get()]
				&& kept_versions(store, channel_id) == ["draft"]
		})
		.await;

	running.session().send(Command::Logout);
	running
		.eventually("sign-out to wipe the kept messages", || {
			kept_deleted_ids(store, channel_id).is_empty()
				&& kept_versions(store, channel_id).is_empty()
				&& cached_ids(store, channel_id).is_empty()
		})
		.await;
	running.stop().await;
}
