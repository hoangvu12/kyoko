use std::sync::Arc;

use protocol::Snowflake;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Formatting {
	Names,
	RelativeTimes,
	Messages,
	Decorations,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Person {
	pub user_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub username: String,
	pub global_name: Option<String>,
	pub nick: Option<String>,
}

impl Person {
	pub fn name(&self) -> &str {
		[self.nick.as_deref(), self.global_name.as_deref()]
			.into_iter()
			.flatten()
			.find(|name| !name.is_empty())
			.unwrap_or(&self.username)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShownName {
	pub name: String,
	pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelativeTime {
	pub seconds: u64,
	pub past: bool,
	pub unit: &'static str,
	pub value: f64,
	pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeptMessage {
	pub id: Snowflake,
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub author_id: Snowflake,
	pub bot: bool,
	pub mine: bool,
	pub content: String,
	pub deleted_at: Option<i64>,
	pub edited_at: Option<i64>,
	pub versions: Vec<KeptVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeptVersion {
	pub content: String,
	pub edited_at: Option<i64>,
	pub replaced_at: Option<i64>,
	pub attachments: usize,
	pub embeds: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeletedLook {
	Text,
	Tint,
	Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryLook {
	Inline,
	Collapsed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Accessory {
	pub plugin: String,
	pub text: String,
	pub action: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct MessageLook {
	pub deleted: Option<DeletedLook>,
	pub history: Option<HistoryLook>,
	pub accessories: Vec<Accessory>,
}

impl MessageLook {
	pub fn is_plain(&self) -> bool {
		self.deleted.is_none() && self.history.is_none() && self.accessories.is_empty()
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Platforms {
	pub desktop: Option<&'static str>,
	pub mobile: Option<&'static str>,
	pub web: Option<&'static str>,
}

impl Platforms {
	pub fn listed(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
		[
			("desktop", self.desktop),
			("mobile", self.mobile),
			("web", self.web),
		]
		.into_iter()
		.filter_map(|(platform, status)| status.map(|status| (platform, status)))
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecoratedUser {
	pub user_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub bot: bool,
	pub mine: bool,
	pub owner: bool,
	pub status: &'static str,
	pub platforms: Platforms,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Badge {
	pub plugin: String,
	pub key: String,
	pub icon: Option<Arc<str>>,
	pub color: String,
	pub tooltip: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Decorations {
	pub members: Vec<Badge>,
	pub messages: Vec<Badge>,
	pub profiles: Vec<Badge>,
	pub crown: bool,
}

impl Decorations {
	pub fn is_plain(&self) -> bool {
		self.members.is_empty() && self.messages.is_empty() && self.profiles.is_empty() && !self.crown
	}
}

pub trait DisplayHooks: Send + Sync {
	fn formatters(&self, formatting: Formatting) -> Vec<String>;

	fn format_name(&self, person: &Person) -> Option<ShownName>;

	fn format_relative(&self, time: &RelativeTime) -> Option<String>;

	fn render_message(&self, message: &KeptMessage) -> Option<MessageLook>;

	fn decorate_user(&self, user: &DecoratedUser) -> Option<Decorations>;
}
