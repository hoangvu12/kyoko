use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
	Messages,
	DirectMessages,
	Users,
	Guilds,
	ReadState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hook {
	Send,
	Edit,
	Typing,
	Notify,
	DisplayName,
	Timestamp,
	EmojiRank,
	MessageRender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
	MarkRead,
	UploadExpression,
	Activity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
	Settings,
	ComposerButtons,
	PanelButtons,
	RailButtons,
	UserDecoration,
	MemberDecoration,
	MessageAccessory,
	MessageMenu,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
	Storage,
	Network(String),
	Ui(Slot),
	Read(Scope),
	Hook(Hook),
	Action(Action),
	Unknown(String),
}

const SCOPES: [(Scope, &str); 5] = [
	(Scope::Messages, "messages"),
	(Scope::DirectMessages, "dms"),
	(Scope::Users, "users"),
	(Scope::Guilds, "guilds"),
	(Scope::ReadState, "read_state"),
];

const HOOKS: [(Hook, &str); 8] = [
	(Hook::Send, "send"),
	(Hook::Edit, "edit"),
	(Hook::Typing, "typing"),
	(Hook::Notify, "notify"),
	(Hook::DisplayName, "display_name"),
	(Hook::Timestamp, "timestamp"),
	(Hook::EmojiRank, "emoji_rank"),
	(Hook::MessageRender, "message_render"),
];

const ACTIONS: [(Action, &str); 3] = [
	(Action::MarkRead, "mark_read"),
	(Action::UploadExpression, "upload_expression"),
	(Action::Activity, "activity"),
];

const SLOTS: [(Slot, &str); 8] = [
	(Slot::Settings, "settings"),
	(Slot::ComposerButtons, "composer.buttons"),
	(Slot::PanelButtons, "panel.buttons"),
	(Slot::RailButtons, "rail.buttons"),
	(Slot::UserDecoration, "user.decoration"),
	(Slot::MemberDecoration, "member.decoration"),
	(Slot::MessageAccessory, "message.accessory"),
	(Slot::MessageMenu, "message.menu"),
];

const HOST_LIMIT: usize = 253;

fn named<T: Copy + PartialEq>(table: &[(T, &'static str)], value: T) -> &'static str {
	table
		.iter()
		.find(|(entry, _)| *entry == value)
		.map_or("", |(_, name)| name)
}

fn lookup<T: Copy>(table: &[(T, &'static str)], name: &str) -> Option<T> {
	table
		.iter()
		.find(|(_, entry)| *entry == name)
		.map(|(value, _)| *value)
}

impl Scope {
	pub fn name(self) -> &'static str {
		named(&SCOPES, self)
	}
}

impl Hook {
	pub fn name(self) -> &'static str {
		named(&HOOKS, self)
	}

	pub fn available(self) -> bool {
		matches!(
			self,
			Hook::Send
				| Hook::Edit | Hook::Typing
				| Hook::Notify
				| Hook::DisplayName
				| Hook::Timestamp
				| Hook::EmojiRank
		)
	}
}

impl Action {
	pub fn name(self) -> &'static str {
		named(&ACTIONS, self)
	}

	pub fn available(self) -> bool {
		matches!(self, Action::MarkRead | Action::UploadExpression)
	}
}

impl Slot {
	pub fn name(self) -> &'static str {
		named(&SLOTS, self)
	}

	pub fn all() -> impl Iterator<Item = Slot> {
		SLOTS.iter().map(|(slot, _)| *slot)
	}

	pub fn available(self) -> bool {
		matches!(
			self,
			Slot::ComposerButtons | Slot::PanelButtons | Slot::RailButtons | Slot::MessageMenu
		)
	}

	pub fn is_menu(self) -> bool {
		self == Slot::MessageMenu
	}

	pub fn parse(name: &str) -> Option<Slot> {
		lookup(&SLOTS, name)
	}
}

impl Permission {
	pub fn parse(text: &str) -> Permission {
		let text = text.trim();
		let parsed = match text.split_once(':') {
			None if text == "storage" => Some(Permission::Storage),
			Some(("network", host)) if is_host(host) => Some(Permission::Network(host.to_owned())),
			Some(("ui", slot)) => lookup(&SLOTS, slot).map(Permission::Ui),
			Some(("read", scope)) => lookup(&SCOPES, scope).map(Permission::Read),
			Some(("hook", hook)) => lookup(&HOOKS, hook).map(Permission::Hook),
			Some(("action", action)) => lookup(&ACTIONS, action).map(Permission::Action),
			_ => None,
		};
		parsed.unwrap_or_else(|| Permission::Unknown(text.to_owned()))
	}

	pub fn name(&self) -> String {
		match self {
			Permission::Storage => "storage".to_owned(),
			Permission::Network(host) => format!("network:{host}"),
			Permission::Ui(slot) => format!("ui:{}", slot.name()),
			Permission::Read(scope) => format!("read:{}", scope.name()),
			Permission::Hook(hook) => format!("hook:{}", hook.name()),
			Permission::Action(action) => format!("action:{}", action.name()),
			Permission::Unknown(text) => text.clone(),
		}
	}

	pub fn describe(&self) -> String {
		match self {
			Permission::Storage => "Keep its own data on this computer.".to_owned(),
			Permission::Network(host) => format!("Contact {host}."),
			Permission::Ui(Slot::Settings) => "Add its own controls to its settings page.".to_owned(),
			Permission::Ui(Slot::MessageMenu) => {
				"Add items to the menu of messages, emoji and stickers.".to_owned()
			}
			Permission::Ui(slot) => format!("Add to the interface at {}.", slot.name()),
			Permission::Read(Scope::Messages) => "Read messages in your servers, as they arrive.".to_owned(),
			Permission::Read(Scope::DirectMessages) => {
				"Include your direct messages in everything it can read.".to_owned()
			}
			Permission::Read(Scope::Users) => {
				"Read names, profiles, presence and typing of people.".to_owned()
			}
			Permission::Read(Scope::Guilds) => "Read your servers, channels and roles.".to_owned(),
			Permission::Read(Scope::ReadState) => "Read which channels are unread.".to_owned(),
			Permission::Hook(Hook::Send) => {
				"Read and rewrite the messages you send, before they leave.".to_owned()
			}
			Permission::Hook(Hook::Edit) => {
				"Read and rewrite your messages when you edit them, before they leave.".to_owned()
			}
			Permission::Hook(Hook::Typing) => "Stop your typing indicator from being sent.".to_owned(),
			Permission::Hook(Hook::Notify) => {
				"Read the messages that would notify you, and silence them.".to_owned()
			}
			Permission::Hook(Hook::DisplayName) => "Change how names are shown.".to_owned(),
			Permission::Hook(Hook::Timestamp) => "Change how relative times are shown.".to_owned(),
			Permission::Hook(Hook::EmojiRank) => {
				"Reorder emoji suggestions, seeing what you type after a colon and your favourite emoji."
					.to_owned()
			}
			Permission::Hook(Hook::MessageRender) => {
				"Show kept deleted and edited versions of messages.".to_owned()
			}
			Permission::Action(Action::MarkRead) => {
				"Mark channels as read when you click its button.".to_owned()
			}
			Permission::Action(Action::UploadExpression) => {
				"Upload an emoji or sticker to a server you pick, when you choose its menu item.".to_owned()
			}
			Permission::Action(Action::Activity) => {
				"Change whether your activity is shared when you click its button.".to_owned()
			}
			Permission::Unknown(text) => {
				format!("Something Kyoko does not know: {text}. It is never granted.")
			}
		}
	}

	pub fn sensitive(&self) -> bool {
		matches!(
			self,
			Permission::Network(_)
				| Permission::Read(Scope::Messages | Scope::DirectMessages)
				| Permission::Hook(Hook::Send | Hook::Edit | Hook::Notify)
				| Permission::Action(_)
		)
	}

	pub fn available(&self) -> bool {
		match self {
			Permission::Storage | Permission::Read(_) => true,
			Permission::Hook(hook) => hook.available(),
			Permission::Action(action) => action.available(),
			Permission::Ui(slot) => slot.available(),
			Permission::Network(_) | Permission::Unknown(_) => false,
		}
	}

	pub fn is_known(&self) -> bool {
		!matches!(self, Permission::Unknown(_))
	}
}

impl fmt::Display for Permission {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.name())
	}
}

fn is_host(host: &str) -> bool {
	!host.is_empty()
		&& host.len() <= HOST_LIMIT
		&& !host.starts_with('.')
		&& !host.ends_with('.')
		&& host.contains('.')
		&& host
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-')
}
