use std::rc::Rc;

use bridge::{BAN_HISTORY_CHOICES, NICKNAME_LIMIT, REASON_LIMIT, TIMEOUT_CHOICES, trimmed};
use chrono::{TimeDelta, Utc};
use protocol::{ChannelType, Snowflake};
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::kit::{self, button, chips, field, footer, hint, input, label};
use super::{Dialog, ModerationLayer, Request};
use crate::motion;
use crate::timeline::tok;

const WIDTH: f32 = 440.;
const WIDE: f32 = 540.;
const NAME_LIMIT: usize = 100;
const TOPIC_LIMIT: usize = 1024;

pub const SLOWMODES: [(&str, u32); 14] = [
	("Off", 0),
	("5s", 5),
	("10s", 10),
	("15s", 15),
	("30s", 30),
	("1m", 60),
	("2m", 120),
	("5m", 300),
	("10m", 600),
	("15m", 900),
	("30m", 1800),
	("1h", 3600),
	("2h", 7200),
	("6h", 21_600),
];

pub const CHANNEL_KINDS: [(&str, ChannelType); 5] = [
	("Text", ChannelType::GuildText),
	("Voice", ChannelType::GuildVoice),
	("Announcement", ChannelType::GuildAnnouncement),
	("Forum", ChannelType::GuildForum),
	("Stage", ChannelType::GuildStageVoice),
];

const TIMEOUT_LABELS: [&str; 6] = [
	TIMEOUT_CHOICES[0].0,
	TIMEOUT_CHOICES[1].0,
	TIMEOUT_CHOICES[2].0,
	TIMEOUT_CHOICES[3].0,
	TIMEOUT_CHOICES[4].0,
	TIMEOUT_CHOICES[5].0,
];
const HISTORY_LABELS: [&str; 7] = [
	BAN_HISTORY_CHOICES[0].0,
	BAN_HISTORY_CHOICES[1].0,
	BAN_HISTORY_CHOICES[2].0,
	BAN_HISTORY_CHOICES[3].0,
	BAN_HISTORY_CHOICES[4].0,
	BAN_HISTORY_CHOICES[5].0,
	BAN_HISTORY_CHOICES[6].0,
];
const KIND_LABELS: [&str; 5] = [
	CHANNEL_KINDS[0].0,
	CHANNEL_KINDS[1].0,
	CHANNEL_KINDS[2].0,
	CHANNEL_KINDS[3].0,
	CHANNEL_KINDS[4].0,
];
const SLOWMODE_LABELS: [&str; 14] = [
	SLOWMODES[0].0,
	SLOWMODES[1].0,
	SLOWMODES[2].0,
	SLOWMODES[3].0,
	SLOWMODES[4].0,
	SLOWMODES[5].0,
	SLOWMODES[6].0,
	SLOWMODES[7].0,
	SLOWMODES[8].0,
	SLOWMODES[9].0,
	SLOWMODES[10].0,
	SLOWMODES[11].0,
	SLOWMODES[12].0,
	SLOWMODES[13].0,
];

struct Act {
	layer: Rc<ModerationLayer>,
	open: Signal<bool>,
}

impl Act {
	fn new(layer: &Rc<ModerationLayer>, open: &Signal<bool>) -> Rc<Self> {
		Rc::new(Self {
			layer: layer.clone(),
			open: open.clone(),
		})
	}

	fn send(&self, command: Command) {
		self.layer.request(Request::Command(command));
		self.close();
	}

	fn close(&self) {
		self.open.set(false);
	}

	fn cancel(self: &Rc<Self>) -> View {
		let this = self.clone();
		button("Cancel", None, move || this.close())
	}

	fn audit_reason(self: &Rc<Self>, caption: &str, reason: &Signal<String>, placeholder: &str) -> View {
		field(
			caption,
			crate::kit::textarea("Reason", reason.clone(), placeholder),
		)
		.child(crate::kit::caption_note("This will be logged in the audit log."))
	}
}

pub fn view(layer: &Rc<ModerationLayer>, dialog: &Rc<Dialog>, open: Signal<bool>) -> View {
	let act = Act::new(layer, &open);
	let (title, body, width): (String, View, f32) = match dialog.as_ref() {
		Dialog::Nickname {
			guild_id,
			user_id,
			value,
		} => (
			"Change Nickname".to_owned(),
			nickname(&act, *guild_id, *user_id, value),
			WIDTH,
		),
		Dialog::Timeout {
			guild_id,
			user_id,
			name,
			choice,
			reason,
		} => (
			format!("Timeout {name}"),
			timeout(&act, (*guild_id, *user_id), choice, reason),
			WIDTH,
		),
		Dialog::Kick {
			guild_id,
			user_id,
			name,
			reason,
		} => (
			format!("Kick '{name}' from Server"),
			kick(&act, *guild_id, *user_id, name, reason),
			WIDTH,
		),
		Dialog::Ban {
			guild_id,
			user_id,
			name,
			history,
			reason,
		} => (
			format!("Ban '{name}'?"),
			ban(&act, (*guild_id, *user_id), name, history, reason, layer),
			WIDTH,
		),
		Dialog::Confirm(confirm) => {
			let action = confirm.action.clone();
			let this = act.clone();
			let body = column()
				.w_full()
				.gap(tok("space_16"))
				.p(tok("space_16"))
				.child(hint(confirm.body.clone()))
				.child(footer(
					act.cancel(),
					button(
						confirm.label,
						Some(if confirm.danger { "danger" } else { "brand" }),
						move || this.send(action.command()),
					),
				));
			(confirm.title.clone(), body, WIDTH)
		}
		Dialog::CreateChannel {
			guild_id,
			parent_id,
			kind,
			name,
		} => (
			"Create Channel".to_owned(),
			create_channel(&act, *guild_id, *parent_id, kind, name),
			WIDTH,
		),
		Dialog::EditSticker {
			guild_id,
			sticker_id,
			name,
			description,
			tags,
		} => (
			"Edit Sticker".to_owned(),
			edit_sticker(&act, (*guild_id, *sticker_id), [name, description, tags]),
			WIDTH,
		),
		Dialog::EditChannel {
			draft,
			name,
			topic,
			slowmode,
			nsfw,
			parent,
		} => (
			format!("Edit #{}", draft.name),
			edit_channel(&act, draft, [name, topic], slowmode, nsfw, parent, layer),
			WIDE,
		),
	};
	let close = act.clone();
	crate::kit::dialog(motion::modal(title.clone(), open))
		.w(width)
		.autofocus(!matches!(
			dialog.as_ref(),
			Dialog::Timeout { .. } | Dialog::Confirm(_)
		))
		.child(crate::kit::header(crate::kit::title(title), None, move || {
			close.close()
		}))
		.child(body)
}

fn nickname(act: &Rc<Act>, guild_id: Snowflake, user_id: Snowflake, value: &Signal<String>) -> View {
	let (save, enter, reset) = (act.clone(), act.clone(), act.clone());
	let (saved, entered) = (value.clone(), value.clone());
	let command = move |value: &Signal<String>| Command::SetNickname {
		guild_id,
		user_id,
		nick: value.with_untracked(|text| trimmed(text, NICKNAME_LIMIT)),
	};
	let on_enter = command;
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(field(
			"Nickname",
			input("Nickname", value.clone(), "No nickname").on_event(move |event| {
				if let zgx::input::InputEvent::KeyDown {
					key: zgx::input::Key::Enter,
					..
				} = event.event
				{
					enter.send(on_enter(&entered));
					event.prevent_default();
				}
			}),
		))
		.child(
			zgx::compose::button()
				.p(0.)
				.border(0.)
				.bg(crate::kit::clear())
				.cursor(Cursor::Pointer)
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.font_weight(500)
				.text_color(tok("accent_1"))
				.hover(|style| style.text_color(tok("accent_2")))
				.child(text("Reset Nickname"))
				.on_click(move || {
					reset.send(Command::SetNickname {
						guild_id,
						user_id,
						nick: None,
					})
				}),
		)
		.child(footer(
			act.cancel(),
			button("Save", Some("brand"), move || save.send(command(&saved))),
		))
}

fn timeout(
	act: &Rc<Act>,
	(guild_id, user_id): (Snowflake, Snowflake),
	choice: &Signal<usize>,
	reason: &Signal<String>,
) -> View {
	let (confirm, chosen, written) = (act.clone(), choice.clone(), reason.clone());
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(hint(
			"Members who are in timeout are temporarily not allowed to chat or react in text channels, or connect to voice and stage channels.",
		).text_color(tok("text_3")))
		.child(field("Duration", chips(&TIMEOUT_LABELS, choice)))
		.child(act.audit_reason("Reason", reason, "Optional"))
		.child(footer(
			act.cancel(),
			button("Timeout", Some("danger"), move || {
				let seconds = TIMEOUT_CHOICES
					.get(chosen.with_untracked(|index| *index))
					.map_or(0, |(_, seconds)| *seconds);
				confirm.send(Command::TimeoutMember {
					guild_id,
					user_id,
					until: Some(Utc::now() + TimeDelta::seconds(seconds)),
					reason: written.with_untracked(|text| trimmed(text, REASON_LIMIT)),
				});
			}),
		))
}

fn kick(act: &Rc<Act>, guild_id: Snowflake, user_id: Snowflake, name: &str, reason: &Signal<String>) -> View {
	let (confirm, written) = (act.clone(), reason.clone());
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(crate::kit::paragraph(format!(
			"Are you sure you want to kick @{name} from the server? They will be able to rejoin again with a new invite."
		)))
		.child(field(
			"Reason for kick",
			crate::kit::textarea("Reason", reason.clone(), "Optional"),
		))
		.child(footer(
			act.cancel(),
			button("Kick", Some("danger"), move || {
				confirm.send(Command::KickMember {
					guild_id,
					user_id,
					reason: written.with_untracked(|text| trimmed(text, REASON_LIMIT)),
				});
			}),
		))
}

fn ban(
	act: &Rc<Act>,
	(guild_id, user_id): (Snowflake, Snowflake),
	name: &str,
	history: &Signal<usize>,
	reason: &Signal<String>,
	layer: &Rc<ModerationLayer>,
) -> View {
	let (confirm, written, chosen) = (act.clone(), reason.clone(), history.clone());
	let menu_open = layer.runtime.signal(false);
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(crate::kit::paragraph(format!(
			"Are you sure you want to ban @{name}? They won't be able to rejoin with an invite."
		)))
		.child(field(
			"Delete message history",
			kit::select("Delete message history", &HISTORY_LABELS, history, menu_open),
		))
		.child(
			act.audit_reason("Reason for ban", reason, "Optional")
				.on_mount(|node| {
					let target = node.clone();
					node.after_layout(move || {
						target.focus();
					});
				}),
		)
		.child(footer(
			act.cancel(),
			button("Ban", Some("danger"), move || {
				let seconds = BAN_HISTORY_CHOICES
					.get(chosen.with_untracked(|index| *index))
					.map_or(0, |(_, seconds)| *seconds);
				confirm.send(Command::BanMember {
					guild_id,
					user_id,
					delete_message_seconds: seconds,
					reason: written.with_untracked(|text| trimmed(text, REASON_LIMIT)),
				});
			}),
		))
}

fn edit_sticker(
	act: &Rc<Act>,
	(guild_id, sticker_id): (Snowflake, Snowflake),
	[name, description, tags]: [&Signal<String>; 3],
) -> View {
	let (save, fields) = (act.clone(), (name.clone(), description.clone(), tags.clone()));
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(field("Name", input("Name", name.clone(), "")))
		.child(field(
			"Description",
			input("Description", description.clone(), "What does this sticker show?"),
		))
		.child(field(
			"Related emoji or tags",
			input("Tags", tags.clone(), "wave, hello"),
		))
		.child(footer(
			act.cancel(),
			button("Save", Some("brand"), move || {
				let read = |signal: &Signal<String>| signal.with_untracked(|text| trimmed(text, NAME_LIMIT));
				save.send(Command::EditSticker {
					guild_id,
					sticker_id,
					name: read(&fields.0),
					description: read(&fields.1),
					tags: read(&fields.2),
				});
			}),
		))
}

fn create_channel(
	act: &Rc<Act>,
	guild_id: Snowflake,
	parent_id: Option<Snowflake>,
	kind: &Signal<usize>,
	name: &Signal<String>,
) -> View {
	let (confirm, chosen, written) = (act.clone(), kind.clone(), name.clone());
	column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(field("Channel type", chips(&KIND_LABELS, kind)))
		.child(field(
			"Channel name",
			input("Channel name", name.clone(), "new-channel"),
		))
		.child(footer(
			act.cancel(),
			button("Create Channel", Some("brand"), move || {
				let Some(name) = written.with_untracked(|text| trimmed(text, NAME_LIMIT)) else {
					return;
				};
				let kind = CHANNEL_KINDS
					.get(chosen.with_untracked(|index| *index))
					.map_or(ChannelType::GuildText, |(_, kind)| *kind);
				confirm.send(Command::CreateChannel {
					guild_id,
					name,
					kind,
					parent_id,
				});
			}),
		))
}

fn edit_channel(
	act: &Rc<Act>,
	draft: &Rc<super::ChannelDraft>,
	[name, topic]: [&Signal<String>; 2],
	slowmode: &Signal<usize>,
	nsfw: &Signal<bool>,
	parent: &Signal<Option<Snowflake>>,
	layer: &Rc<ModerationLayer>,
) -> View {
	let texted = !matches!(draft.kind, ChannelType::GuildVoice | ChannelType::GuildStageVoice);
	let save = {
		let (act, draft) = (act.clone(), draft.clone());
		let (name, topic, slowmode, nsfw, parent) = (
			name.clone(),
			topic.clone(),
			slowmode.clone(),
			nsfw.clone(),
			parent.clone(),
		);
		move || {
			let new_name = name.with_untracked(|text| trimmed(text, NAME_LIMIT));
			let new_topic: String =
				topic.with_untracked(|text| text.trim().chars().take(TOPIC_LIMIT).collect());
			let seconds = SLOWMODES
				.get(slowmode.with_untracked(|index| *index))
				.map_or(0, |(_, seconds)| *seconds);
			let flagged = nsfw.with_untracked(|on| *on);
			let moved = parent.with_untracked(|parent| *parent);
			act.send(Command::EditChannel {
				channel_id: draft.channel_id,
				name: new_name.filter(|name| *name != draft.name),
				topic: (texted && new_topic != draft.topic).then_some(new_topic),
				nsfw: (flagged != draft.nsfw).then_some(flagged),
				slowmode: (texted && seconds != draft.slowmode).then_some(seconds),
				position: None,
				parent_id: moved.filter(|moved| Some(*moved) != draft.parent_id),
			});
		}
	};
	let categories: Vec<View> = draft
		.categories
		.iter()
		.map(|(id, category)| {
			let (pick, chosen, id) = (parent.clone(), parent.clone(), *id);
			kit::choice(category)
				.menu_checked_when(move || chosen.get() == Some(id))
				.on_click(move || {
					pick.set(Some(id));
				})
		})
		.collect();
	let shown_parent = parent.clone();
	let names: Vec<(Snowflake, String)> = draft.categories.clone();
	let category_open = layer.runtime.signal(false);
	let category_anchor = kit::select_anchor(
		text_signal(move || {
			let chosen = shown_parent.get();
			names
				.iter()
				.find(|(id, _)| Some(*id) == chosen)
				.map_or_else(|| "No category".to_owned(), |(_, name)| name.clone())
		}),
		category_open.clone(),
	);
	let mut form = column()
		.w_full()
		.gap(tok("space_16"))
		.p(tok("space_16"))
		.child(field("Channel name", input("Channel name", name.clone(), "")));
	if texted {
		form = form
			.child(field(
				"Channel topic",
				input(
					"Channel topic",
					topic.clone(),
					"Let everyone know how to use this channel",
				),
			))
			.child(field(
				"Slowmode",
				kit::select(
					"Slowmode",
					&SLOWMODE_LABELS,
					slowmode,
					layer.runtime.signal(false),
				),
			));
	}
	form.child(
		row()
			.w_full()
			.items_center()
			.justify_between()
			.gap(16.)
			.child(
				column()
					.grow()
					.min_w(0.)
					.child(
						label("Age-restricted channel", "font_md")
							.font_weight(600)
							.text_color(tok("text_2")),
					)
					.child(hint(
						"Users must confirm they are of legal age to view this channel.",
					)),
			)
			.child(kit::switch(nsfw.clone())),
	)
	.child(field(
		"Category",
		kit::menu_panel(menu("Category", category_open, category_anchor).match_anchor_width())
			.w_full()
			.children(categories),
	))
	.child(footer(act.cancel(), button("Save Changes", Some("brand"), save)))
}
