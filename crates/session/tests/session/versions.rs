use std::sync::Arc;

use mockcord::fixtures::CURRENT_USER_ID;
use mockcord::{Control, Recording};
use parking_lot::Mutex;
use protocol::{ChannelType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;
use state::State;

use crate::harness::{Options, Running};
use crate::reads::tuning;

const LATE_DM: Snowflake = Snowflake(990_000_000_000_000_001);
const LATE_GROUP: Snowflake = Snowflake(990_000_000_000_000_002);
const LATECOMER: u64 = 990_000_000_000_000_100;
const ONE_MESSAGE_LATER: u64 = 1 << 22;

fn identifies(recording: &Recording) -> Vec<Value> {
	recording
		.frames_with(Opcode::Identify)
		.map(|frame| frame.data["client_state"].clone())
		.collect()
}

async fn client_state_of_launch(running: &Running, launch: usize) -> Value {
	running
		.until_wire("an IDENTIFY for this launch", |recording| {
			identifies(recording).get(launch).cloned()
		})
		.await
}

fn read_up_to(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	state
		.read_state(channel_id)
		.and_then(|read_state| read_state.last_message_id)
}

fn text_channels_with_read_states(state: &State) -> Vec<(Snowflake, Snowflake)> {
	state
		.guilds()
		.flat_map(|guild| {
			guild
				.channels
				.values()
				.filter(|channel| channel.kind == ChannelType::GuildText)
				.filter(|channel| read_up_to(state, channel.id).is_some())
				.map(move |channel| (guild.id, channel.id))
		})
		.collect()
}

fn capture_read_state_container(mock: &Control) -> Arc<Mutex<Option<Value>>> {
	let seen = Arc::new(Mutex::new(None));
	let sink = seen.clone();
	mock.transform_ready(move |_, ready| *sink.lock() = Some(ready["read_state"].clone()));
	seen
}

fn channel_row_unread(state: &State, guild_id: Snowflake, channel_id: Snowflake) -> bool {
	state
		.guild_channel_tree(guild_id)
		.iter()
		.flat_map(|node| node.children.iter())
		.any(|row| row.id == channel_id && row.unread)
}

fn guild_unread(state: &State, guild_id: Snowflake) -> bool {
	state
		.guild_summary(guild_id)
		.is_some_and(|summary| summary.unread)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_state_versions_are_persisted_and_sent_on_the_next_launch() {
	let mut running = Running::start_with(Options::new("versions-persisted").rest(tuning())).await;
	let fresh = client_state_of_launch(&running, 0).await;
	assert_eq!(
		fresh,
		json!({ "guild_versions": {} }),
		"a fresh store claims no cache"
	);
	running.connected().await;
	let first = running.session().state().read().versions();
	assert!(first.read_state.is_some() && first.user_guild_settings.is_some());
	let channel_id = {
		let state = running.session().state().read();
		text_channels_with_read_states(&state)
			.into_iter()
			.map(|(_, channel_id)| channel_id)
			.find(|channel_id| {
				let last = state
					.channel(*channel_id)
					.and_then(|channel| channel.last_message_id);
				last > read_up_to(&state, *channel_id)
			})
			.expect("an unread text channel in the fixtures")
	};
	running.session().send(Command::MarkChannelRead { channel_id });
	running
		.until("the ack echo to advance the read-state version", |state| {
			state.versions().read_state > first.read_state
		})
		.await;
	let acked = running.session().state().read().versions();
	let read_to = read_up_to(&running.session().state().read(), channel_id);
	running.restart().await;
	let warm = client_state_of_launch(&running, 1).await;
	assert_eq!(warm["read_state_version"].as_u64(), acked.read_state);
	assert_eq!(
		warm["user_guild_settings_version"].as_u64(),
		acked.user_guild_settings
	);
	assert_eq!(warm["guild_versions"], json!({}));
	running.connected().await;
	assert_eq!(read_up_to(&running.session().state().read(), channel_id), read_to);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_partial_ready_merges_into_the_cached_read_states() {
	let mut running = Running::start("versions-partial").await;
	running.connected().await;
	let channels = text_channels_with_read_states(&running.session().state().read());
	let [(_, kept), (_, changed), ..] = channels[..] else {
		panic!("two text channels with read states in the fixtures");
	};
	let kept_before = read_up_to(&running.session().state().read(), kept);
	let mut posted = None;
	let mut seen = None;
	running
		.restart_after(|mock| {
			posted = mock.post_message(changed.get(), CURRENT_USER_ID, "sent from another device");
			seen = Some(capture_read_state_container(mock));
		})
		.await;
	let posted = Snowflake(posted.expect("mockcord posts the message"));
	let seen = seen.expect("the READY capture is installed");
	running
		.until("the changed channel to update from the partial READY", |state| {
			read_up_to(state, changed) == Some(posted)
		})
		.await;
	let container = seen.lock().clone().expect("a READY was sent");
	assert_eq!(container["partial"], json!(true));
	let entry_ids: Vec<&str> = container["entries"]
		.as_array()
		.expect("read state entries")
		.iter()
		.filter_map(|entry| entry["id"].as_str())
		.collect();
	assert_eq!(entry_ids, vec![changed.to_string().as_str()]);
	assert_eq!(read_up_to(&running.session().state().read(), kept), kept_before);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_ready_replaces_the_cached_read_states() {
	let mut running = Running::start("versions-full").await;
	running.connected().await;
	let channels = text_channels_with_read_states(&running.session().state().read());
	let [(_, dropped), (_, kept), ..] = channels[..] else {
		panic!("two text channels with read states in the fixtures");
	};
	let kept_to = read_up_to(&running.session().state().read(), kept).expect("a read state");
	running
		.restart_after(|mock| {
			mock.transform_ready(move |_, ready| {
				ready["read_state"] = json!({
					"entries": [{ "id": kept.to_string(), "last_message_id": kept_to.to_string(), "mention_count": 0 }],
					"version": 50,
					"partial": false,
				});
			});
		})
		.await;
	running
		.until("the full READY to replace the cache", |state| {
			state.versions().read_state == Some(50)
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(read_up_to(&state, dropped), None);
		assert_eq!(read_up_to(&state, kept), Some(kept_to));
	}
	running.stop().await;
}

fn ack(channel_id: Snowflake, message_id: Snowflake, version: u64) -> Value {
	json!({
		"channel_id": channel_id.to_string(),
		"message_id": message_id.to_string(),
		"version": version,
		"mention_count": 0,
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_older_message_ack_does_not_move_read_state_back() {
	let running = Running::start("versions-ack").await;
	running.connected().await;
	let channels = text_channels_with_read_states(&running.session().state().read());
	let [(_, channel_id), (_, marker), ..] = channels[..] else {
		panic!("two text channels with read states in the fixtures");
	};
	let base = read_up_to(&running.session().state().read(), channel_id).expect("a read state");
	let newer = Snowflake(base.get() + 2 * ONE_MESSAGE_LATER);
	let older = Snowflake(base.get() + ONE_MESSAGE_LATER);
	running
		.mock()
		.dispatch("MESSAGE_ACK", &ack(channel_id, newer, 1_000));
	running
		.until("the newer ack", |state| {
			read_up_to(state, channel_id) == Some(newer)
		})
		.await;
	running
		.mock()
		.dispatch("MESSAGE_ACK", &ack(channel_id, older, 999));
	running.mock().dispatch("MESSAGE_ACK", &ack(marker, newer, 1_001));
	running
		.until("the marker ack", |state| read_up_to(state, marker) == Some(newer))
		.await;
	assert_eq!(
		read_up_to(&running.session().state().read(), channel_id),
		Some(newer)
	);
	running
		.mock()
		.dispatch("MESSAGE_ACK", &ack(channel_id, older, 1_002));
	running
		.until(
			"a newer-version ack to win even when it points earlier",
			|state| read_up_to(state, channel_id) == Some(older),
		)
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_channels_that_arrive_after_ready_join_the_dm_list() {
	let running = Running::start("versions-late-dms").await;
	running.connected().await;
	let before = running.session().state().read().dm_rows().len();
	let known = running
		.session()
		.state()
		.read()
		.private_channels()
		.find_map(|channel| channel.recipient_ids.first().copied())
		.expect("a DM recipient in the fixtures");
	let late_dm = json!({
		"id": LATE_DM.to_string(),
		"type": 1,
		"last_message_id": LATE_DM.to_string(),
		"recipients": [{ "id": LATECOMER.to_string(), "username": "latecomer", "discriminator": "0" }],
	});
	running.mock().dispatch("CHANNEL_CREATE", &late_dm);
	running.mock().dispatch("CHANNEL_CREATE", &late_dm);
	running.mock().dispatch(
		"READY_SUPPLEMENTAL",
		&json!({
			"lazy_private_channels": [{
				"id": LATE_GROUP.to_string(),
				"type": 3,
				"name": "late group",
				"recipient_ids": [known.to_string()],
			}],
		}),
	);
	running
		.until("both late private channels", |state| {
			let rows = state.dm_rows();
			rows.iter().any(|row| row.channel_id == LATE_DM)
				&& rows.iter().any(|row| row.channel_id == LATE_GROUP)
		})
		.await;
	{
		let state = running.session().state().read();
		let rows = state.dm_rows();
		assert_eq!(rows.len(), before + 2, "the repeated CHANNEL_CREATE is ignored");
		let dm = rows
			.iter()
			.find(|row| row.channel_id == LATE_DM)
			.expect("the late DM");
		assert_eq!(dm.recipients, vec![Snowflake(LATECOMER)]);
		assert_eq!(dm.name, "latecomer");
		assert!(
			state.user(Snowflake(LATECOMER)).is_some(),
			"the recipient is resolved"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_channel_unread_update_marks_a_closed_guild_unread() {
	let running = Running::start_with(Options::new("versions-unread-update").rest(tuning())).await;
	running.connected().await;
	let (guild_id, channel_id) = {
		let state = running.session().state().read();
		let open_guild = state.selected().and_then(|selected| selected.guild_id);
		text_channels_with_read_states(&state)
			.into_iter()
			.find(|(guild_id, _)| Some(*guild_id) != open_guild)
			.expect("a text channel in a guild that isn't open")
	};
	running.session().send(Command::MarkGuildRead { guild_id });
	running
		.until("the guild to be read", |state| !guild_unread(state, guild_id))
		.await;
	let last = running
		.session()
		.state()
		.read()
		.channel(channel_id)
		.and_then(|channel| channel.last_message_id)
		.unwrap_or_default();
	running.mock().dispatch(
		"CHANNEL_UNREAD_UPDATE",
		&json!({
			"guild_id": guild_id.to_string(),
			"channel_unread_updates": [
				{ "id": channel_id.to_string(), "last_message_id": (last.get() + ONE_MESSAGE_LATER).to_string() },
				{ "id": "not a snowflake" },
			],
		}),
	);
	running
		.until("the channel and guild to show unread", |state| {
			channel_row_unread(state, guild_id, channel_id) && guild_unread(state, guild_id)
		})
		.await;
	{
		let state = running.session().state().read();
		let others_unread = state
			.guild_channel_tree(guild_id)
			.iter()
			.flat_map(|node| node.children.iter())
			.any(|row| row.id != channel_id && row.unread);
		assert!(!others_unread, "only the updated channel turns unread");
	}
	running.stop().await;
}
