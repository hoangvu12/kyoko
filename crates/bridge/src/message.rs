use std::fmt::Display;

use chrono::{DateTime, Local, TimeZone, Utc};
use markdown::{Document, GuildNavKind, Mentions, Node, TimestampStyle};
use protocol::{Message, MessageType, Poll, Snowflake, User};
use state::{RenderedMessage, ReplyTarget, State};
use url::Url;

pub const UNKNOWN_USER: &str = "@unknown-user";
pub const UNKNOWN_ROLE: &str = "@deleted-role";
pub const UNKNOWN_CHANNEL: &str = "#unknown";
pub const SNIPPET_LENGTH: usize = 120;
pub const ATTACHMENT_BOX: (f32, f32) = (550., 350.);
pub const EMBED_MEDIA_BOX: (f32, f32) = (400., 300.);
pub const EMBED_THUMBNAIL: f32 = 80.;
pub const STICKER_SIDE: f32 = 160.;
const FALLBACK_BOX: (f32, f32) = (300., 200.);
const KIB: f64 = 1024.;
const TRUSTED_HOSTS: [&str; 10] = [
	"discord.com",
	"github.com",
	"youtube.com",
	"youtu.be",
	"twitter.com",
	"x.com",
	"reddit.com",
	"wikipedia.org",
	"spotify.com",
	"twitch.tv",
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Body {
	pub content: Document,
	pub embeds: Vec<EmbedText>,
	pub jumbo: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EmbedText {
	pub description: Option<Document>,
	pub fields: Vec<Document>,
}

impl Body {
	pub fn parse(message: &Message, rendered: Option<&RenderedMessage>) -> Self {
		let content = match rendered {
			Some(rendered) => Document {
				nodes: rendered.ast.clone(),
			},
			None => markdown::parse(&message.content),
		};
		let jumbo = !message.content.is_empty() && content.is_only_emoji();
		let embeds = message
			.embeds
			.iter()
			.map(|embed| EmbedText {
				description: embed.description.as_deref().map(markdown::parse),
				fields: embed
					.fields
					.iter()
					.map(|field| markdown::parse(&field.value))
					.collect(),
			})
			.collect();
		Self {
			content,
			embeds,
			jumbo,
		}
	}

	pub fn documents(&self) -> impl Iterator<Item = &Document> {
		std::iter::once(&self.content).chain(
			self.embeds
				.iter()
				.flat_map(|embed| embed.description.iter().chain(embed.fields.iter())),
		)
	}
}

type RoleName = (String, Option<u32>);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Names {
	users: Vec<(u64, Option<String>)>,
	roles: Vec<(u64, Option<RoleName>)>,
	channels: Vec<(u64, Option<String>)>,
}

impl Names {
	pub fn collect<'a>(
		documents: impl IntoIterator<Item = &'a Document>,
		user: impl Fn(u64) -> Option<String>,
		role: impl Fn(u64) -> Option<RoleName>,
		channel: impl Fn(u64) -> Option<String>,
	) -> Self {
		let mut mentions = Mentions::default();
		for document in documents {
			let found = document.mentions();
			merge(&mut mentions.users, found.users);
			merge(&mut mentions.roles, found.roles);
			merge(&mut mentions.channels, found.channels);
		}
		Self {
			users: mentions.users.into_iter().map(|id| (id, user(id))).collect(),
			roles: mentions.roles.into_iter().map(|id| (id, role(id))).collect(),
			channels: mentions
				.channels
				.into_iter()
				.map(|id| (id, channel(id)))
				.collect(),
		}
	}

	pub fn of<'a>(
		state: &State,
		guild_id: Option<Snowflake>,
		message: &Message,
		documents: impl IntoIterator<Item = &'a Document>,
	) -> Self {
		Self::collect(
			documents,
			|id| user_name(state, guild_id, Snowflake(id), &message.mentions),
			|id| role_name(state, guild_id?, Snowflake(id)),
			|id| state.channel(Snowflake(id))?.name.clone(),
		)
	}

	pub fn user(&self, id: u64) -> String {
		match find(&self.users, id) {
			Some(name) => format!("@{name}"),
			None => UNKNOWN_USER.to_owned(),
		}
	}

	pub fn role(&self, id: u64) -> (String, Option<u32>) {
		match find(&self.roles, id) {
			Some((name, color)) => (format!("@{name}"), *color),
			None => (UNKNOWN_ROLE.to_owned(), None),
		}
	}

	pub fn channel(&self, id: u64) -> String {
		match find(&self.channels, id) {
			Some(name) => format!("#{name}"),
			None => UNKNOWN_CHANNEL.to_owned(),
		}
	}

	pub fn unresolved_users(&self) -> impl Iterator<Item = u64> + '_ {
		self.users
			.iter()
			.filter(|(_, name)| name.is_none())
			.map(|(id, _)| *id)
	}
}

fn merge(into: &mut Vec<u64>, found: Vec<u64>) {
	for id in found {
		if !into.contains(&id) {
			into.push(id);
		}
	}
}

fn find<T>(entries: &[(u64, Option<T>)], id: u64) -> Option<&T> {
	entries
		.iter()
		.find(|(key, _)| *key == id)
		.and_then(|(_, value)| value.as_ref())
}

pub fn user_display(user: &User) -> String {
	user.global_name
		.as_deref()
		.filter(|name| !name.is_empty())
		.unwrap_or(&user.username)
		.to_owned()
}

fn user_name(
	state: &State,
	guild_id: Option<Snowflake>,
	id: Snowflake,
	mentioned: &[User],
) -> Option<String> {
	let member = guild_id
		.and_then(|guild| state.guild(guild))
		.and_then(|guild| guild.members.get(&id));
	if state.user(id).is_some() || member.is_some_and(|member| member.nick.is_some()) {
		return Some(state.display_name(guild_id, id));
	}
	mentioned.iter().find(|user| user.id == id).map(user_display)
}

fn role_name(state: &State, guild_id: Snowflake, id: Snowflake) -> Option<RoleName> {
	let role = state.guild(guild_id)?.roles.get(&id)?;
	let color = u32::try_from(role.color).ok().filter(|color| *color != 0);
	Some((role.name.clone(), color))
}

pub fn person_name(state: &State, guild_id: Option<Snowflake>, user: &User) -> String {
	user_name(state, guild_id, user.id, std::slice::from_ref(user)).unwrap_or_else(|| user_display(user))
}

pub fn snippet(document: &Document, names: &Names, limit: usize) -> String {
	let mut out = String::new();
	write_text(&document.nodes, names, &mut out);
	let flat = out.split_whitespace().collect::<Vec<_>>().join(" ");
	flat.chars().take(limit).collect()
}

fn write_text(nodes: &[Node], names: &Names, out: &mut String) {
	for (index, node) in nodes.iter().enumerate() {
		if index > 0 && (node.is_block() || nodes[index - 1].is_block()) {
			out.push('\n');
		}
		match node {
			Node::Text(text) | Node::Code(text) => out.push_str(text),
			Node::Bold(children)
			| Node::Italic(children)
			| Node::Underline(children)
			| Node::Strike(children)
			| Node::Quote(children)
			| Node::Subtext(children)
			| Node::Heading { children, .. }
			| Node::Link {
				text: Some(children), ..
			} => write_text(children, names, out),
			Node::Spoiler(_) => out.push_str("[spoiler]"),
			Node::CodeBlock { code, .. } => out.push_str(code),
			Node::List { items, .. } => {
				for item in items {
					out.push(' ');
					write_text(item, names, out);
				}
			}
			Node::UserMention(id) => out.push_str(&names.user(*id)),
			Node::RoleMention(id) => out.push_str(&names.role(*id).0),
			Node::ChannelMention(id) => out.push_str(&names.channel(*id)),
			Node::Everyone => out.push_str("@everyone"),
			Node::Here => out.push_str("@here"),
			Node::CustomEmoji { name, .. } => {
				out.push(':');
				out.push_str(name);
				out.push(':');
			}
			Node::Timestamp { unix, style } => out.push_str(&timestamp_text(*unix, *style, Utc::now())),
			Node::CommandMention { name, .. } => {
				out.push('/');
				out.push_str(name);
			}
			Node::GuildNav(kind) => out.push_str(guild_nav_label(*kind)),
			Node::Link { url, text: None, .. } => out.push_str(url),
			Node::LineBreak => out.push('\n'),
		}
	}
}

pub fn guild_nav_label(kind: GuildNavKind) -> &'static str {
	match kind {
		GuildNavKind::Customize => "Customize",
		GuildNavKind::Browse => "Browse Channels",
		GuildNavKind::Guide => "Server Guide",
		GuildNavKind::LinkedRoles => "Linked Roles",
	}
}

pub fn timestamp_text(unix: i64, style: TimestampStyle, now: DateTime<Utc>) -> String {
	timestamp_in(unix, style, now, &Local)
}

pub fn timestamp_in<Z: TimeZone>(unix: i64, style: TimestampStyle, now: DateTime<Utc>, zone: &Z) -> String
where
	Z::Offset: Display,
{
	let Some(time) = DateTime::from_timestamp(unix, 0) else {
		return unix.to_string();
	};
	let local = time.with_timezone(zone);
	match style {
		TimestampStyle::ShortTime => local.format("%-I:%M %p").to_string(),
		TimestampStyle::LongTime => local.format("%-I:%M:%S %p").to_string(),
		TimestampStyle::ShortDate => local.format("%m/%d/%Y").to_string(),
		TimestampStyle::LongDate => local.format("%B %-d, %Y").to_string(),
		TimestampStyle::ShortDateTime => local.format("%B %-d, %Y %-I:%M %p").to_string(),
		TimestampStyle::LongDateTime => local.format("%A, %B %-d, %Y %-I:%M %p").to_string(),
		TimestampStyle::Relative => {
			crate::display::relative_time(unix, now).map_or_else(|| unix.to_string(), |time| time.text)
		}
	}
}

pub fn file_size(bytes: u64) -> String {
	let mut value = bytes as f64;
	let mut unit = 0;
	while value >= KIB && unit < 3 {
		value /= KIB;
		unit += 1;
	}
	let suffix = ["B", "KB", "MB", "GB"][unit];
	if unit == 0 {
		format!("{bytes} {suffix}")
	} else {
		format!("{value:.1} {suffix}")
	}
}

pub fn fit_box(width: Option<i64>, height: Option<i64>, max: (f32, f32)) -> (f32, f32) {
	let (width, height) = match (width, height) {
		(Some(width), Some(height)) if width > 0 && height > 0 => (width as f32, height as f32),
		_ => FALLBACK_BOX,
	};
	let scale = (max.0 / width).min(max.1 / height).min(1.);
	((width * scale).round().max(1.), (height * scale).round().max(1.))
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplyPreview {
	Message(Quote),
	Deleted(Quote),
	Unavailable,
	Loading,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
	pub id: Snowflake,
	pub author: User,
	pub name: String,
	pub text: String,
}

impl Quote {
	pub fn attachment_only(&self) -> bool {
		self.text.is_empty()
	}
}

impl ReplyPreview {
	pub fn from_target(target: ReplyTarget<'_>, readable: bool, quote: impl Fn(&Message) -> Quote) -> Self {
		match target {
			ReplyTarget::Message(message) => Self::Message(quote(message)),
			ReplyTarget::Deleted(message) => Self::Deleted(quote(message)),
			ReplyTarget::Unavailable => Self::Unavailable,
			ReplyTarget::Loading if readable => Self::Loading,
			ReplyTarget::Loading => Self::Unavailable,
		}
	}

	pub fn of(state: &State, guild_id: Option<Snowflake>, message: &Message) -> Option<Self> {
		let target = state.reply_target(message)?;
		let readable = state.can_read_history(message.channel_id);
		Some(Self::from_target(target, readable, |original| {
			let document = markdown::parse(&original.content);
			let names = Names::of(state, guild_id, original, [&document]);
			Quote {
				id: original.id,
				author: original.author.clone(),
				name: person_name(state, guild_id, &original.author),
				text: snippet(&document, &names, SNIPPET_LENGTH),
			}
		}))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemIcon {
	Join,
	Pin,
	Thread,
	Boost,
	Leave,
	Call,
	Rename,
	Picture,
	Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLine {
	pub icon: SystemIcon,
	pub text: String,
}

pub fn system_line(kind: MessageType, name: &str, target: &str, content: &str) -> SystemLine {
	let (icon, text) = match kind {
		MessageType::UserJoin => (SystemIcon::Join, format!("{name} joined the server.")),
		MessageType::ChannelPinnedMessage => (
			SystemIcon::Pin,
			format!("{name} pinned a message to this channel."),
		),
		MessageType::ThreadCreated => (SystemIcon::Thread, format!("{name} started a thread: {content}")),
		MessageType::PremiumGuildSubscription => {
			(SystemIcon::Boost, format!("{name} just boosted the server!"))
		}
		MessageType::PremiumGuildSubscriptionTier1
		| MessageType::PremiumGuildSubscriptionTier2
		| MessageType::PremiumGuildSubscriptionTier3 => {
			let tier = match kind {
				MessageType::PremiumGuildSubscriptionTier1 => 1,
				MessageType::PremiumGuildSubscriptionTier2 => 2,
				_ => 3,
			};
			(
				SystemIcon::Boost,
				format!("{name} just boosted the server! It has reached Level {tier}."),
			)
		}
		MessageType::RecipientAdd => (SystemIcon::Join, format!("{name} added {target} to the group.")),
		MessageType::RecipientRemove => (
			SystemIcon::Leave,
			format!("{name} removed {target} from the group."),
		),
		MessageType::Call => (SystemIcon::Call, format!("{name} started a call.")),
		MessageType::ChannelNameChange => (
			SystemIcon::Rename,
			format!("{name} changed the channel name: {content}"),
		),
		MessageType::ChannelIconChange => (SystemIcon::Picture, format!("{name} changed the group icon.")),
		_ => (SystemIcon::Other, format!("{name} sent a system message.")),
	};
	SystemLine { icon, text }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollSummary {
	pub question: String,
	pub choices: Vec<PollChoice>,
	pub total: i64,
	pub finished: bool,
	pub multi: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollChoice {
	pub id: i64,
	pub text: String,
	pub count: i64,
	pub share: f32,
	pub voted: bool,
}

impl PollSummary {
	pub fn of(poll: &Poll, now: DateTime<Utc>) -> Self {
		let count_of = |id: i64| {
			poll.results
				.as_ref()
				.and_then(|results| results.answer_counts.iter().find(|entry| entry.id == id))
				.map_or((0, false), |entry| (entry.count.max(0), entry.me_voted))
		};
		let total: i64 = poll
			.answers
			.iter()
			.map(|answer| count_of(answer.answer_id.unwrap_or_default()).0)
			.sum();
		let choices = poll
			.answers
			.iter()
			.map(|answer| {
				let id = answer.answer_id.unwrap_or_default();
				let (count, voted) = count_of(id);
				PollChoice {
					id,
					text: answer.poll_media.text.clone().unwrap_or_default(),
					count,
					share: if total > 0 {
						count as f32 / total as f32
					} else {
						0.
					},
					voted,
				}
			})
			.collect();
		Self {
			question: poll.question.text.clone().unwrap_or_default(),
			choices,
			total,
			finished: poll.results.as_ref().is_some_and(|results| results.is_finalized)
				|| poll.expiry.is_some_and(|expiry| expiry <= now),
			multi: poll.allow_multiselect,
		}
	}

	pub fn voted(&self) -> Vec<i64> {
		self.choices
			.iter()
			.filter(|choice| choice.voted)
			.map(|choice| choice.id)
			.collect()
	}

	pub fn hint(&self) -> &'static str {
		if self.finished {
			"Poll closed"
		} else if self.multi {
			"Select one or more answers"
		} else {
			"Select one answer"
		}
	}

	pub fn votes_label(&self) -> String {
		if self.total == 1 {
			"1 vote".to_owned()
		} else {
			format!("{} votes", self.total)
		}
	}
}

pub fn percent(share: f32) -> u32 {
	(share * 100.).round() as u32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
	Open,
	Confirm { host: String },
	Ignore,
}

impl LinkAction {
	pub fn warned(self, warn: bool) -> LinkAction {
		match self {
			LinkAction::Confirm { .. } if !warn => LinkAction::Open,
			action => action,
		}
	}
}

pub fn link_action(url: &str, trusted: &[String]) -> LinkAction {
	let Ok(parsed) = Url::parse(url) else {
		return LinkAction::Ignore;
	};
	match parsed.scheme() {
		"mailto" => return LinkAction::Open,
		"http" | "https" => {}
		_ => return LinkAction::Ignore,
	}
	let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
		return LinkAction::Ignore;
	};
	let known = TRUSTED_HOSTS
		.iter()
		.copied()
		.chain(trusted.iter().map(String::as_str))
		.any(|domain| matches_domain(&host, domain));
	if known {
		LinkAction::Open
	} else {
		LinkAction::Confirm { host }
	}
}

fn matches_domain(host: &str, domain: &str) -> bool {
	host == domain || host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'))
}

pub fn reaction_key(emoji: &protocol::PartialEmoji) -> String {
	state::reaction_name(emoji)
}
