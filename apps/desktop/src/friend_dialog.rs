use std::cell::RefCell;
use std::rc::Rc;

use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};

use crate::kit::{self, Variant};
use crate::motion;
use crate::timeline::tok;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FriendConfirm {
	Remove {
		user_id: Snowflake,
		name: String,
	},
	Block {
		user_id: Snowflake,
		name: String,
		friend: bool,
	},
}

impl FriendConfirm {
	fn title(&self) -> String {
		match self {
			Self::Remove { name, .. } => format!("Remove '{name}'"),
			Self::Block { name, .. } => format!("Block '{name}'?"),
		}
	}

	fn body(&self) -> String {
		match self {
			Self::Remove { name, .. } => {
				format!("Are you sure you want to permanently remove {name} from your friends?")
			}
			Self::Block {
				name, friend: true, ..
			} => format!(
				"Are you sure you want to block {name}? Blocking will also remove them from your friends list."
			),
			Self::Block {
				name, friend: false, ..
			} => format!("Are you sure you want to block {name}?"),
		}
	}

	fn confirm_label(&self) -> &'static str {
		match self {
			Self::Remove { .. } => "Remove Friend",
			Self::Block { .. } => "Block",
		}
	}

	pub fn command(&self) -> Command {
		match *self {
			Self::Remove { user_id, .. } => Command::RemoveRelationship { user_id },
			Self::Block { user_id, .. } => Command::BlockUser { user_id },
		}
	}
}

type Sender = Rc<dyn Fn(Command)>;

pub struct FriendDialog {
	pending: Signal<Option<FriendConfirm>>,
	open: Signal<bool>,
	send: RefCell<Option<Sender>>,
}

impl FriendDialog {
	pub fn new(runtime: &Runtime) -> Rc<Self> {
		Rc::new(Self {
			pending: runtime.signal(None),
			open: runtime.signal(false),
			send: RefCell::new(None),
		})
	}

	pub fn on_command(&self, send: impl Fn(Command) + 'static) {
		*self.send.borrow_mut() = Some(Rc::new(send));
	}

	pub fn ask(&self, confirm: FriendConfirm) {
		self.pending.set(Some(confirm));
		self.open.set(true);
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn escape(&self) -> bool {
		let open = self.is_open();
		self.close();
		open
	}

	fn close(&self) {
		self.open.set(false);
	}

	fn confirm(&self) {
		let send = self.send.borrow().clone();
		if self.is_open()
			&& let Some(confirm) = self.pending.with_untracked(Clone::clone)
			&& let Some(send) = send
		{
			send(confirm.command());
		}
		self.close();
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (shown, open) = (self.pending.clone(), self.open.clone());
		let this = self.clone();
		kit::dialog(motion::modal("Friend confirmation", self.open.clone()))
			.w(tok("modal_width_sm"))
			.child_when(
				move || if open.get() { shown.get() } else { None },
				move |confirm: FriendConfirm, _| this.content(&confirm),
			)
	}

	fn content(self: &Rc<Self>, confirm: &FriendConfirm) -> View {
		let (close, cancel, accept) = (self.clone(), self.clone(), self.clone());
		column()
			.w_full()
			.child(kit::header(kit::title(confirm.title()), None, move || {
				close.close()
			}))
			.child(
				kit::body()
					.child(kit::paragraph(confirm.body()))
					.child(kit::footer(vec![
						kit::action("Cancel", Variant::Link, move || cancel.close())
							.id("friend-confirm-cancel"),
						kit::action(confirm.confirm_label(), Variant::Critical, move || {
							accept.confirm()
						})
						.id("friend-confirm-accept"),
					])),
			)
	}
}
