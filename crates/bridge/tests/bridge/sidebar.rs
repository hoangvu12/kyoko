use std::collections::BTreeSet;

use bridge::{ChannelKind, Sidebar};
use session::Command;

use crate::harness::Running;

fn guild_names(sidebar: &Sidebar) -> BTreeSet<String> {
	sidebar.guilds().iter().map(|guild| guild.name.clone()).collect()
}

fn channel_names(sidebar: &Sidebar) -> Vec<String> {
	sidebar
		.channels()
		.iter()
		.map(|channel| channel.name.clone())
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lists_the_mock_guilds_and_the_first_guilds_channels() {
	let running = Running::start("listing").await;
	let fixtures = running.server.fixtures();
	let expected: BTreeSet<String> = fixtures
		.guilds
		.iter()
		.filter_map(|guild| guild.properties.name.clone())
		.collect();
	assert_eq!(guild_names(&running.view.sidebar), expected);
	let first = running.view.sidebar.guilds()[0].id;
	assert_eq!(running.view.sidebar.selected(), Some(first));
	let guild = fixtures.guild(first.0).expect("the selected guild is a fixture");
	let known: BTreeSet<String> = guild
		.channel_ids
		.iter()
		.chain(&guild.thread_ids)
		.filter_map(|id| fixtures.channels.get(id))
		.filter_map(|channel| channel.name.clone())
		.collect();
	for name in channel_names(&running.view.sidebar) {
		assert!(
			known.contains(&name),
			"{name} is not a channel of the selected guild"
		);
	}
	assert!(
		running
			.view
			.sidebar
			.channels()
			.iter()
			.any(|channel| channel.kind == ChannelKind::Text)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_channel_created_on_the_server_appears_live() {
	let mut running = Running::start("channel").await;
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	let channel_id = running
		.server
		.create_channel(guild_id, "spike-live")
		.expect("mockcord creates the channel");
	running
		.until("the new channel to be listed", |view| {
			view.sidebar
				.channels()
				.iter()
				.any(|channel| channel.id == channel_id && channel.name == "spike-live")
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_guild_created_on_the_server_appears_and_can_be_selected() {
	let mut running = Running::start("guild").await;
	let guild_id = running
		.server
		.create_guild("Spike Guild")
		.expect("mockcord creates the guild");
	running
		.until("the new guild to be listed", |view| {
			view.sidebar
				.guilds()
				.iter()
				.any(|guild| guild.id == guild_id && guild.name == "Spike Guild")
		})
		.await;
	let command = running.select_guild(guild_id);
	assert!(matches!(command, Some(Command::SubscribeGuild { guild_id: id }) if id == guild_id));
	assert_eq!(running.view.sidebar.selected(), Some(guild_id));
	assert_eq!(channel_names(&running.view.sidebar), ["general"]);
	assert!(running.select_guild(guild_id).is_none());
	running.session().send(command.expect("a subscribe command"));
	running.stop().await;
}
