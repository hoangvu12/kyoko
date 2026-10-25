use std::rc::Rc;

use bridge::{MessageEntry, PollSummary};
use chrono::Utc;
use protocol::{Reaction, Snowflake};
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::attachments::picture;
use super::icons::icon;
use super::pictures::REACTION_EMOJI;
use super::rows::{RowEnv, tok};

const POLL_WIDTH: f32 = 432.;
const POLL_ROW: f32 = 40.;
const POLL_DOT: f32 = 16.;
const CHIP_HEIGHT: f32 = 24.;

pub fn reactions(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	let reactions = &entry.message.reactions;
	if reactions.is_empty() || !env.prefs.show_reactions {
		return None;
	}
	Some(
		row()
			.flex_wrap()
			.gap(4.)
			.mt(4.)
			.children(reactions.iter().map(|reaction| chip(reaction, env))),
	)
}

fn chip(reaction: &Reaction, env: &Rc<RowEnv>) -> View {
	let size = REACTION_EMOJI as f32;
	let glyph = match env.pictures.reaction(&reaction.emoji) {
		Some(name) => div().size(size, size).shrink_0().child(picture(
			&env.pictures,
			Some(name),
			None,
			ObjectFit::Contain,
		)),
		None => text(reaction.emoji.name.clone().unwrap_or_default())
			.text_size(size)
			.line_height(size + 2.),
	};
	let (border, background, color) = if reaction.me {
		("brand", "reaction_me", "mention_text")
	} else {
		(
			"background_secondary",
			"background_secondary",
			"interactive_normal",
		)
	};
	row()
		.items_center()
		.gap(6.)
		.h(CHIP_HEIGHT)
		.px(6.)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok(border))
		.bg(tok(background))
		.text_size(tok("font_sm"))
		.font_weight(600)
		.text_color(tok(color))
		.cursor(Cursor::Pointer)
		.when(!reaction.me, |chip| {
			chip.hover(|style| style.border_color(tok("interactive_muted")))
		})
		.child(glyph)
		.child(text(reaction.count.to_string()))
}

pub fn thread_chip(entry: &MessageEntry) -> Option<View> {
	let thread = entry.message.thread.as_ref()?;
	let count = thread.message_count.unwrap_or(0);
	let activity = thread
		.last_message_id
		.map_or_else(|| thread.id.timestamp(), Snowflake::timestamp);
	let label = if count == 1 {
		"1 message".to_owned()
	} else {
		format!("{count} messages")
	};
	Some(
		row()
			.items_center()
			.gap(8.)
			.mt(4.)
			.px(12.)
			.py(8.)
			.max_w(432.)
			.rounded(tok("radius_md"))
			.border(1.)
			.border_color(tok("divider"))
			.bg(tok("background_secondary"))
			.hover(|style| style.bg(tok("modifier_selected")))
			.child(icon("thread", 20., "interactive_normal"))
			.child(
				text(thread.name.clone().unwrap_or_default())
					.truncate()
					.min_w(0.)
					.text_size(tok("font_sm"))
					.font_weight(600)
					.text_color(tok("header_primary")),
			)
			.child(
				text(label)
					.shrink_0()
					.text_size(tok("font_tiny"))
					.text_color(tok("text_muted")),
			)
			.child(
				text(bridge::message_time(activity, bridge::today()))
					.shrink_0()
					.text_size(tok("font_tiny"))
					.text_color(tok("text_muted")),
			),
	)
}

pub fn poll(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>) -> Option<View> {
	let poll = entry.message.poll.as_ref()?;
	let summary = PollSummary::of(poll, Utc::now());
	let (channel_id, message_id) = (entry.message.channel_id, entry.id);
	let voted = summary.voted();
	let picks = env.picks.borrow().get(&message_id).cloned();
	let current = picks.clone().unwrap_or_else(|| voted.clone());
	let dirty = picks.is_some_and(|picks| picks != voted);
	let rows = summary.choices.iter().map(|choice| {
		let chosen = current.contains(&choice.id);
		let fill = if chosen { "poll_fill_chosen" } else { "poll_fill" };
		let answer = choice.id;
		let (env, bump, current) = (env.clone(), bump.clone(), current.clone());
		let multi = summary.multi;
		row()
			.relative()
			.items_center()
			.justify_between()
			.gap(8.)
			.h(POLL_ROW)
			.px(12.)
			.rounded(tok("radius_md"))
			.overflow_hidden()
			.bg(tok("background_tertiary"))
			.when(!summary.finished, move |row| {
				row.cursor(Cursor::Pointer)
					.hover(|style| style.bg(tok("modifier_hover")))
					.on_click(move || {
						if multi {
							let mut next = current.clone();
							match next.iter().position(|id| *id == answer) {
								Some(at) => {
									next.remove(at);
								}
								None => next.push(answer),
							}
							env.picks.borrow_mut().insert(message_id, next);
							bump.update(|count| *count += 1);
						} else {
							env.send(Command::VotePoll {
								channel_id,
								message_id,
								answer_ids: if chosen { Vec::new() } else { vec![answer] },
							});
						}
					})
			})
			.child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.h_full()
					.w_percent(choice.share * 100.)
					.bg(tok(fill)),
			)
			.child(
				div()
					.size(POLL_DOT, POLL_DOT)
					.shrink_0()
					.rounded(POLL_DOT / 2.)
					.border(1.)
					.border_color(tok(if chosen { "brand" } else { "interactive_muted" }))
					.items_center()
					.justify_center()
					.when(chosen, |dot| {
						dot.bg(tok("brand")).child(icon("check", 12., "text_on_brand"))
					}),
			)
			.child(
				text(choice.text.as_str())
					.grow()
					.min_w(0.)
					.truncate()
					.text_size(tok("font_sm"))
					.font_weight(500)
					.text_color(tok("header_primary")),
			)
			.child(
				text(format!("{}%", bridge::percent(choice.share)))
					.shrink_0()
					.text_size(tok("font_sm"))
					.text_color(tok("header_secondary")),
			)
	});
	let mut footer = row().items_center().gap(8.).child(
		text(summary.votes_label())
			.grow()
			.text_size(tok("font_tiny"))
			.text_color(tok("text_muted")),
	);
	if summary.multi && !summary.finished && dirty {
		let (env, picks) = (env.clone(), current.clone());
		footer = footer.child(poll_button("Vote", "brand", "text_on_brand", move || {
			env.picks.borrow_mut().remove(&message_id);
			env.send(Command::VotePoll {
				channel_id,
				message_id,
				answer_ids: picks.clone(),
			});
		}));
	}
	if entry.mine && !summary.finished {
		let env = env.clone();
		footer = footer.child(poll_button(
			"End Poll",
			"background_secondary",
			"text_normal",
			move || {
				env.send(Command::EndPoll {
					channel_id,
					message_id,
				});
			},
		));
	}
	Some(
		column()
			.mt(4.)
			.gap(8.)
			.w(POLL_WIDTH)
			.max_w_percent(100.)
			.p(12.)
			.rounded(tok("radius_md"))
			.border(1.)
			.border_color(tok("code_border"))
			.bg(tok("background_secondary"))
			.child(
				text(summary.question.as_str())
					.text_size(tok("font_md"))
					.font_weight(700)
					.text_color(tok("header_primary")),
			)
			.child(
				text(summary.hint())
					.text_size(tok("font_tiny"))
					.text_color(tok("text_muted")),
			)
			.children(rows)
			.child(footer),
	)
}

fn poll_button(label: &str, background: &str, color: &str, action: impl FnMut() + 'static) -> View {
	button()
		.h(24.)
		.px(12.)
		.rounded(tok("radius_sm"))
		.border(1.)
		.border_color(tok("interactive_muted"))
		.bg(tok(background))
		.text_size(tok("font_sm"))
		.text_color(tok(color))
		.cursor(Cursor::Pointer)
		.child(text(label.to_owned()))
		.on_click(action)
}
