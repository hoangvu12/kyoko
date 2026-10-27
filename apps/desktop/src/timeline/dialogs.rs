use std::rc::Rc;

use bridge::MessageEntry;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};
use zgx::style::ObjectFit;

use super::Pictures;
use super::rows::{Sender, tok};
use crate::kit::{self, Variant};
use crate::motion;

const PREVIEW_AVATAR: f32 = 40.;
const PREVIEW_CHARACTERS: usize = 300;

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
			Self::Delete { .. } => "Are you sure you want to delete this message?",
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
			Self::Delete { .. } | Self::DeleteMany { .. } => "Delete",
			Self::Pin { pin: true, .. } => "Pin",
			Self::Pin { pin: false, .. } => "Unpin",
		}
	}

	fn variant(&self) -> Variant {
		match self {
			Self::Pin { pin: true, .. } => Variant::Primary,
			_ => Variant::Critical,
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
	preview: Signal<Option<Rc<MessageEntry>>>,
	open: Signal<bool>,
	commands: Rc<Sender>,
	pictures: Rc<Pictures>,
}

impl Dialogs {
	pub fn new(runtime: &Runtime, commands: Rc<Sender>, pictures: Rc<Pictures>) -> Self {
		Self {
			pending: runtime.signal(None),
			preview: runtime.signal(None),
			open: runtime.signal(false),
			commands,
			pictures,
		}
	}

	pub fn ask(&self, confirm: Confirm) {
		self.show(confirm, None);
	}

	pub fn ask_about(&self, confirm: Confirm, entry: &MessageEntry) {
		self.show(confirm, Some(Rc::new(entry.clone())));
	}

	fn show(&self, confirm: Confirm, preview: Option<Rc<MessageEntry>>) {
		self.preview.set(preview);
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
		let (shown, open) = (self.pending.clone(), self.open.clone());
		let this = self.clone();
		kit::dialog(motion::modal("Confirm", self.open.clone()))
			.w(tok("modal_width_sm"))
			.child_when(
				move || if open.get() { shown.get() } else { None },
				move |confirm: Confirm, _| this.content(&confirm),
			)
	}

	fn content(self: &Rc<Self>, confirm: &Confirm) -> View {
		let (close, cancel, accept) = (self.clone(), self.clone(), self.clone());
		let preview = self.preview.with_untracked(Clone::clone);
		let deleting = matches!(confirm, Confirm::Delete { .. });
		column()
			.w_full()
			.child(kit::header(kit::title(confirm.title()), None, move || {
				close.close()
			}))
			.child(
				kit::body()
					.child(kit::paragraph(confirm.body()))
					.when_some(preview, |body, entry| body.child(self.preview(&entry)))
					.when(deleting, |body| body.child(protip()))
					.child(kit::footer(vec![
						kit::action("Cancel", Variant::Link, move || cancel.close()),
						kit::action(confirm.confirm_label(), confirm.variant(), move || {
							accept.confirm()
						}),
					])),
			)
	}

	fn preview(&self, entry: &MessageEntry) -> View {
		let snippet: String = entry.content.chars().take(PREVIEW_CHARACTERS).collect();
		let author = match entry.author_color {
			Some(color) => text(entry.author.clone()).text_color(rgb(color)),
			None => text(entry.author.clone()).text_color(tok("text_2")),
		};
		row()
			.w_full()
			.items_start()
			.gap(tok("space_16"))
			.px(tok("space_12"))
			.py(tok("space_8"))
			.rounded(tok("radius_md"))
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.child(self.avatar(entry))
			.child(
				column()
					.grow()
					.min_w(0.)
					.child(
						row()
							.items_center()
							.gap(tok("space_6"))
							.child(
								author
									.text_size(tok("font_size_md"))
									.line_height(tok("line_height_md"))
									.font_weight(500),
							)
							.child(
								text(bridge::message_time(entry.sent_at, bridge::today()))
									.text_size(tok("font_size_xs"))
									.line_height(tok("line_height_md"))
									.font_weight(500)
									.text_color(tok("text_5")),
							),
					)
					.child(
						text(snippet)
							.w_full()
							.text_wrap(true)
							.text_size(tok("font_size_md"))
							.line_height(tok("line_height_md"))
							.text_color(tok("text_3")),
					),
			)
	}

	fn avatar(&self, entry: &MessageEntry) -> View {
		let name = self
			.pictures
			.member_avatar(&entry.avatar, f64::from(PREVIEW_AVATAR), false);
		let picture = self.pictures.picture(&name);
		div()
			.size(PREVIEW_AVATAR, PREVIEW_AVATAR)
			.shrink_0()
			.rounded(PREVIEW_AVATAR / 2.)
			.overflow_hidden()
			.bg(tok("bg_2"))
			.reactive_style(move || match picture.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			})
	}
}

fn protip() -> View {
	row()
		.w_full()
		.items_start()
		.gap(tok("space_4"))
		.child(
			text("PROTIP:")
				.shrink_0()
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.font_weight(700)
				.text_color(tok("green_2")),
		)
		.child(
			text(
				"You can hold down shift when clicking delete message to bypass this confirmation entirely.",
			)
			.grow()
			.min_w(0.)
			.text_wrap(true)
			.text_size(tok("font_size_xs"))
			.line_height(tok("line_height_xs"))
			.text_color(tok("text_4")),
		)
}
