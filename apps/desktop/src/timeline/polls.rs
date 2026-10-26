use std::rc::Rc;

use bridge::{MessageEntry, PollChoice, PollOutcome, PollSummary};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::theme::Themed;

use super::icons::icon;
use super::inline::{self, Art, Paragraph, Placed};
use super::reactions::emoji_view;
use super::rows::{RowEnv, tok};
use crate::emoji;

const TROPHY: &str = "\u{1f3c6}";
const ANSWER_EMOJI: f32 = 20.;
const CHECK: f32 = 12.;
const RESULT_EMOJI: f32 = 22.;
const RESULT_EMOJI_MARGIN: f32 = 1.;
const RESULT_ICON: f32 = 20.;
const RESULT_ICON_LEFT: f32 = 26.;
const RESULT_ICON_TOP: f32 = 2.;
const TIME_SIZE: f32 = 12.;

#[derive(Clone)]
struct Target {
	env: Rc<RowEnv>,
	bump: Signal<u32>,
	channel_id: Snowflake,
	message_id: Snowflake,
}

impl Target {
	fn send_vote(&self, answer_ids: Vec<i64>) {
		self.env.picks.borrow_mut().remove(&self.message_id);
		self.env.peeks.borrow_mut().remove(&self.message_id);
		self.env.send(Command::VotePoll {
			channel_id: self.channel_id,
			message_id: self.message_id,
			answer_ids,
		});
		self.redraw();
	}

	fn pick(&self, answer: i64, multi: bool) {
		{
			let mut picks = self.env.picks.borrow_mut();
			let chosen = picks.entry(self.message_id).or_default();
			match chosen.iter().position(|id| *id == answer) {
				Some(at) => {
					chosen.remove(at);
				}
				None if multi => chosen.push(answer),
				None => *chosen = vec![answer],
			}
		}
		self.redraw();
	}

	fn peek(&self, shown: bool) {
		if shown {
			self.env.peeks.borrow_mut().insert(self.message_id);
		} else {
			self.env.peeks.borrow_mut().remove(&self.message_id);
		}
		self.redraw();
	}

	fn redraw(&self) {
		self.bump.update(|count| *count += 1);
	}
}

pub fn poll(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>) -> Option<View> {
	let poll = entry.message.poll.as_ref()?;
	let summary = PollSummary::of(poll, Utc::now());
	let target = Target {
		env: env.clone(),
		bump: bump.clone(),
		channel_id: entry.message.channel_id,
		message_id: entry.id,
	};
	let live = !entry.pending && !summary.finished;
	let voted = summary.has_voted();
	let peeked = env.peeks.borrow().contains(&entry.id);
	let results = summary.finished || voted || peeked;
	let picks = env.picks.borrow().get(&entry.id).cloned().unwrap_or_default();
	let rows: Vec<View> = summary
		.choices
		.iter()
		.map(|choice| {
			let picked = picks.contains(&choice.id);
			if results {
				result_row(choice, env)
			} else {
				voting_row(choice, picked, summary.multi, live, env, &target)
			}
		})
		.collect();
	let mut footer = row()
		.mt(tok("space_12"))
		.items_center()
		.gap(tok("space_8"))
		.child(
			text(summary.footer())
				.grow()
				.min_w(0.)
				.truncate()
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.text_color(tok("text_4")),
		);
	if live && entry.mine {
		let end = target.clone();
		footer = footer.child(text_button("End Poll", move || {
			end.env.send(Command::EndPoll {
				channel_id: end.channel_id,
				message_id: end.message_id,
			});
		}));
	}
	if live && !results {
		let (show, vote) = (target.clone(), target.clone());
		let chosen = picks.clone();
		footer = footer
			.child(text_button("Show results", move || show.peek(true)).id(format!("poll-show-{}", entry.id)))
			.child(
				vote_button(!picks.is_empty(), move || {
					if !chosen.is_empty() {
						vote.send_vote(chosen.clone());
					}
				})
				.id(format!("poll-vote-{}", entry.id)),
			);
	} else if live && voted {
		let remove = target.clone();
		footer = footer.child(
			text_button("Remove Vote", move || remove.send_vote(Vec::new()))
				.id(format!("poll-remove-{}", entry.id)),
		);
	} else if live && peeked {
		let back = target.clone();
		footer = footer.child(text_button("Go back to vote", move || back.peek(false)));
	}
	Some(
		column()
			.id(format!("poll-{}", entry.id))
			.mt(tok("space_4"))
			.w(tok("poll_max_width"))
			.max_w_percent(100.)
			.p(tok("space_12"))
			.rounded(tok("radius_md"))
			.bg(tok("bg_4"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.child(
				text(summary.question.clone())
					.w_full()
					.text_wrap(true)
					.text_size(tok("font_size_md"))
					.line_height(tok("line_height_label"))
					.font_weight(600)
					.text_color(tok("text_2")),
			)
			.child(
				text(summary.hint())
					.text_size(tok("font_size_sm"))
					.line_height(tok("line_height_sm"))
					.text_color(tok("text_4")),
			)
			.child(column().mt(tok("space_12")).gap(tok("space_4")).children(rows))
			.child(footer),
	)
}

fn answer_frame(choice: &PollChoice, outlined: bool) -> View {
	row()
		.relative()
		.w_full()
		.h(tok("poll_row_height"))
		.items_center()
		.gap(tok("space_12"))
		.px(tok("space_12"))
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok(if outlined { "accent_2" } else { "bg_3" }))
		.id(format!("poll-answer-{}", choice.id))
}

fn answer_label(choice: &PollChoice, env: &Rc<RowEnv>) -> View {
	row()
		.grow()
		.min_w(0.)
		.items_center()
		.gap(tok("space_12"))
		.when_some(choice.emoji.as_ref(), |label, emoji| {
			label.child(emoji_view(&env.pictures, emoji, ANSWER_EMOJI))
		})
		.child(
			text(choice.text.clone())
				.grow()
				.min_w(0.)
				.truncate()
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.font_weight(500)
				.text_color(tok("text_2")),
		)
}

fn voting_row(
	choice: &PollChoice,
	picked: bool,
	multi: bool,
	live: bool,
	env: &Rc<RowEnv>,
	target: &Target,
) -> View {
	let mark = if multi {
		crate::kit::checkbox(move || picked)
	} else {
		radio(picked)
	};
	let (answer, target) = (choice.id, target.clone());
	answer_frame(choice, picked)
		.when(live, move |frame| {
			frame
				.cursor(Cursor::Pointer)
				.on_click(move || target.pick(answer, multi))
		})
		.child(answer_label(choice, env))
		.child(mark)
}

fn radio(picked: bool) -> View {
	let dot = div()
		.w(tok("radio_size"))
		.h(tok("radio_size"))
		.shrink_0()
		.rounded(tok("radio_size"));
	if picked {
		dot.bg(tok("bg_4"))
			.border(tok("radio_border_selected"))
			.border_color(tok("accent_2"))
	} else {
		dot.bg(tok("bg_2"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
	}
}

fn result_row(choice: &PollChoice, env: &Rc<RowEnv>) -> View {
	let fill = if choice.winner {
		"poll_fill_winner"
	} else {
		"poll_fill"
	};
	answer_frame(choice, choice.voted)
		.child(
			div()
				.absolute()
				.left(0.)
				.top(0.)
				.h_full()
				.w_percent(choice.share * 100.)
				.bg(tok(fill)),
		)
		.child(answer_label(choice, env))
		.child(
			row()
				.shrink_0()
				.items_center()
				.gap(tok("space_8"))
				.when(choice.voted, |side| side.child(check()))
				.when(choice.winner, |side| {
					side.child(emoji::glyph(TROPHY, ANSWER_EMOJI))
				})
				.child(
					text(format!("{}%", bridge::percent(choice.share)))
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.font_weight(600)
						.text_color(tok("text_2")),
				),
		)
}

fn check() -> View {
	div()
		.w(tok("icon_size_md"))
		.h(tok("icon_size_md"))
		.shrink_0()
		.rounded(tok("icon_size_md"))
		.bg(tok("accent_2"))
		.items_center()
		.justify_center()
		.child(icon("check", CHECK, "text_0"))
}

fn text_button(label: &str, action: impl FnMut() + 'static) -> View {
	button()
		.focusable(false)
		.h(tok("button_height_sm"))
		.px(tok("space_12"))
		.shrink_0()
		.border(0.)
		.bg(crate::kit::clear())
		.items_center()
		.cursor(Cursor::Pointer)
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.font_weight(500)
		.text_color(tok("text_3"))
		.hover(|style| style.text_color(tok("text_2")))
		.child(text(label.to_owned()))
		.on_click(action)
}

fn vote_button(enabled: bool, action: impl FnMut() + 'static) -> View {
	button()
		.h(tok("button_height_sm"))
		.px(tok("space_16"))
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(tok("accent_3"))
		.border(tok("border_thickness"))
		.border_color(tok("button_border"))
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_label"))
		.font_weight(500)
		.text_color(tok("text_0"))
		.when(enabled, |vote| {
			vote.cursor(Cursor::Pointer)
				.hover(|style| style.bg(tok("accent_4")))
		})
		.when(!enabled, |vote| vote.opacity(tok("disabled_opacity_control")))
		.child(text("Vote"))
		.on_click(action)
}

struct Line {
	spans: Vec<TextSpan>,
	placed: Vec<Placed>,
	length: usize,
}

impl Line {
	fn push(&mut self, piece: &str, span: impl FnOnce(TextSpan) -> TextSpan) {
		self.length += piece.len();
		self.spans.push(span(text_span(piece.to_owned())));
	}

	fn emoji(&mut self, glyph: &str) {
		let Some(art) = emoji::svg_of(glyph) else {
			self.push(glyph, |span| span);
			return;
		};
		let start = self.length;
		self.length += glyph.len();
		self.placed.push(Placed {
			range: start..self.length,
			art: Art::Svg(art),
			target: None,
			subject: None,
		});
		self.spans.push(inline::placeholder(
			text_span(glyph.to_owned()),
			RESULT_EMOJI / 2. + RESULT_EMOJI_MARGIN,
		));
	}
}

fn strong(span: TextSpan) -> TextSpan {
	span.font_weight(600).text_color(tok("text_2"))
}

pub fn result_row_view(entry: &MessageEntry, env: &Rc<RowEnv>, gutter: f32, right: f32, gap: f32) -> View {
	let Some(outcome) = PollOutcome::of(&entry.message) else {
		return div();
	};
	let author_color = match entry.author_color {
		Some(color) => Themed::Value(Color((color >> 16) as u8, (color >> 8) as u8, color as u8, 255)),
		None => Themed::Token(tok("text_2")),
	};
	let mut line = Line {
		spans: Vec::new(),
		placed: Vec::new(),
		length: 0,
	};
	line.push(&entry.author, |span| {
		span.font_weight(500).text_color(author_color)
	});
	line.push("'s poll ", |span| span);
	line.push(&outcome.question, strong);
	line.push(" has closed. ", |span| span);
	if let Some(winner) = &outcome.winner {
		line.emoji(TROPHY);
		line.push(" ", |span| span);
		line.push(winner, strong);
		line.push(&format!(" \u{b7} {}% ", bridge::percent(outcome.share)), |span| {
			span
		});
	}
	line.push(&bridge::message_time(entry.sent_at, bridge::today()), |span| {
		span.text_size(TIME_SIZE).text_color(tok("text_5"))
	});
	let paragraph = inline::paragraph(
		Paragraph {
			spans: line.spans,
			placed: line.placed,
			side: RESULT_EMOJI,
		},
		&env.pictures,
		&env.actions,
	)
	.text_size(tok("font_size_md"))
	.line_height(tok("line_height_md"))
	.text_color(tok("text_4"));
	let actions = env.actions.clone();
	column()
		.id(format!("poll-result-{}", entry.id))
		.relative()
		.w_full()
		.pl(gutter)
		.pr(right)
		.mt(gap)
		.py(tok("space_2"))
		.rounded(tok("radius_md"))
		.hover(|style| style.bg(tok("message_hover")))
		.child(
			div()
				.absolute()
				.left(RESULT_ICON_LEFT)
				.top(RESULT_ICON_TOP)
				.child(icon("poll", RESULT_ICON, "text_4")),
		)
		.child(paragraph)
		.when_some(outcome.poll_id, move |column, poll_id| {
			column.child(
				div().mt(tok("space_4")).child(
					crate::kit::button_view(text("View Poll"), crate::kit::Variant::Secondary, move || {
						actions.jump(poll_id)
					})
					.h(tok("button_height_sm"))
					.id(format!("poll-view-{poll_id}")),
				),
			)
		})
}
