use std::collections::BTreeSet;

use bridge::{ChannelEntry, ChannelKind, Sidebar, SidebarRow};
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

fn guild_names(sidebar: &Sidebar) -> BTreeSet<String> {
	sidebar.guilds().iter().map(|guild| guild.name.clone()).collect()
}

fn channel_names(sidebar: &Sidebar) -> Vec<String> {
	sidebar.channels().map(|channel| channel.name.clone()).collect()
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

fn category_names(sidebar: &Sidebar) -> Vec<String> {
	sidebar
		.sections()
		.iter()
		.filter_map(|section| section.category.as_ref())
		.map(|category| category.name.clone())
		.collect()
}

fn visible_names(sidebar: &Sidebar) -> Vec<String> {
	sidebar
		.rows()
		.iter()
		.filter_map(|row| match row {
			SidebarRow::Channel(channel) => Some(channel.name.clone()),
			SidebarRow::Category(_) => None,
		})
		.collect()
}

fn named(sidebar: &Sidebar, name: &str) -> ChannelEntry {
	sidebar
		.channels()
		.find(|channel| channel.name == name)
		.cloned()
		.unwrap_or_else(|| panic!("{name} is listed"))
}

fn first_category(sidebar: &Sidebar) -> Snowflake {
	sidebar
		.sections()
		.iter()
		.find_map(|section| section.category.as_ref())
		.map(|category| category.id)
		.expect("the guild has a category")
}

fn expected_when_collapsed(sidebar: &Sidebar, open: Option<Snowflake>) -> Vec<String> {
	sidebar
		.sections()
		.iter()
		.flat_map(|section| {
			let collapsed = section
				.category
				.as_ref()
				.is_some_and(|category| category.collapsed);
			section
				.channels
				.iter()
				.filter(move |channel| {
					!collapsed || Some(channel.id) == open || channel.unread || !channel.voice.is_empty()
				})
				.map(|channel| channel.name.clone())
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tree_follows_categories_positions_and_threads() {
	let running = Running::start("tree").await;
	let sidebar = &running.view.sidebar;
	assert_eq!(category_names(sidebar), ["Text Channels", "Voice Channels"]);
	let text: Vec<String> = sidebar.sections()[0]
		.channels
		.iter()
		.map(|channel| channel.name.clone())
		.collect();
	assert_eq!(
		text,
		[
			"general",
			"bug triage",
			"weekend plans",
			"random",
			"staff-only",
			"announcements",
			"help-forum",
			"showcase"
		]
	);
	let threads: Vec<(bool, bool)> = sidebar.sections()[0]
		.channels
		.iter()
		.filter(|channel| channel.kind == ChannelKind::Thread)
		.map(|channel| (channel.thread, channel.last_thread))
		.collect();
	assert_eq!(threads, [(true, false), (true, true)]);
	let voice: Vec<(String, ChannelKind, usize)> = sidebar.sections()[1]
		.channels
		.iter()
		.map(|channel| (channel.name.clone(), channel.kind, channel.voice.len()))
		.collect();
	assert_eq!(
		voice,
		[
			("Lounge".to_owned(), ChannelKind::Voice, 2),
			("Gaming".to_owned(), ChannelKind::Voice, 2),
			("Town Hall".to_owned(), ChannelKind::Stage, 0),
		]
	);
	assert_eq!(named(sidebar, "announcements").icon, "megaphone");
	assert_eq!(named(sidebar, "help-forum").icon, "forum");
	assert!(!named(sidebar, "help-forum").kind.opens_timeline());
	assert!(named(sidebar, "Lounge").kind.opens_timeline());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unread_and_mentions_follow_new_messages_live() {
	let mut running = Running::start("unread").await;
	let quiet = running
		.view
		.sidebar
		.channels()
		.find(|channel| {
			channel.kind == ChannelKind::Text && !channel.unread && !channel.muted && !channel.locked
		})
		.cloned()
		.expect("a read text channel");
	assert_eq!(quiet.mentions, 0);
	running
		.server
		.post_message(quiet.id, "<@100> could you look at this")
		.expect("mockcord posts the message");
	running
		.until("the mention to show on the row", |view| {
			view.sidebar.rows().iter().any(|row| {
				matches!(row, SidebarRow::Channel(channel)
					if channel.id == quiet.id && channel.unread && channel.mentions == 1)
			})
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn muted_channels_and_servers_are_flagged() {
	let mut running = Running::start("muted").await;
	let fixtures = running.server.fixtures();
	let first = &fixtures.guilds[0];
	let muted_channel = Snowflake(first.muted_channel_id);
	let muted_guild = Snowflake(fixtures.guilds[1].id);
	let entry = running
		.view
		.sidebar
		.channel(muted_channel)
		.cloned()
		.expect("the muted channel is listed");
	assert!(entry.muted);
	assert!(!running.view.sidebar.header().muted);
	assert!(
		running
			.view
			.sidebar
			.channels()
			.filter(|channel| channel.id != muted_channel)
			.any(|channel| !channel.muted)
	);
	running.select_guild(muted_guild);
	assert!(running.view.sidebar.header().muted);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_collapsed_category_hides_quiet_channels_and_survives_a_restart() {
	let mut running = Running::start("collapse").await;
	let general = named(&running.view.sidebar, "general").id;
	let guild_id = running.view.sidebar.selected().expect("a guild is selected");
	running.view.sidebar.set_open(Some(general));
	let category = first_category(&running.view.sidebar);
	let command = running
		.view
		.sidebar
		.toggle_command(category)
		.expect("a category toggles");
	assert!(
		matches!(command, Command::ToggleCategory { guild_id: g, category_id: c } if g == guild_id && c == category)
	);
	running.session().send(command);
	running
		.until("the category to collapse", |view| {
			view.sidebar.sections()[0]
				.category
				.as_ref()
				.is_some_and(|entry| entry.collapsed)
		})
		.await;
	assert_eq!(
		visible_names(&running.view.sidebar),
		expected_when_collapsed(&running.view.sidebar, Some(general))
	);
	assert!(visible_names(&running.view.sidebar).len() < running.view.sidebar.channels().count());
	let path = format!("/users/@me/guilds/{guild_id}/settings");
	let control = running.server.control();
	let sent = control
		.wait_for(std::time::Duration::from_secs(10), |recording| {
			recording
				.requests_to("PATCH", &path)
				.find_map(|request| request.json())
		})
		.await
		.expect("the collapse is saved");
	assert_eq!(
		sent,
		serde_json::json!({ "channel_overrides": { category.to_string(): { "collapsed": true } } })
	);
	running
		.until_state("the saved settings to echo", |state| {
			state.guild_settings(Some(guild_id)).is_some_and(|settings| {
				settings
					.channel_overrides
					.iter()
					.any(|over| over.channel_id == category && over.collapsed == Some(true))
			})
		})
		.await;
	running.restart().await;
	running
		.until("the category to stay collapsed", |view| {
			view.sidebar
				.sections()
				.iter()
				.filter_map(|section| section.category.as_ref())
				.any(|entry| entry.id == category && entry.collapsed)
		})
		.await;
	assert_eq!(
		visible_names(&running.view.sidebar),
		expected_when_collapsed(&running.view.sidebar, None)
	);
	running.stop().await;
}
