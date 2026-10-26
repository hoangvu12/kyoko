use bridge::{
	Access, ChannelKind, DeleteProgress, EmojiPicker, MenuContext, MenuEntry, MessageAction, MessageEntry,
	PickerSection, RailIcon, ReactionToggle,
};
use chrono::Utc;
use mockcord::fixtures::CURRENT_USER_ID;
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

const PARTY: &str = "\u{1f389}";
const WAVE: &str = "\u{1f44b}";

async fn open_first_text_channel(running: &mut Running) -> Snowflake {
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	channel_id
}

async fn post_mine(running: &mut Running, channel_id: Snowflake, content: &str) -> Snowflake {
	let id = running
		.server
		.control()
		.post_message(channel_id.get(), CURRENT_USER_ID, content)
		.expect("mockcord posts the message");
	running
		.until("the posted message to show", |view| {
			view.timeline.messages().any(|entry| entry.id.get() == id)
		})
		.await;
	Snowflake(id)
}

fn entry(running: &Running, id: Snowflake) -> MessageEntry {
	running
		.view
		.timeline
		.messages()
		.find(|entry| entry.id == id)
		.cloned()
		.expect("the message is in the timeline")
}

fn someone_elses(running: &Running) -> MessageEntry {
	running
		.view
		.timeline
		.messages()
		.rev()
		.find(|entry| !entry.mine && entry.is_plain() && !entry.pending)
		.cloned()
		.expect("a message from someone else")
}

fn access(running: &Running, channel_id: Snowflake) -> Access {
	let state = running.session().state().clone();
	let guild_id = running.view.timeline.open().and_then(|open| open.guild_id);
	Access::of(&state.read(), guild_id, channel_id, Utc::now())
}

fn actions(menu: &[MenuEntry]) -> Vec<MessageAction> {
	menu.iter()
		.filter_map(|entry| match entry {
			MenuEntry::Action(action) => Some(*action),
			MenuEntry::Separator | MenuEntry::Plugin(_) => None,
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_message_menu_follows_ownership_and_permissions() {
	use MessageAction::*;
	let mut running = Running::start("actions-menu").await;
	let channel_id = open_first_text_channel(&mut running).await;
	let mine = post_mine(&mut running, channel_id, "my own words").await;
	let admin = access(&running, channel_id);
	assert!(admin.manage_messages, "the mock user administers the first guild");
	let own = entry(&running, mine);
	let menu = bridge::message_menu(&own, &admin, MenuContext::default());
	assert_eq!(
		menu,
		vec![
			MenuEntry::Action(React),
			MenuEntry::Separator,
			MenuEntry::Action(Edit),
			MenuEntry::Action(Reply),
			MenuEntry::Action(Forward),
			MenuEntry::Action(CreateThread),
			MenuEntry::Separator,
			MenuEntry::Action(CopyText),
			MenuEntry::Action(Pin),
			MenuEntry::Action(MarkUnread),
			MenuEntry::Action(CopyLink),
			MenuEntry::Separator,
			MenuEntry::Action(Select),
			MenuEntry::Action(Delete),
		],
		"Discord's order for an own message"
	);
	let theirs = someone_elses(&running);
	let moderated = actions(&bridge::message_menu(&theirs, &admin, MenuContext::default()));
	assert_eq!(
		moderated,
		vec![
			React,
			Reply,
			Forward,
			CreateThread,
			CopyText,
			Pin,
			MarkUnread,
			CopyLink,
			Select,
			Delete,
			Report
		],
		"Discord's order for someone else's message, with Manage Messages"
	);
	let member = Access {
		manage_messages: false,
		..admin
	};
	let plain = actions(&bridge::message_menu(&theirs, &member, MenuContext::default()));
	assert!(
		!plain.contains(&Delete) && !plain.contains(&Select) && !plain.contains(&Pin),
		"without Manage Messages: {plain:?}"
	);
	let own_as_member = actions(&bridge::message_menu(&own, &member, MenuContext::default()));
	assert!(own_as_member.contains(&Delete) && own_as_member.contains(&Select));
	let developer = actions(&bridge::message_menu(
		&own,
		&admin,
		MenuContext {
			developer_mode: true,
			selected: true,
			selection: 2,
		},
	));
	assert_eq!(developer.last(), Some(&CopyId), "Copy Message ID comes last");
	assert!(developer.contains(&Deselect) && developer.contains(&DeleteSelected));
	assert_eq!(DeleteSelected.label(2), "Delete 2 Selected");
	assert_eq!(CopyLink.label(0), "Copy Message Link");
	assert_eq!(
		bridge::toolbar_actions(&own, &admin),
		vec![React, Reply, Edit],
		"the hover bar of an own message"
	);
	assert_eq!(bridge::toolbar_actions(&theirs, &admin), vec![React, Reply]);
	let guild_id = admin.guild_id.expect("a guild channel");
	assert_eq!(
		bridge::message_link(admin.guild_id, channel_id, mine),
		format!("https://discord.com/channels/{guild_id}/{channel_id}/{mine}")
	);
	assert_eq!(
		bridge::message_link(None, channel_id, mine),
		format!("https://discord.com/channels/@me/{channel_id}/{mine}")
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reactions_toggle_and_new_ones_need_add_reactions() {
	let mut running = Running::start("actions-reactions").await;
	let channel_id = open_first_text_channel(&mut running).await;
	let target = someone_elses(&running).id;
	running.session().send(Command::AddReaction {
		channel_id,
		message_id: target,
		emoji: PARTY.to_owned(),
		burst: false,
	});
	running
		.until("my reaction to show", |view| {
			view.timeline.messages().any(|entry| {
				entry.id == target
					&& entry
						.message
						.reactions
						.iter()
						.any(|reaction| reaction.me && reaction.emoji.name.as_deref() == Some(PARTY))
			})
		})
		.await;
	let reacted = entry(&running, target);
	assert_eq!(
		bridge::reaction_toggle(&reacted, PARTY),
		ReactionToggle::Remove(PARTY.to_owned())
	);
	let fresh = bridge::reaction_toggle(&reacted, WAVE);
	assert_eq!(fresh, ReactionToggle::Add(WAVE.to_owned()));
	let full = access(&running, channel_id);
	assert!(bridge::reaction_allowed(&reacted, &full, &fresh));
	let muted = Access {
		add_reactions: false,
		..full
	};
	assert!(
		!bridge::reaction_allowed(&reacted, &muted, &fresh),
		"a new emoji needs Add Reactions"
	);
	let joining = ReactionToggle::Add(PARTY.to_owned());
	assert!(
		bridge::reaction_allowed(&reacted, &muted, &joining),
		"joining needs only history"
	);
	let blind = Access {
		read_history: false,
		..full
	};
	assert!(!bridge::reaction_allowed(&reacted, &blind, &joining));
	let picker = EmojiPicker::of(&running.session().state().read(), full.guild_id);
	assert_eq!(
		picker.default_section(),
		PickerSection::Recent,
		"a used emoji is recent"
	);
	let (_, recent) = picker.section(PickerSection::Recent, 0);
	assert_eq!(recent.first().map(|cell| cell.reaction.as_str()), Some(PARTY));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_picker_lists_sections_and_searches() {
	let mut running = Running::start("actions-picker").await;
	open_first_text_channel(&mut running).await;
	let guild_id = running.view.timeline.open().and_then(|open| open.guild_id);
	let picker = EmojiPicker::of(&running.session().state().read(), guild_id);
	let rail = picker.rail();
	let glyphs = rail
		.iter()
		.filter(|entry| matches!(entry.icon, RailIcon::Category(_)))
		.count();
	assert_eq!(glyphs, 8, "one rail entry per unicode category");
	let (title, cells) = picker.section(PickerSection::Unicode(0), 0);
	assert_eq!(title, "People");
	assert!(!cells.is_empty());
	let found = picker.search("wave", 2);
	assert!(
		found.iter().any(|cell| cell.reaction == bridge::toned(WAVE, 2)),
		"search applies the skin tone"
	);
	assert_eq!(bridge::toned(WAVE, 1), "\u{1f44b}\u{1f3fb}");
	assert!(picker.search("   ", 0).is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_progress_summarises_the_latest_batch() {
	let mut running = Running::start("actions-progress").await;
	let channel_id = open_first_text_channel(&mut running).await;
	let first = post_mine(&mut running, channel_id, "first of mine").await;
	let second = post_mine(&mut running, channel_id, "second of mine").await;
	running.session().send(Command::DeleteMessages {
		channel_id,
		ids: vec![first, second],
	});
	running
		.until_state("the batch to finish", |state| {
			DeleteProgress::of(state, channel_id).is_some_and(|progress| progress.finished())
		})
		.await;
	let progress = DeleteProgress::of(&running.session().state().read(), channel_id).expect("a delete batch");
	assert_eq!((progress.total, progress.deleted, progress.refused), (2, 2, 0));
	assert_eq!(progress.summary(), "Deleted 2 of 2");
	running.stop().await;
}
