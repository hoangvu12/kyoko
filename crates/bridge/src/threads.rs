use std::collections::HashSet;
use std::sync::Arc;

use chrono::NaiveDate;
use protocol::{
	Channel, ChannelFlags, ChannelType, ForumSortOrder, ForumTag, Message, Permissions, Snowflake,
};
use state::{ForumPost, State};

use crate::clock::message_time;
use crate::message::{Body, Names, snippet};
use crate::timeline::detached_entry;

pub const PIN_SNIPPET: usize = 260;
pub const FORUM_EXCERPT: usize = 180;
pub const THREAD_NAME_LIMIT: usize = 100;
pub const ARCHIVE_CHOICES: [(u32, &str); 4] = [
	(60, "1 Hour"),
	(1440, "24 Hours"),
	(4320, "3 Days"),
	(10080, "1 Week"),
];
pub const DEFAULT_ARCHIVE_CHOICE: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThreadTab {
	#[default]
	Active,
	Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadEntry {
	pub id: Snowflake,
	pub name: String,
	pub detail: String,
	pub joined: bool,
	pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadHeader {
	pub id: Snowflake,
	pub parent_id: Option<Snowflake>,
	pub name: String,
	pub members: i64,
	pub archived: bool,
	pub joined: bool,
	pub can_manage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinEntry {
	pub id: Snowflake,
	pub author: String,
	pub author_id: Snowflake,
	pub time: String,
	pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagChip {
	pub id: Snowflake,
	pub label: String,
	pub moderated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForumSort {
	#[default]
	Activity,
	Creation,
}

impl ForumSort {
	pub fn label(self) -> &'static str {
		match self {
			Self::Activity => "Recently Active",
			Self::Creation => "Date Posted",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumCard {
	pub id: Snowflake,
	pub title: String,
	pub excerpt: String,
	pub tags: Vec<TagChip>,
	pub author: String,
	pub author_id: Option<Snowflake>,
	pub replies: String,
	pub activity: String,
	pub pinned: bool,
	pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPostForm {
	pub tags: Vec<TagChip>,
	pub require_tag: bool,
}

pub fn is_thread(kind: ChannelType) -> bool {
	matches!(
		kind,
		ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::AnnouncementThread
	)
}

pub fn is_forum(kind: ChannelType) -> bool {
	matches!(kind, ChannelType::GuildForum | ChannelType::GuildMedia)
}

pub fn thread_parent(state: &State, channel_id: Snowflake) -> Option<Snowflake> {
	let channel = state.channel(channel_id)?;
	is_thread(channel.kind).then_some(channel.parent_id).flatten()
}

pub fn can_create_threads(state: &State, channel_id: Snowflake) -> bool {
	state.channel(channel_id).is_some_and(|channel| {
		matches!(
			channel.kind,
			ChannelType::GuildText | ChannelType::GuildAnnouncement
		) && state
			.my_permissions(channel_id)
			.contains(Permissions::CREATE_PUBLIC_THREADS)
	})
}

pub fn messages_label(count: i64) -> String {
	match count {
		1 => "1 message".to_owned(),
		count => format!("{count} messages"),
	}
}

pub fn replies_label(count: i64) -> String {
	match count {
		1 => "1 reply".to_owned(),
		count => format!("{count} replies"),
	}
}

fn archived(thread: &Channel) -> bool {
	thread
		.thread_metadata
		.as_ref()
		.is_some_and(|metadata| metadata.archived)
}

fn owner_name(state: &State, guild_id: Option<Snowflake>, owner: Option<Snowflake>) -> String {
	owner
		.map(|owner| state.display_name(guild_id, owner))
		.unwrap_or_default()
}

fn entry(state: &State, guild_id: Option<Snowflake>, thread: &Channel, today: NaiveDate) -> ThreadEntry {
	let activity = thread.last_message_id.unwrap_or(thread.id).timestamp();
	ThreadEntry {
		id: thread.id,
		name: thread.name.clone().unwrap_or_default(),
		detail: format!(
			"{} - {} - {}",
			owner_name(state, guild_id, thread.owner_id),
			messages_label(thread.message_count.unwrap_or(0)),
			message_time(activity, today)
		),
		joined: thread.member.is_some(),
		archived: archived(thread),
	}
}

pub fn thread_list(
	state: &State,
	channel_id: Snowflake,
	tab: ThreadTab,
	today: NaiveDate,
) -> Vec<ThreadEntry> {
	let guild_id = state.channel_guild(channel_id);
	let mut threads: Vec<&Channel> = match tab {
		ThreadTab::Active => guild_id
			.and_then(|guild_id| state.guild(guild_id))
			.map(|guild| {
				guild
					.threads
					.values()
					.filter(|thread| thread.parent_id == Some(channel_id) && !archived(thread))
					.collect()
			})
			.unwrap_or_default(),
		ThreadTab::Archived => state.archived_threads(channel_id),
	};
	threads.sort_by_key(|thread| std::cmp::Reverse(thread.last_message_id.unwrap_or(thread.id)));
	threads
		.into_iter()
		.map(|thread| entry(state, guild_id, thread, today))
		.collect()
}

pub fn thread_header(state: &State, thread_id: Snowflake) -> Option<ThreadHeader> {
	let channel = state.channel(thread_id)?;
	if !is_thread(channel.kind) {
		return None;
	}
	let me = state.current_user_id();
	let manage = state
		.my_permissions(thread_id)
		.contains(Permissions::MANAGE_THREADS);
	Some(ThreadHeader {
		id: thread_id,
		parent_id: channel.parent_id,
		name: channel.name.clone().unwrap_or_default(),
		members: channel.member_count.unwrap_or(0),
		archived: archived(channel),
		joined: channel.member.is_some(),
		can_manage: manage || (me.is_some() && channel.owner_id == me),
	})
}

fn plain_text(state: &State, guild_id: Option<Snowflake>, message: &Arc<Message>, limit: usize) -> String {
	let body = Body::parse(message, None);
	let names = Names::of(state, guild_id, message, body.documents());
	snippet(&body.content, &names, limit)
}

pub fn pin_entries(state: &State, channel_id: Snowflake, today: NaiveDate) -> Option<Vec<PinEntry>> {
	let guild_id = state.channel_guild(channel_id);
	let pins = state.pins(channel_id)?;
	Some(
		pins.iter()
			.map(|message| {
				let entry = detached_entry(state, guild_id, message);
				let text = if message.content.is_empty() {
					"Click to see attachment".to_owned()
				} else {
					plain_text(state, guild_id, message, PIN_SNIPPET)
				};
				PinEntry {
					id: message.id,
					author: entry.author,
					author_id: message.author.id,
					time: message_time(entry.sent_at, today),
					text,
				}
			})
			.collect(),
	)
}

fn tag_label(tag: &ForumTag) -> String {
	match tag.emoji_name.as_deref().filter(|emoji| !emoji.is_empty()) {
		Some(emoji) => format!("{emoji} {}", tag.name),
		None => tag.name.clone(),
	}
}

pub fn forum_tags(state: &State, forum_id: Snowflake) -> Vec<TagChip> {
	state
		.channel(forum_id)
		.map(|forum| {
			forum
				.available_tags
				.iter()
				.map(|tag| TagChip {
					id: tag.id,
					label: tag_label(tag),
					moderated: tag.moderated,
				})
				.collect()
		})
		.unwrap_or_default()
}

pub fn new_post_form(state: &State, forum_id: Snowflake) -> NewPostForm {
	NewPostForm {
		tags: forum_tags(state, forum_id),
		require_tag: state
			.channel(forum_id)
			.is_some_and(|forum| forum.flags.contains(ChannelFlags::REQUIRE_TAG)),
	}
}

pub fn default_sort(state: &State, forum_id: Snowflake) -> ForumSort {
	match state.channel(forum_id).and_then(|forum| forum.default_sort_order) {
		Some(ForumSortOrder::CreationDate) => ForumSort::Creation,
		_ => ForumSort::Activity,
	}
}

fn card(
	state: &State,
	guild_id: Option<Snowflake>,
	post: &ForumPost,
	tags: &[TagChip],
	today: NaiveDate,
) -> ForumCard {
	let excerpt = state
		.forum_starter(post.id)
		.map(|message| plain_text(state, guild_id, message, FORUM_EXCERPT))
		.unwrap_or_default();
	ForumCard {
		id: post.id,
		title: post.name.clone(),
		excerpt,
		tags: tags
			.iter()
			.filter(|tag| post.tags.contains(&tag.id))
			.cloned()
			.collect(),
		author: owner_name(state, guild_id, post.owner_id),
		author_id: post.owner_id,
		replies: replies_label((post.message_count - 1).max(0)),
		activity: message_time(post.last_activity, today),
		pinned: post.pinned,
		archived: post.archived,
	}
}

pub fn forum_cards(
	state: &State,
	forum_id: Snowflake,
	sort: ForumSort,
	filter: &HashSet<Snowflake>,
	today: NaiveDate,
) -> Vec<ForumCard> {
	let guild_id = state.channel_guild(forum_id);
	let tags = forum_tags(state, forum_id);
	let mut posts: Vec<ForumPost> = state
		.forum_posts(forum_id)
		.into_iter()
		.filter(|post| filter.is_empty() || post.tags.iter().any(|tag| filter.contains(tag)))
		.collect();
	posts.sort_by_key(|post| {
		let key = match sort {
			ForumSort::Activity => post.last_activity,
			ForumSort::Creation => post.created_at,
		};
		(
			std::cmp::Reverse(post.pinned),
			std::cmp::Reverse(key),
			std::cmp::Reverse(post.id),
		)
	});
	posts
		.iter()
		.map(|post| card(state, guild_id, post, &tags, today))
		.collect()
}

pub fn starter_preview(state: &State, channel_id: Snowflake, message_id: Snowflake) -> Option<String> {
	let message = state.messages(channel_id)?.by_id.get(&message_id)?;
	Some(message.content.chars().take(THREAD_NAME_LIMIT).collect())
}

pub fn trimmed_name(text: &str) -> String {
	text.trim().chars().take(THREAD_NAME_LIMIT).collect()
}
