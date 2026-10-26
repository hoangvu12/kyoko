use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;

use protocol::PartialEmoji;
use session::{
	POLL_ANSWER_MAX, POLL_ANSWERS_MAX, POLL_DEFAULT_HOURS, POLL_DURATIONS, POLL_QUESTION_MAX, PollDraft,
	PollDraftAnswer, poll_length,
};
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Effect, Runtime, Signal};

use crate::kit::{self, Variant};
use crate::moderation::kit::select;
use crate::motion;
use crate::pickers::CELL_IMAGE;
use crate::timeline::Pictures;
use crate::timeline::{icon, tok};

pub const DIALOG_WIDTH: f32 = 570.;
const FIRST_ANSWERS: usize = 2;
const DURATION_WIDTH: f32 = 285.;
const ANSWERS_HEIGHT: f32 = 376.;
const ANSWER_GAP: f32 = 8.;
const ANSWER_PITCH: f32 = 48.;
const FIELD_ICON: f32 = 20.;
const ADD_ICON: f32 = 16.;
const DASH: f32 = 4.;

static DURATION_LABELS: LazyLock<Vec<&'static str>> =
	LazyLock::new(|| POLL_DURATIONS.iter().map(|(_, label)| *label).collect());

fn default_duration() -> usize {
	POLL_DURATIONS
		.iter()
		.position(|(hours, _)| *hours == POLL_DEFAULT_HOURS)
		.unwrap_or_default()
}

pub struct AnswerField {
	key: u64,
	text: Signal<String>,
	emoji: Signal<Option<PartialEmoji>>,
}

impl PartialEq for AnswerField {
	fn eq(&self, other: &Self) -> bool {
		self.key == other.key
	}
}

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

pub struct PollForm {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	open: Signal<bool>,
	question: Signal<String>,
	answers: Signal<Vec<Rc<AnswerField>>>,
	duration: Signal<usize>,
	duration_open: Signal<bool>,
	multiple: Signal<bool>,
	keys: Cell<u64>,
	picking: Cell<Option<u64>>,
	spot: Cell<(f32, f32)>,
	effects: RefCell<Vec<Effect>>,
	post: Hook<PollDraft>,
	pick: Hook<(f32, f32)>,
}

impl PollForm {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let form = Rc::new(Self {
			runtime: runtime.clone(),
			pictures,
			open: runtime.signal(false),
			question: runtime.signal(String::new()),
			answers: runtime.signal(Vec::new()),
			duration: runtime.signal(default_duration()),
			duration_open: runtime.signal(false),
			multiple: runtime.signal(false),
			keys: Cell::new(0),
			picking: Cell::new(None),
			spot: Cell::new((0., 0.)),
			effects: RefCell::default(),
			post: RefCell::new(None),
			pick: RefCell::new(None),
		});
		form.limit(&form.question, POLL_QUESTION_MAX);
		form
	}

	pub fn on_post(&self, hook: impl Fn(PollDraft) + 'static) {
		*self.post.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_pick_emoji(&self, hook: impl Fn((f32, f32)) + 'static) {
		*self.pick.borrow_mut() = Some(Box::new(hook));
	}

	fn limit(&self, value: &Signal<String>, most: usize) {
		let value = value.clone();
		let effect = self.runtime.effect(move || {
			let typed = value.get();
			if typed.chars().count() > most {
				value.set(typed.chars().take(most).collect());
			}
		});
		self.effects.borrow_mut().push(effect);
	}

	fn field(&self) -> Rc<AnswerField> {
		let key = self.keys.get() + 1;
		self.keys.set(key);
		let field = Rc::new(AnswerField {
			key,
			text: self.runtime.signal(String::new()),
			emoji: self.runtime.signal(None),
		});
		self.limit(&field.text, POLL_ANSWER_MAX);
		field
	}

	pub fn open(&self) {
		self.effects.borrow_mut().truncate(1);
		self.question.set(String::new());
		let fields = (0..FIRST_ANSWERS).map(|_| self.field()).collect();
		self.answers.set(fields);
		self.duration.set(default_duration());
		self.duration_open.set(false);
		self.multiple.set(false);
		self.picking.set(None);
		self.open.set(true);
	}

	pub fn close(&self) {
		self.duration_open.set(false);
		self.open.set(false);
	}

	pub fn spot(&self) -> (f32, f32) {
		self.spot.get()
	}

	pub fn custom_emoji(&self) -> Vec<(u64, bool)> {
		self.answers.with_untracked(|answers| {
			answers
				.iter()
				.filter_map(|answer| {
					answer.emoji.with_untracked(|emoji| {
						emoji
							.as_ref()
							.and_then(|emoji| Some((emoji.id?.get(), emoji.animated)))
					})
				})
				.collect()
		})
	}

	pub fn fill(&self, question: &str, answers: &[(&str, Option<PartialEmoji>)]) {
		self.question.set(question.to_owned());
		let fields: Vec<Rc<AnswerField>> = answers
			.iter()
			.map(|(text, emoji)| {
				let field = self.field();
				field.text.set((*text).to_owned());
				field.emoji.set(emoji.clone());
				field
			})
			.collect();
		self.answers.set(fields);
	}

	pub fn add_answer(&self) {
		if self.answers.with_untracked(Vec::len) >= POLL_ANSWERS_MAX {
			return;
		}
		let field = self.field();
		self.answers.update(|answers| answers.push(field));
	}

	fn remove(&self, key: u64) {
		self.answers.update(|answers| {
			if answers.len() > 1 {
				answers.retain(|answer| answer.key != key);
			} else if let Some(only) = answers.first() {
				only.text.set(String::new());
				only.emoji.set(None);
			}
		});
	}

	pub fn draft(&self) -> PollDraft {
		PollDraft {
			question: self.question.with_untracked(Clone::clone),
			answers: self.answers.with_untracked(|answers| {
				answers
					.iter()
					.map(|answer| PollDraftAnswer {
						text: answer.text.with_untracked(Clone::clone),
						emoji: answer.emoji.with_untracked(Clone::clone),
					})
					.collect()
			}),
			duration_hours: self.duration.with_untracked(|index| {
				POLL_DURATIONS
					.get(*index)
					.map_or(POLL_DEFAULT_HOURS, |(hours, _)| *hours)
			}),
			multiple: self.multiple.with_untracked(|on| *on),
		}
	}

	pub fn set_emoji(&self, emoji: PartialEmoji) {
		let Some(key) = self.picking.take() else {
			return;
		};
		self.answers.with_untracked(|answers| {
			if let Some(answer) = answers.iter().find(|answer| answer.key == key) {
				answer.emoji.set(Some(emoji));
			}
		});
	}

	fn submit(&self) {
		let draft = self.draft();
		if draft.problem().is_some() {
			return;
		}
		self.close();
		if let Some(post) = self.post.borrow().as_ref() {
			post(draft);
		}
	}

	fn pick_emoji(&self, key: u64) {
		self.picking.set(Some(key));
		if let Some(pick) = self.pick.borrow().as_ref() {
			pick(self.spot.get());
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (open, this) = (self.open.clone(), self.clone());
		kit::dialog(motion::modal("Create a poll", self.open.clone()))
			.id("poll-dialog")
			.w(tok("modal_width_wide"))
			.child_when(move || open.get().then_some(()), move |(), _| this.content())
	}

	fn content(self: &Rc<Self>) -> View {
		let close = self.clone();
		column()
			.w_full()
			.child(kit::header(kit::title("Create a poll"), None, move || {
				close.close()
			}))
			.child(
				kit::body()
					.child(label_field("Question", self.question_box()))
					.child(label_field("Answers", self.answers_view()))
					.child(label_field("Duration", self.duration_select()))
					.child(self.multiple_row())
					.child(kit::footer(vec![self.post_button()])),
			)
	}

	fn question_box(&self) -> View {
		let counted = self.question.clone();
		input_box(inline_input(
			text_input("Question", self.question.clone())
				.id("poll-question")
				.placeholder("What question do you want to ask?"),
		))
		.child(
			text_signal(move || format!("{} / {POLL_QUESTION_MAX}", poll_length(&counted.get())))
				.shrink_0()
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.text_color(tok("text_5")),
		)
	}

	fn answers_view(self: &Rc<Self>) -> View {
		let (answers, this) = (self.answers.clone(), self.clone());
		let (count, adder) = (self.answers.clone(), self.clone());
		let rows = self.answers.clone();
		column()
			.w_full()
			.gap(tok("space_8"))
			.child(
				scroll(self.runtime.signal(0.))
					.w_full()
					.reactive_style(move || {
						let shown = rows.with(Vec::len).max(1) as f32;
						Styles::new().h((shown * ANSWER_PITCH - ANSWER_GAP).min(ANSWERS_HEIGHT))
					})
					.child(column().w_full().gap(tok("space_8")).child_when(
						move || Some(answers.get()),
						move |fields: Vec<Rc<AnswerField>>, _| {
							column().w_full().gap(tok("space_8")).children(
								fields
									.iter()
									.enumerate()
									.map(|(index, field)| this.answer_row(index, field)),
							)
						},
					)),
			)
			.child_when(
				move || (count.with(Vec::len) < POLL_ANSWERS_MAX).then_some(()),
				move |(), _| {
					let adder = adder.clone();
					row()
						.w_full()
						.gap(tok("space_8"))
						.child(add_answer_button(move || adder.add_answer()).w(0.).grow())
						.child(div().w(tok("button_size")).shrink_0())
				},
			)
	}

	fn answer_row(self: &Rc<Self>, index: usize, field: &Rc<AnswerField>) -> View {
		let (remover, key) = (self.clone(), field.key);
		row()
			.w_full()
			.items_center()
			.gap(tok("space_8"))
			.child(
				input_box(self.emoji_button(index, field))
					.grow()
					.min_w(0.)
					.child(inline_input(
						text_input(format!("Answer {}", index + 1), field.text.clone())
							.id(format!("poll-answer-{index}"))
							.placeholder("Type your answer"),
					)),
			)
			.child(
				button()
					.id(format!("poll-remove-{index}"))
					.focusable(false)
					.w(tok("button_size"))
					.h(tok("button_size"))
					.shrink_0()
					.p(0.)
					.border(0.)
					.items_center()
					.justify_center()
					.bg(kit::clear())
					.cursor(Cursor::Pointer)
					.text_color(tok("text_4"))
					.hover(|style| style.text_color(tok("text_3")))
					.child(crate::timeline::icons::glyph("close", FIELD_ICON))
					.on_click(move || remover.remove(key)),
			)
	}

	fn emoji_button(self: &Rc<Self>, index: usize, field: &Rc<AnswerField>) -> View {
		let (shown, picker, spot, key) = (field.emoji.clone(), self.clone(), self.clone(), field.key);
		let pictures = self.pictures.clone();
		button()
			.id(format!("poll-emoji-{index}"))
			.focusable(false)
			.w(FIELD_ICON)
			.h(FIELD_ICON)
			.shrink_0()
			.p(0.)
			.border(0.)
			.items_center()
			.justify_center()
			.bg(kit::clear())
			.cursor(Cursor::Pointer)
			.on_event(move |event| {
				if let InputEvent::PointerDown { x, y, .. } = event.event {
					spot.spot.set((x, y));
				}
			})
			.on_click(move || picker.pick_emoji(key))
			.child(switch(
				move || shown.get(),
				move |emoji: Option<PartialEmoji>, _| match emoji {
					Some(PartialEmoji {
						id: None,
						name: Some(name),
						..
					}) => crate::emoji::glyph(name, FIELD_ICON),
					Some(PartialEmoji {
						id: Some(id),
						animated,
						..
					}) => custom_emoji(&pictures, id.get(), animated),
					_ => icon("smile", FIELD_ICON, "text_4"),
				},
			))
	}

	fn duration_select(&self) -> View {
		div().w(DURATION_WIDTH).child(
			select(
				"Duration",
				DURATION_LABELS.as_slice(),
				&self.duration,
				self.duration_open.clone(),
			)
			.id("poll-duration"),
		)
	}

	fn multiple_row(&self) -> View {
		let (checked, flip) = (self.multiple.clone(), self.multiple.clone());
		row()
			.id("poll-multiple")
			.items_center()
			.gap(tok("space_10"))
			.cursor(Cursor::Pointer)
			.on_click(move || {
				flip.set(!flip.with_untracked(|on| *on));
			})
			.child(kit::checkbox(move || checked.get()))
			.child(
				text("Allow Multiple Answers")
					.text_size(tok("font_size_md"))
					.line_height(tok("line_height_label"))
					.font_weight(500)
					.text_color(tok("text_3")),
			)
	}

	fn post_button(self: &Rc<Self>) -> View {
		let (submit, ready) = (self.clone(), self.clone());
		let (question, answers) = (self.question.clone(), self.answers.clone());
		kit::action("Post", Variant::Primary, move || submit.submit())
			.id("poll-post")
			.reactive_style(move || {
				question.with(|_| ());
				answers.with(|answers| {
					for answer in answers {
						answer.text.with(|_| ());
					}
				});
				let blocked = ready.draft().problem().is_some();
				Styles::new().opacity(if blocked { DISABLED } else { 1. })
			})
	}
}

const DISABLED: f32 = 0.5;

fn label_field(label: &str, content: View) -> View {
	column()
		.w_full()
		.gap(tok("space_8"))
		.child(
			text(label.to_owned())
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.font_weight(600)
				.text_color(tok("text_2")),
		)
		.child(content)
}

fn input_box(first: View) -> View {
	row()
		.w_full()
		.h(tok("input_height"))
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_12"))
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border_light"))
		.hover(|style| style.border_color(tok("border")))
		.child(first)
}

fn custom_emoji(pictures: &Pictures, id: u64, animated: bool) -> View {
	let picture = pictures.picture(&pictures.emoji(id, animated, f64::from(CELL_IMAGE)));
	div()
		.w(FIELD_ICON)
		.h(FIELD_ICON)
		.shrink_0()
		.reactive_style(move || match picture.get() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
			None => Styles::new(),
		})
}

fn inline_input(input: View) -> View {
	input
		.w(0.)
		.grow()
		.min_w(0.)
		.h_full()
		.p(0.)
		.border(0.)
		.bg(kit::clear())
		.text_color(tok("text_3"))
		.text_size(tok("font_size_md"))
		.focus(|style| style.outline_width(0.))
}

fn add_answer_button(action: impl FnMut() + 'static) -> View {
	button()
		.id("poll-add-answer")
		.focusable(false)
		.w_full()
		.h(tok("input_height"))
		.items_center()
		.justify_start()
		.gap(tok("space_8"))
		.px(tok("space_12"))
		.rounded(tok("radius_sm"))
		.bg(kit::clear())
		.border(tok("border_thickness"))
		.border_style(BorderStyle::Dashed {
			length: DASH,
			gap: DASH,
		})
		.border_color(tok("border"))
		.cursor(Cursor::Pointer)
		.text_color(tok("text_4"))
		.hover(|style| style.text_color(tok("text_3")))
		.child(crate::timeline::icons::glyph("plus", ADD_ICON))
		.child(
			text("Add another answer")
				.text_size(tok("font_size_md"))
				.line_height(tok("line_height_label"))
				.font_weight(500),
		)
		.on_click(action)
}
