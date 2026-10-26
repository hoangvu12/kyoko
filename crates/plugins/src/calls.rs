use protocol::{DispatchEvent, Message, Snowflake};
use session::{DeletedLook, HistoryLook, KeptMessage, Person, RankedEmoji, RelativeTime, ShownName};
use state::Expression;

use crate::permission::Scope;

#[derive(Clone, Debug)]
pub(crate) struct MessageView {
	pub id: Snowflake,
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub author_id: Snowflake,
	pub author_name: String,
	pub content: String,
}

impl MessageView {
	pub(crate) fn of(message: &Message) -> MessageView {
		MessageView {
			id: message.id,
			channel_id: message.channel_id,
			guild_id: message.guild_id,
			author_id: message.author.id,
			author_name: message
				.author
				.global_name
				.clone()
				.unwrap_or_else(|| message.author.username.clone()),
			content: message.content.clone(),
		}
	}
}

#[derive(Clone, Debug)]
pub(crate) enum Call {
	BeforeSend {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
		content: String,
	},
	BeforeEdit {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
		message_id: Snowflake,
		content: String,
	},
	AllowTyping {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
	},
	AllowNotification(MessageView),
	FormatName {
		person: Person,
		shown: ShownName,
	},
	FormatRelative(RelativeTime),
	RankEmoji {
		guild_id: Option<Snowflake>,
		query: String,
		emojis: Vec<RankedEmoji>,
	},
	Action {
		key: String,
		context: Vec<(String, String)>,
		target: Option<Expression>,
	},
	SettingsChanged,
	RenderMessage(Box<KeptMessage>),
	ShowDeleted,
}

impl Call {
	pub(crate) fn hook_name(&self) -> &'static str {
		match self {
			Call::BeforeSend { .. } => "before_send",
			Call::BeforeEdit { .. } => "before_edit",
			Call::AllowTyping { .. } => "allow_typing",
			Call::AllowNotification(_) => "allow_notification",
			Call::FormatName { .. } => "format_name",
			Call::FormatRelative(_) => "format_relative",
			Call::RankEmoji { .. } => RANK_EMOJI,
			Call::Action { .. } => "on_action",
			Call::SettingsChanged => "on_settings_changed",
			Call::RenderMessage(_) => RENDER_MESSAGE,
			Call::ShowDeleted => SHOW_DELETED,
		}
	}

	pub(crate) fn is_gesture(&self) -> bool {
		matches!(self, Call::Action { .. })
	}

	pub(crate) fn target(&self) -> Option<Expression> {
		match self {
			Call::Action { target, .. } => target.clone(),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reply {
	Text(Option<String>),
	Name(Option<ShownName>),
	Allow(bool),
	Order(Option<Vec<String>>),
	Look(Option<Look>),
	Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Look {
	pub deleted: Option<DeletedLook>,
	pub history: Option<HistoryLook>,
	pub accessory: Option<(String, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
	Error(String),
	Overrun,
	Busy,
	Gone,
}

impl Fault {
	pub fn describe(&self) -> String {
		match self {
			Fault::Error(error) => format!("it failed with an error: {error}"),
			Fault::Overrun => "it ran past its time limit".to_owned(),
			Fault::Busy => "it fell too far behind".to_owned(),
			Fault::Gone => "it stopped responding".to_owned(),
		}
	}
}

pub(crate) const ON_EVENT: &str = "on_event";
pub(crate) const RANK_EMOJI: &str = "rank_emoji";
pub(crate) const RENDER_MESSAGE: &str = "render_message";
pub(crate) const SHOW_DELETED: &str = "show_deleted";

#[derive(Clone, Debug)]
pub(crate) enum PluginEvent {
	Ready {
		user_id: Snowflake,
	},
	MessageCreate(MessageView),
	MessageUpdate {
		id: Snowflake,
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
		content: Option<String>,
	},
	MessageDelete {
		id: Snowflake,
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
	},
	TypingStart {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
		user_id: Snowflake,
	},
	PresenceUpdate {
		user_id: Snowflake,
		guild_id: Option<Snowflake>,
		status: String,
	},
	MessageAck {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	GuildCreate {
		guild_id: Snowflake,
		name: String,
	},
	GuildDelete {
		guild_id: Snowflake,
	},
}

impl PluginEvent {
	pub(crate) fn of(event: &DispatchEvent) -> Option<PluginEvent> {
		Some(match event {
			DispatchEvent::Ready(ready) => PluginEvent::Ready {
				user_id: ready.user.id,
			},
			DispatchEvent::MessageCreate(message) => PluginEvent::MessageCreate(MessageView::of(message)),
			DispatchEvent::MessageUpdate(update) => PluginEvent::MessageUpdate {
				id: update.id,
				channel_id: update.channel_id,
				guild_id: update.guild_id,
				content: update.content.clone(),
			},
			DispatchEvent::MessageDelete(delete) => PluginEvent::MessageDelete {
				id: delete.id,
				channel_id: delete.channel_id,
				guild_id: delete.guild_id,
			},
			DispatchEvent::TypingStart(typing) => PluginEvent::TypingStart {
				channel_id: typing.channel_id,
				guild_id: typing.guild_id,
				user_id: typing.user_id,
			},
			DispatchEvent::PresenceUpdate(presence) => PluginEvent::PresenceUpdate {
				user_id: presence.user_id.or(presence.user.as_ref().map(|user| user.id))?,
				guild_id: presence.guild_id,
				status: presence.status.as_str().to_owned(),
			},
			DispatchEvent::MessageAck(ack) => PluginEvent::MessageAck {
				channel_id: ack.channel_id,
				message_id: ack.message_id,
			},
			DispatchEvent::GuildCreate(guild) => PluginEvent::GuildCreate {
				guild_id: guild.id,
				name: guild.name.clone().unwrap_or_default(),
			},
			DispatchEvent::GuildDelete(guild) => PluginEvent::GuildDelete { guild_id: guild.id },
			_ => return None,
		})
	}

	pub(crate) fn name(&self) -> &'static str {
		match self {
			PluginEvent::Ready { .. } => "ready",
			PluginEvent::MessageCreate(_) => "message_create",
			PluginEvent::MessageUpdate { .. } => "message_update",
			PluginEvent::MessageDelete { .. } => "message_delete",
			PluginEvent::TypingStart { .. } => "typing_start",
			PluginEvent::PresenceUpdate { .. } => "presence_update",
			PluginEvent::MessageAck { .. } => "message_ack",
			PluginEvent::GuildCreate { .. } => "guild_create",
			PluginEvent::GuildDelete { .. } => "guild_delete",
		}
	}

	pub(crate) fn scope(&self) -> Option<Scope> {
		match self {
			PluginEvent::Ready { .. } => None,
			PluginEvent::MessageCreate(_)
			| PluginEvent::MessageUpdate { .. }
			| PluginEvent::MessageDelete { .. } => Some(Scope::Messages),
			PluginEvent::TypingStart { .. } | PluginEvent::PresenceUpdate { .. } => Some(Scope::Users),
			PluginEvent::MessageAck { .. } => Some(Scope::ReadState),
			PluginEvent::GuildCreate { .. } | PluginEvent::GuildDelete { .. } => Some(Scope::Guilds),
		}
	}

	pub(crate) fn is_direct(&self) -> bool {
		match self {
			PluginEvent::MessageCreate(message) => message.guild_id.is_none(),
			PluginEvent::MessageUpdate { guild_id, .. }
			| PluginEvent::MessageDelete { guild_id, .. }
			| PluginEvent::TypingStart { guild_id, .. } => guild_id.is_none(),
			_ => false,
		}
	}
}
