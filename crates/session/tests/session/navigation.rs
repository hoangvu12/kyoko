use protocol::{ChannelType, Snowflake};
use serde_json::{Value, json};
use session::Command;

use crate::harness::Running;

const KEY: &str = "navigation.last_channels";

fn text_channels(running: &Running, guild_id: Snowflake) -> Vec<Snowflake> {
	let state = running.session().state().read();
	let mut channels: Vec<(i64, Snowflake)> = state
		.guild(guild_id)
		.expect("the guild is loaded")
		.channels
		.values()
		.filter(|channel| channel.kind == ChannelType::GuildText)
		.map(|channel| (channel.position.unwrap_or(0), channel.id))
		.collect();
	channels.sort_unstable();
	channels.into_iter().map(|(_, id)| id).collect()
}

fn saved(running: &Running) -> Option<Value> {
	let bytes = running.store().db.kv_get(KEY).ok().flatten()?;
	serde_json::from_slice(&bytes).ok()
}

fn select(running: &Running, guild_id: Snowflake, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_last_viewed_channel_of_each_server_is_saved_and_restored_on_launch() {
	let mut running = Running::start("navigation-last-channels").await;
	running.connected().await;
	let first = Snowflake(running.fixtures().guilds[0].id);
	let second = Snowflake(running.fixtures().guilds[1].id);
	running
		.until("both servers", |state| {
			state.guild(first).is_some() && state.guild(second).is_some()
		})
		.await;
	let first_channels = text_channels(&running, first);
	let second_channels = text_channels(&running, second);
	select(&running, first, first_channels[1]);
	select(&running, second, second_channels[0]);
	let mut pairs = [(first, first_channels[1]), (second, second_channels[0])];
	pairs.sort_unstable();
	let expected = json!(
		pairs
			.iter()
			.map(|(guild_id, channel_id)| [guild_id.to_string(), channel_id.to_string()])
			.collect::<Vec<_>>()
	);
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
	while saved(&running).as_ref() != Some(&expected) {
		assert!(
			std::time::Instant::now() < deadline,
			"each server's channel is saved: {:?}",
			saved(&running)
		);
		tokio::time::sleep(std::time::Duration::from_millis(20)).await;
	}
	let mut garbled = expected.clone();
	garbled
		.as_array_mut()
		.expect("a list")
		.push(json!({"not": "a pair"}));
	running
		.store()
		.db
		.kv_set(KEY, garbled.to_string().as_bytes())
		.expect("the saved list is replaced");
	running.restart().await;
	running
		.until("the saved channels to load", |state| {
			state.last_channel(first).is_some()
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(state.last_channel(first), Some(first_channels[1]));
		assert_eq!(
			state.last_channel(second),
			Some(second_channels[0]),
			"a bad entry on disk leaves the others"
		);
	}
	running.stop().await;
}
