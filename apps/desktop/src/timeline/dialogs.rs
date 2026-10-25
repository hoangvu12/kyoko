use std::rc::Rc;

use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};

use super::rows::{Sender, tok};

const DIALOG_WIDTH: f32 = 440.;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
	Delete {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	DeleteMany {
		channel_id: Snowflake,
		ids: Vec<Snowflake>,
	},
	Pin {
		channel_id: Snowflake,
		message_id: Snowflake,
		pin: bool,
	},
}

impl Confirm {
	fn title(&self) -> String {
		match self {
			Self::Delete { .. } => "Delete Message".to_owned(),
			Self::DeleteMany { ids, .. } => format!("Delete {} messages?", ids.len()),
			Self::Pin { pin: true, .. } => "Pin this message?".to_owned(),
			Self::Pin { pin: false, .. } => "Unpin this message?".to_owned(),
		}
	}

	fn body(&self) -> &'static str {
		match self {
			Self::Delete { .. } => "Are you sure you want to delete this message? This cannot be undone.",
			Self::DeleteMany { .. } => {
				"The selected messages are removed for everyone. This cannot be undone."
			}
			Self::Pin { pin: true, .. } => {
				"The message will be pinned to this channel and visible to everyone."
			}
			Self::Pin { pin: false, .. } => "The message will no longer be pinned to this channel.",
		}
	}

	fn confirm_label(&self) -> &'static str {
		match self {
			Self::Delete { .. } | Self::DeleteMany { .. } => "Delete Message",
			Self::Pin { pin: true, .. } => "Pin",
			Self::Pin { pin: false, .. } => "Unpin",
		}
	}

	fn confirm_color(&self) -> &'static str {
		match self {
			Self::Pin { pin: true, .. } => "brand",
			_ => "danger",
		}
	}

	pub fn command(&self) -> Command {
		match self.clone() {
			Self::Delete {
				channel_id,
				message_id,
			} => Command::DeleteMessage {
				channel_id,
				message_id,
			},
			Self::DeleteMany { channel_id, ids } => Command::DeleteMessages { channel_id, ids },
			Self::Pin {
				channel_id,
				message_id,
				pin: true,
			} => Command::PinMessage {
				channel_id,
				message_id,
			},
			Self::Pin {
				channel_id,
				message_id,
				pin: false,
			} => Command::UnpinMessage {
				channel_id,
				message_id,
			},
		}
	}
}

pub struct Dialogs {
	pending: Signal<Option<Confirm>>,
	open: Signal<bool>,
	commands: Rc<Sender>,
}

impl Dialogs {
	pub fn new(runtime: &Runtime, commands: Rc<Sender>) -> Self {
		Self {
			pending: runtime.signal(None),
			open: runtime.signal(false),
			commands,
		}
	}

	pub fn ask(&self, confirm: Confirm) {
		self.pending.set(Some(confirm));
		self.open.set(true);
	}

	fn close(&self) {
		self.open.set(false);
	}

	fn confirm(&self) {
		if let Some(confirm) = self.pending.with_untracked(Clone::clone)
			&& let Some(send) = self.commands.borrow().as_ref()
		{
			send(confirm.command());
		}
		self.close();
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (title, body, label, color) = (
			self.pending.clone(),
			self.pending.clone(),
			self.pending.clone(),
			self.pending.clone(),
		);
		let (cancel, confirm) = (self.clone(), self.clone());
		modal("Confirm", self.open.clone())
			.w(DIALOG_WIDTH)
			.p(16.)
			.gap(12.)
			.rounded(tok("radius_md"))
			.bg(tok("background_primary"))
			.border(1.)
			.border_color(tok("divider"))
			.child(
				text_signal(move || {
					title.with(|pending| pending.as_ref().map(Confirm::title).unwrap_or_default())
				})
				.text_size(tok("font_lg"))
				.font_weight(700)
				.text_color(tok("header_primary")),
			)
			.child(
				text_signal(move || {
					body.with(|pending| pending.as_ref().map(Confirm::body).unwrap_or_default().to_owned())
				})
				.text_wrap(true)
				.text_size(tok("font_sm"))
				.text_color(tok("text_normal")),
			)
			.child(
				row()
					.justify_end()
					.gap(8.)
					.child(dialog_button(text("Cancel"), move || cancel.close()).bg(tok("button_secondary")))
					.child(
						dialog_button(
							text_signal(move || {
								label.with(|pending| {
									pending
										.as_ref()
										.map(Confirm::confirm_label)
										.unwrap_or_default()
										.to_owned()
								})
							}),
							move || confirm.confirm(),
						)
						.reactive_style(move || {
							let color = color
								.with(|pending| pending.as_ref().map_or("danger", Confirm::confirm_color));
							Styles::new().bg(tok(color))
						}),
					),
			)
	}
}

fn dialog_button(label: View, action: impl FnMut() + 'static) -> View {
	button()
		.h(32.)
		.px(16.)
		.rounded(tok("radius_sm"))
		.text_size(tok("font_sm"))
		.font_weight(500)
		.text_color(tok("text_on_brand"))
		.cursor(Cursor::Pointer)
		.child(label)
		.on_click(action)
}
