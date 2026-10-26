mod dialogs;
mod kit;
mod menu;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{DEFAULT_TIMEOUT, ModerationItem};
use protocol::{ChannelType, Snowflake};
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};

pub use self::dialogs::SLOWMODES;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug)]
pub enum Request {
	Command(Command),
	Profile(Snowflake, (f32, f32)),
	Message(Snowflake),
	CopyId(Snowflake),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
	DeleteChannel(Snowflake),
	DeleteRole {
		guild_id: Snowflake,
		role_id: Snowflake,
	},
	Unban {
		guild_id: Snowflake,
		user_id: Snowflake,
	},
	RevokeInvite {
		guild_id: Snowflake,
		code: String,
	},
	DeleteEmoji {
		guild_id: Snowflake,
		emoji_id: Snowflake,
	},
	DeleteSticker {
		guild_id: Snowflake,
		sticker_id: Snowflake,
	},
}

impl ConfirmAction {
	fn command(&self) -> Command {
		match self.clone() {
			Self::DeleteChannel(channel_id) => Command::DeleteChannel { channel_id },
			Self::DeleteRole { guild_id, role_id } => Command::DeleteRole { guild_id, role_id },
			Self::Unban { guild_id, user_id } => Command::UnbanMember { guild_id, user_id },
			Self::RevokeInvite { guild_id, code } => Command::RevokeInvite { guild_id, code },
			Self::DeleteEmoji { guild_id, emoji_id } => Command::DeleteEmoji { guild_id, emoji_id },
			Self::DeleteSticker { guild_id, sticker_id } => Command::DeleteSticker { guild_id, sticker_id },
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
	pub title: String,
	pub body: String,
	pub label: &'static str,
	pub danger: bool,
	pub action: ConfirmAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDraft {
	pub channel_id: Snowflake,
	pub kind: ChannelType,
	pub name: String,
	pub topic: String,
	pub slowmode: u32,
	pub nsfw: bool,
	pub parent_id: Option<Snowflake>,
	pub categories: Vec<(Snowflake, String)>,
}

#[derive(Clone)]
pub enum Dialog {
	Nickname {
		guild_id: Snowflake,
		user_id: Snowflake,
		value: Signal<String>,
	},
	Timeout {
		guild_id: Snowflake,
		user_id: Snowflake,
		name: String,
		choice: Signal<usize>,
		reason: Signal<String>,
	},
	Kick {
		guild_id: Snowflake,
		user_id: Snowflake,
		name: String,
		reason: Signal<String>,
	},
	Ban {
		guild_id: Snowflake,
		user_id: Snowflake,
		name: String,
		history: Signal<usize>,
		reason: Signal<String>,
	},
	Confirm(Confirm),
	CreateChannel {
		guild_id: Snowflake,
		parent_id: Option<Snowflake>,
		kind: Signal<usize>,
		name: Signal<String>,
	},
	EditSticker {
		guild_id: Snowflake,
		sticker_id: Snowflake,
		name: Signal<String>,
		description: Signal<String>,
		tags: Signal<String>,
	},
	EditChannel {
		draft: Rc<ChannelDraft>,
		name: Signal<String>,
		topic: Signal<String>,
		slowmode: Signal<usize>,
		nsfw: Signal<bool>,
		parent: Signal<Option<Snowflake>>,
	},
}

#[derive(Clone)]
struct Shown {
	serial: u64,
	dialog: Rc<Dialog>,
	open: Signal<bool>,
}

impl PartialEq for Shown {
	fn eq(&self, other: &Self) -> bool {
		self.serial == other.serial
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberMenu {
	pub guild_id: Snowflake,
	pub user_id: Snowflake,
	pub name: String,
	pub nick: String,
	pub head: bool,
	pub developer: bool,
	pub items: Vec<ModerationItem>,
}

#[derive(Clone)]
struct MenuShown {
	serial: u64,
	at: (f32, f32),
	menu: Rc<MemberMenu>,
	open: Signal<bool>,
}

impl PartialEq for MenuShown {
	fn eq(&self, other: &Self) -> bool {
		self.serial == other.serial
	}
}

pub struct ModerationLayer {
	runtime: Runtime,
	window: Signal<(f32, f32)>,
	shown: Signal<Option<Shown>>,
	menu: Signal<Option<MenuShown>>,
	serials: Cell<u64>,
	requests: Hook<Request>,
}

impl ModerationLayer {
	pub fn new(runtime: &Runtime) -> Rc<Self> {
		Rc::new(Self {
			runtime: runtime.clone(),
			window: runtime.signal((0., 0.)),
			shown: runtime.signal(None),
			menu: runtime.signal(None),
			serials: Cell::new(0),
			requests: RefCell::new(None),
		})
	}

	pub fn on_request(&self, hook: impl Fn(Request) + 'static) {
		*self.requests.borrow_mut() = Some(Box::new(hook));
	}

	fn request(&self, request: Request) {
		if let Some(hook) = self.requests.borrow().as_ref() {
			hook(request);
		}
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	fn serial(&self) -> u64 {
		let serial = self.serials.get() + 1;
		self.serials.set(serial);
		serial
	}

	pub fn open(&self, dialog: Dialog) {
		self.close_menu();
		let shown = Shown {
			serial: self.serial(),
			dialog: Rc::new(dialog),
			open: self.runtime.signal(true),
		};
		self.shown.set(Some(shown));
	}

	pub fn nickname(&self, guild_id: Snowflake, user_id: Snowflake, current: &str) {
		self.open(Dialog::Nickname {
			guild_id,
			user_id,
			value: self.runtime.signal(current.to_owned()),
		});
	}

	pub fn timeout(&self, guild_id: Snowflake, user_id: Snowflake, name: &str) {
		self.open(Dialog::Timeout {
			guild_id,
			user_id,
			name: name.to_owned(),
			choice: self.runtime.signal(DEFAULT_TIMEOUT),
			reason: self.runtime.signal(String::new()),
		});
	}

	pub fn kick(&self, guild_id: Snowflake, user_id: Snowflake, name: &str) {
		self.open(Dialog::Kick {
			guild_id,
			user_id,
			name: name.to_owned(),
			reason: self.runtime.signal(String::new()),
		});
	}

	pub fn ban(&self, guild_id: Snowflake, user_id: Snowflake, name: &str) {
		self.open(Dialog::Ban {
			guild_id,
			user_id,
			name: name.to_owned(),
			history: self.runtime.signal(0),
			reason: self.runtime.signal(String::new()),
		});
	}

	pub fn confirm(&self, confirm: Confirm) {
		self.open(Dialog::Confirm(confirm));
	}

	pub fn create_channel(&self, guild_id: Snowflake, parent_id: Option<Snowflake>) {
		self.open(Dialog::CreateChannel {
			guild_id,
			parent_id,
			kind: self.runtime.signal(0),
			name: self.runtime.signal(String::new()),
		});
	}

	pub fn edit_channel(&self, draft: ChannelDraft) {
		let slowmode = SLOWMODES
			.iter()
			.position(|(_, seconds)| *seconds >= draft.slowmode)
			.unwrap_or(0);
		self.open(Dialog::EditChannel {
			name: self.runtime.signal(draft.name.clone()),
			topic: self.runtime.signal(draft.topic.clone()),
			slowmode: self.runtime.signal(slowmode),
			nsfw: self.runtime.signal(draft.nsfw),
			parent: self.runtime.signal(draft.parent_id),
			draft: Rc::new(draft),
		});
	}

	pub fn edit_sticker(&self, guild_id: Snowflake, sticker: &protocol::Sticker) {
		self.open(Dialog::EditSticker {
			guild_id,
			sticker_id: sticker.id,
			name: self.runtime.signal(sticker.name.clone()),
			description: self
				.runtime
				.signal(sticker.description.clone().unwrap_or_default()),
			tags: self.runtime.signal(sticker.tags.clone().unwrap_or_default()),
		});
	}

	pub fn open_menu(&self, menu: MemberMenu, at: (f32, f32)) -> bool {
		if !menu.head && menu.items.is_empty() {
			return false;
		}
		let (width, height) = self.window.with_untracked(|window| *window);
		let size = menu::size(&menu);
		let x = (at.0 - kit::MENU_BORDER)
			.min(width - kit::MENU_MARGIN - size.0)
			.max(0.);
		let y = (at.1 - kit::MENU_BORDER)
			.min(height - kit::MENU_MARGIN - size.1)
			.max(0.);
		self.menu.set(Some(MenuShown {
			serial: self.serial(),
			at: (x, y),
			menu: Rc::new(menu),
			open: self.runtime.signal(true),
		}));
		true
	}

	pub fn close_menu(&self) {
		if let Some(shown) = self.menu.with_untracked(Clone::clone) {
			shown.open.set(false);
		}
	}

	fn run_item(&self, menu: &MemberMenu, item: &ModerationItem) {
		let (guild_id, user_id) = (menu.guild_id, menu.user_id);
		match item {
			ModerationItem::Nickname => self.nickname(guild_id, user_id, &menu.nick),
			ModerationItem::Timeout => self.timeout(guild_id, user_id, &menu.name),
			ModerationItem::RemoveTimeout => self.request(Request::Command(Command::TimeoutMember {
				guild_id,
				user_id,
				until: None,
				reason: None,
			})),
			ModerationItem::Kick => self.kick(guild_id, user_id, &menu.name),
			ModerationItem::Ban => self.ban(guild_id, user_id, &menu.name),
			ModerationItem::ServerMute(muted) => self.request(Request::Command(Command::ServerMuteMember {
				guild_id,
				user_id,
				muted: !muted,
			})),
			ModerationItem::ServerDeafen(deafened) => {
				self.request(Request::Command(Command::ServerDeafenMember {
					guild_id,
					user_id,
					deafened: !deafened,
				}));
			}
			ModerationItem::Separator | ModerationItem::Roles(_) | ModerationItem::MoveTo(_) => {}
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		div()
			.absolute()
			.left(0.)
			.top(0.)
			.w(0.)
			.h(0.)
			.child(self.menu_view())
			.child(self.dialog_view())
	}

	fn menu_view(self: &Rc<Self>) -> View {
		let this = self.clone();
		let menu = self.menu.clone();
		div().child_when(
			move || menu.get(),
			move |shown: MenuShown, _| menu::view(&this, &shown.menu, shown.at, shown.open.clone()),
		)
	}

	fn dialog_view(self: &Rc<Self>) -> View {
		let this = self.clone();
		let shown = self.shown.clone();
		div().child_when(
			move || shown.get(),
			move |shown: Shown, _| dialogs::view(&this, &shown.dialog, shown.open.clone()),
		)
	}
}
