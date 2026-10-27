use bridge::{ChannelKind, OpenChannel, landing_channel};
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

fn text_channels(running: &Running) -> Vec<Snowflake> {
	running
		.view
		.sidebar
		.channels()
		.filter(|channel| channel.kind == ChannelKind::Text && channel.selectable())
		.map(|channel| channel.id)
		.collect()
}

fn landing(running: &Running, guild_id: Snowflake) -> Option<Snowflake> {
	landing_channel(&running.session().state().read(), guild_id).map(|open| open.channel_id)
}

async fn enter(running: &mut Running, guild_id: Snowflake) {
	if let Some(command) = running.select_guild(guild_id) {
		running.session().send(command);
	}
	running
		.until("the server's channels to list", |view| {
			view.sidebar.selected() == Some(guild_id) && view.sidebar.channels().next().is_some()
		})
		.await;
}

async fn view_channel(running: &mut Running, guild_id: Snowflake, channel_id: Snowflake) {
	let command = running.open_channel(channel_id).expect("the channel opens");
	running.session().send(command);
	running
		.until_state("the channel to be remembered", move |state| {
			state.last_channel(guild_id) == Some(channel_id)
		})
		.await;
}

fn guild_with_hidden_channel(running: &Running) -> (Snowflake, Snowflake) {
	let state = running.session().state().read();
	state
		.guilds()
		.find_map(|guild| {
			guild
				.channels
				.values()
				.find(|channel| state.is_hidden(channel.id))
				.map(|channel| (guild.id, channel.id))
		})
		.expect("a channel the user cannot view in the fixtures")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_opens_its_default_channel_first_then_the_last_viewed_one_across_restarts() {
	let mut running = Running::start("landing-last-viewed").await;
	let guild_id = running.view.sidebar.guilds()[0].id;
	enter(&mut running, guild_id).await;
	let channels = text_channels(&running);
	assert!(channels.len() >= 2, "the server has two text channels");
	assert_eq!(
		landing_channel(&running.session().state().read(), guild_id),
		Some(OpenChannel {
			guild_id: Some(guild_id),
			channel_id: channels[0],
		}),
		"a first visit opens the top text channel in sidebar order"
	);
	view_channel(&mut running, guild_id, channels[1]).await;
	assert_eq!(landing(&running, guild_id), Some(channels[1]));
	running.restart().await;
	running
		.until_state("the last viewed channel to load", move |state| {
			state.last_channel(guild_id).is_some()
		})
		.await;
	assert_eq!(
		landing(&running, guild_id),
		Some(channels[1]),
		"the last viewed channel survives a restart"
	);
	let other = running.view.sidebar.guilds()[1].id;
	assert_eq!(
		running.session().state().read().last_channel(other),
		None,
		"each server remembers its own channel"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deleted_last_viewed_channel_falls_back_to_the_default() {
	let mut running = Running::start("landing-deleted").await;
	let guild_id = running.view.sidebar.guilds()[0].id;
	enter(&mut running, guild_id).await;
	let channels = text_channels(&running);
	view_channel(&mut running, guild_id, channels[1]).await;
	running.session().send(Command::DeleteChannel {
		channel_id: channels[1],
	});
	running
		.until("the deleted channel to leave the sidebar", |view| {
			view.sidebar.channel(channels[1]).is_none()
		})
		.await;
	assert_eq!(landing(&running, guild_id), Some(channels[0]));
	running.restart().await;
	let deleted = channels[1];
	running
		.until_state("the live READY without the deleted channel", move |state| {
			state.channel(deleted).is_none()
		})
		.await;
	assert_eq!(
		landing(&running, guild_id),
		Some(channels[0]),
		"a remembered channel that is gone is skipped after a restart too"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_last_viewed_channel_that_became_unreadable_falls_back_to_the_default() {
	let mut running = Running::start("landing-unreadable").await;
	let (guild_id, hidden) = guild_with_hidden_channel(&running);
	enter(&mut running, guild_id).await;
	let channels = text_channels(&running);
	assert!(channels.len() >= 2, "the server has two readable text channels");
	view_channel(&mut running, guild_id, channels[1]).await;
	let update = {
		let state = running.session().state().read();
		let overwrites = &state
			.channel(hidden)
			.expect("the hidden channel")
			.permission_overwrites;
		let mut update = serde_json::to_value(state.channel(channels[1]).expect("the viewed channel"))
			.expect("the channel serializes");
		update["permission_overwrites"] = serde_json::to_value(overwrites).expect("overwrites serialize");
		update["guild_id"] = serde_json::json!(guild_id.to_string());
		update
	};
	running.server.control().dispatch("CHANNEL_UPDATE", &update);
	running
		.until_state("the viewed channel to become unreadable", |state| {
			state.is_hidden(channels[1])
		})
		.await;
	assert_eq!(
		running.session().state().read().last_channel(guild_id),
		Some(channels[1])
	);
	assert_eq!(
		landing(&running, guild_id),
		Some(channels[0]),
		"a channel the user can no longer read is never reopened"
	);
	running.stop().await;
}
