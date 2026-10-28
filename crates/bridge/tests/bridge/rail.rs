use bridge::{RailGuild, RailItem};
use mockcord::fixtures::CURRENT_USER_ID;
use protocol::{ChannelType, Snowflake};
use session::Command;

use crate::harness::Running;

fn guild(rail: &bridge::Rail, guild_id: Snowflake) -> Option<RailGuild> {
	rail.guilds().find(|guild| guild.id == guild_id).cloned()
}

fn quiet_text_channel(running: &Running) -> (Snowflake, Snowflake, u32) {
	let state = running.session().state().clone();
	let state = state.read();
	let selected = running.view.sidebar.selected();
	state
		.guild_summaries()
		.into_iter()
		.filter(|summary| !summary.muted && Some(summary.id) != selected)
		.find_map(|summary| {
			state
				.guild_channel_tree(summary.id)
				.into_iter()
				.flat_map(|node| node.children)
				.find(|row| row.kind == ChannelType::GuildText && !row.locked && !row.muted)
				.map(|row| (summary.id, row.id, summary.mentions))
		})
		.expect("an unmuted guild with a readable text channel")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_rail_lists_guilds_in_layout_order_with_the_mock_folder() {
	let mut running = Running::start("rail-folder").await;
	running
		.until("the rail to fill", |view| !view.rail.items().is_empty())
		.await;
	let folder = running
		.view
		.rail
		.items()
		.iter()
		.find_map(|item| match item {
			RailItem::Folder(folder) => Some(folder.clone()),
			RailItem::Guild(_) => None,
		})
		.expect("the mock folder is in the rail");
	assert_eq!(folder.name, "Mock folder");
	assert_eq!(folder.color, 0x6666ff);
	assert_eq!(folder.guilds.len(), 2);
	let state = running.session().state().clone();
	let order: Vec<Snowflake> = running.view.rail.guilds().map(|guild| guild.id).collect();
	let summaries: Vec<Snowflake> = state
		.read()
		.guild_summaries()
		.iter()
		.map(|summary| summary.id)
		.collect();
	assert_eq!(order, summaries);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mention_shows_on_the_rail_live_and_marking_the_guild_read_clears_it() {
	let mut running = Running::start("rail-live").await;
	running
		.until("the rail to fill", |view| !view.rail.items().is_empty())
		.await;
	let (guild_id, channel_id, before) = quiet_text_channel(&running);
	running
		.server
		.post_message(channel_id, &format!("<@{CURRENT_USER_ID}> look"))
		.expect("mockcord posts the mention");
	running
		.until("the mention badge", |view| {
			guild(&view.rail, guild_id).is_some_and(|guild| guild.mentions == before + 1 && guild.unread)
		})
		.await;
	running.session().send(Command::MarkGuildRead { guild_id });
	running
		.until("the badge and pill to clear", |view| {
			guild(&view.rail, guild_id).is_some_and(|guild| guild.mentions == 0 && !guild.unread)
		})
		.await;
	running.stop().await;
}

fn voice_state(guild_id: Snowflake, channel_id: Option<Snowflake>, user_id: Snowflake) -> serde_json::Value {
	serde_json::json!({
		"guild_id": guild_id.to_string(),
		"channel_id": channel_id.map(|id| id.to_string()),
		"user_id": user_id.to_string(),
		"session_id": "rail",
		"deaf": false,
		"mute": false,
		"self_deaf": false,
		"self_mute": false,
		"self_video": false,
		"suppress": false,
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_rail_follows_who_is_in_voice_in_each_server() {
	let mut running = Running::start("rail-voice").await;
	running
		.until("a server with people in voice", |view| {
			view.rail.guilds().any(|guild| !guild.voice.is_empty())
		})
		.await;
	let guild_id = running
		.view
		.rail
		.guilds()
		.find(|guild| !guild.voice.is_empty())
		.map(|guild| guild.id)
		.expect("a server with people in voice");
	let (mut present, voice_channel) = {
		let state = running.session().state().clone();
		let state = state.read();
		let guild = state.guild(guild_id).expect("the server is in state");
		let present: Vec<Snowflake> = guild.voice_states.keys().copied().collect();
		let channel = state
			.guild_channel_tree(guild_id)
			.into_iter()
			.flat_map(|node| node.children)
			.find(|row| row.kind == ChannelType::GuildVoice)
			.map(|row| row.id)
			.expect("a voice channel");
		(present, channel)
	};
	let mut listed = guild(&running.view.rail, guild_id).expect("the server").voice;
	listed.sort();
	present.sort();
	assert_eq!(listed, present);

	for user_id in &present {
		running
			.server
			.control()
			.dispatch("VOICE_STATE_UPDATE", &voice_state(guild_id, None, *user_id));
	}
	running
		.until("the server to show nobody in voice", |view| {
			guild(&view.rail, guild_id).is_some_and(|guild| guild.voice.is_empty())
		})
		.await;

	let joiner = present[0];
	running.server.control().dispatch(
		"VOICE_STATE_UPDATE",
		&voice_state(guild_id, Some(voice_channel), joiner),
	);
	running
		.until("the joiner to show on the rail", |view| {
			guild(&view.rail, guild_id).is_some_and(|guild| guild.voice == vec![joiner])
		})
		.await;
	running.stop().await;
}
