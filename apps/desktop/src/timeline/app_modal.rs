use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use bridge::components::{ChoiceOption, FieldKind, ModalField, ModalForm};
use session::{Command, ModalAnswer, ModalValue};
use state::ModalStatus;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::{Effect, Runtime, Signal};

use super::icons::icon;
use super::rows::{Sender, tok};
use crate::kit::{self, Variant};
use crate::motion;

const APP_ICON: f32 = 24.;
const REQUIRED: &str = "This field is required";

enum Answer {
	Text(Signal<String>),
	Values(Signal<Vec<String>>),
}

pub struct AppModal {
	runtime: Runtime,
	form: Signal<Option<Rc<ModalForm>>>,
	open: Signal<bool>,
	answers: RefCell<HashMap<String, Answer>>,
	serial: Cell<u64>,
	commands: Rc<Sender>,
	watcher: RefCell<Option<Effect>>,
}

impl AppModal {
	pub fn new(runtime: &Runtime, commands: Rc<Sender>) -> Rc<Self> {
		let modal = Rc::new(Self {
			runtime: runtime.clone(),
			form: runtime.signal(None),
			open: runtime.signal(false),
			answers: RefCell::default(),
			serial: Cell::new(0),
			commands,
			watcher: RefCell::new(None),
		});
		let (open, weak) = (modal.open.clone(), Rc::downgrade(&modal));
		let mut was = false;
		let effect = runtime.effect(move || {
			let now = open.get();
			if was
				&& !now && let Some(modal) = weak.upgrade()
				&& modal.form.with_untracked(Option::is_some)
			{
				modal.form.set(None);
				modal.send(Command::CloseModal);
			}
			was = now;
		});
		*modal.watcher.borrow_mut() = Some(effect);
		modal
	}

	fn send(&self, command: Command) {
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn set(&self, form: Option<ModalForm>) {
		let Some(form) = form else {
			if self.form.with_untracked(Option::is_some) {
				self.form.set(None);
				self.open.set(false);
			}
			return;
		};
		if self.form.with_untracked(|shown| shown.as_deref() == Some(&form)) {
			return;
		}
		if self.serial.replace(form.serial) != form.serial {
			let answers = form
				.fields
				.iter()
				.filter_map(|field| {
					let answer = match &field.kind {
						FieldKind::Text(_) => return None,
						FieldKind::Select { options, .. } => Answer::Values(
							self.runtime.signal(
								options
									.iter()
									.filter(|option| option.default)
									.map(|option| option.value.clone())
									.collect(),
							),
						),
						_ => Answer::Text(self.runtime.signal(field.value.clone().unwrap_or_default())),
					};
					Some((field.custom_id.clone(), answer))
				})
				.collect();
			*self.answers.borrow_mut() = answers;
		}
		self.form.set(Some(Rc::new(form)));
		self.open.set(true);
	}

	pub fn type_into(&self, custom_id: &str, value: &str) {
		if let Some(Answer::Text(signal)) = self.answers.borrow().get(custom_id) {
			signal.set(value.to_owned());
		}
	}

	pub fn submit(&self) {
		let answers = self
			.answers
			.borrow()
			.iter()
			.map(|(custom_id, answer)| ModalAnswer {
				custom_id: custom_id.clone(),
				value: match answer {
					Answer::Text(signal) => ModalValue::Text(signal.with_untracked(Clone::clone)),
					Answer::Values(signal) => ModalValue::Values(signal.with_untracked(Clone::clone)),
				},
			})
			.collect();
		self.send(Command::SubmitModal { answers });
	}

	fn close(&self) {
		self.open.set(false);
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (shown, open) = (self.form.clone(), self.open.clone());
		let this = self.clone();
		kit::dialog(motion::modal("App modal", self.open.clone()))
			.w(tok("modal_width_sm"))
			.child_when(
				move || if open.get() { shown.get() } else { None },
				move |form: Rc<ModalForm>, _| this.content(&form),
			)
	}

	fn content(self: &Rc<Self>, form: &ModalForm) -> View {
		let (close, cancel, accept) = (self.clone(), self.clone(), self.clone());
		let fields: Vec<View> = form
			.fields
			.iter()
			.map(|field| self.field(field, form.missing.contains(&field.custom_id)))
			.collect();
		let submitting = form.status == ModalStatus::Submitting;
		column()
			.id("app-modal")
			.w_full()
			.relative()
			.child(
				column()
					.w_full()
					.pt(tok("space_16"))
					.px(tok("space_16"))
					.pb(tok("space_8"))
					.child(
						row()
							.items_center()
							.gap(tok("space_8"))
							.child(app_icon(&form.application.name))
							.child(
								text(form.application.name.clone())
									.text_size(tok("font_size_xs"))
									.line_height(tok("line_height_xs"))
									.font_weight(500)
									.text_color(tok("text_4")),
							),
					)
					.child(
						kit::title(form.title.clone())
							.id("app-modal-title")
							.mt(tok("space_8")),
					),
			)
			.child(
				div()
					.absolute()
					.top(12.)
					.right(12.)
					.child(kit::close_button(move || close.close())),
			)
			.child(
				column()
					.w_full()
					.pt(tok("space_8"))
					.px(tok("space_16"))
					.pb(tok("space_16"))
					.gap(tok("space_16"))
					.children(fields),
			)
			.when_some(form.failure(), |column, failure| {
				column.child(
					row()
						.id("app-modal-error")
						.items_center()
						.gap(tok("space_6"))
						.px(tok("space_16"))
						.pb(tok("space_12"))
						.child(icon("warning_fill", 16., "red_2"))
						.child(
							text(failure)
								.text_size(tok("font_size_sm"))
								.line_height(tok("line_height_sm"))
								.font_weight(500)
								.text_color(tok("red_2")),
						),
				)
			})
			.child(
				row()
					.w_full()
					.justify_end()
					.gap(tok("space_8"))
					.p(tok("space_16"))
					.bg(tok("bg_4"))
					.border_edges(zgx::scene::Insets {
						top: 1.,
						..Default::default()
					})
					.border_color(tok("border"))
					.child(kit::action("Cancel", Variant::Link, move || cancel.close()))
					.child(
						kit::action("Submit", Variant::Primary, move || accept.submit())
							.id("app-modal-submit")
							.when(submitting, |button| button.opacity(0.5)),
					),
			)
	}

	fn field(self: &Rc<Self>, field: &ModalField, missing: bool) -> View {
		let answers = self.answers.borrow();
		let input = match (&field.kind, answers.get(&field.custom_id)) {
			(FieldKind::Text(content), _) => {
				return text(content.clone())
					.w_full()
					.text_wrap(true)
					.text_size(tok("font_size_sm"))
					.line_height(tok("line_height_sm"))
					.text_color(tok("text_3"));
			}
			(FieldKind::Short, Some(Answer::Text(value))) => short(field, value.clone(), missing),
			(FieldKind::Paragraph, Some(Answer::Text(value))) => paragraph(field, value.clone(), missing),
			(FieldKind::Select { multi, options }, Some(Answer::Values(values))) => {
				select(&self.runtime, field, options, *multi, values.clone(), missing)
			}
			_ => return div(),
		};
		column()
			.w_full()
			.child(
				row()
					.gap(tok("space_4"))
					.child(kit::caption(&field.label))
					.when(field.required, |row| {
						row.child(
							text("*")
								.text_size(tok("font_size_xs"))
								.line_height(tok("line_height_xs"))
								.font_weight(600)
								.text_color(tok("red_2")),
						)
					}),
			)
			.when_some(field.description.clone(), |column, description| {
				column.child(kit::note(description).mt(tok("space_4")))
			})
			.child(input.mt(tok("space_8")))
			.when(missing, |column| {
				column.child(
					text(REQUIRED)
						.mt(tok("space_4"))
						.text_size(tok("font_size_xs"))
						.line_height(tok("line_height_xs"))
						.text_color(tok("red_2")),
				)
			})
	}
}

fn app_icon(name: &str) -> View {
	let initial: String = name
		.chars()
		.find(|character| character.is_alphanumeric())
		.map(|character| character.to_uppercase().collect())
		.unwrap_or_default();
	div()
		.size(APP_ICON, APP_ICON)
		.shrink_0()
		.rounded(APP_ICON / 2.)
		.bg(tok("accent_3"))
		.items_center()
		.justify_center()
		.child(
			text(initial)
				.text_size(APP_ICON * 0.45)
				.font_weight(700)
				.text_color(tok("text_0")),
		)
}

fn outlined(view: View, missing: bool) -> View {
	if missing {
		view.border_color(tok("red_2"))
	} else {
		view
	}
}

fn short(field: &ModalField, value: Signal<String>, missing: bool) -> View {
	let input = kit::input(
		&field.label,
		value,
		field.placeholder.as_deref().unwrap_or_default(),
	)
	.id(format!("app-modal-field-{}", field.custom_id));
	outlined(input, missing)
}

fn paragraph(field: &ModalField, value: Signal<String>, missing: bool) -> View {
	let counted = value.clone();
	let limit = field.max_length;
	let area = kit::textarea(
		&field.label,
		value,
		field.placeholder.as_deref().unwrap_or_default(),
	)
	.id(format!("app-modal-field-{}", field.custom_id))
	.h(tok("app_modal_paragraph_height"));
	div()
		.relative()
		.w_full()
		.child(outlined(area, missing))
		.when_some(limit, move |frame, limit| {
			frame.child(
				text_signal(move || format!("{} / {limit}", counted.with(|text| text.chars().count())))
					.absolute()
					.right(8.)
					.bottom(6.)
					.text_size(tok("font_size_xs"))
					.line_height(tok("line_height_xs"))
					.text_color(tok("text_5")),
			)
		})
}

fn select(
	runtime: &Runtime,
	field: &ModalField,
	options: &[ChoiceOption],
	multi: bool,
	values: Signal<Vec<String>>,
	missing: bool,
) -> View {
	let runtime_open = runtime.signal(false);
	let (shown, labels) = (values.clone(), options.to_vec());
	let placeholder = field
		.placeholder
		.clone()
		.unwrap_or_else(|| "Make a selection".to_owned());
	let toggle = runtime_open.clone();
	let anchor = button()
		.id(format!("app-modal-field-{}", field.custom_id))
		.w_full()
		.h(tok("component_select_height"))
		.pl(tok("space_12"))
		.pr(tok("space_8"))
		.gap(tok("space_8"))
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok(if missing { "red_2" } else { "hover" }))
		.cursor(Cursor::Pointer)
		.on_click(move || {
			toggle.set(!toggle.with_untracked(|open| *open));
		})
		.child(
			text_signal(move || {
				let picked: Vec<String> = shown.with(|values| {
					labels
						.iter()
						.filter(|option| values.contains(&option.value))
						.map(|option| option.label.clone())
						.collect()
				});
				if picked.is_empty() {
					placeholder.clone()
				} else {
					picked.join(", ")
				}
			})
			.grow()
			.min_w(0.)
			.truncate()
			.text_size(tok("font_size_sm"))
			.line_height(tok("line_height_sm"))
			.font_weight(500)
			.reactive_style({
				let values = values.clone();
				move || {
					Styles::new().text_color(tok(if values.with(Vec::is_empty) {
						"text_5"
					} else {
						"text_3"
					}))
				}
			}),
		)
		.child(icon("chevron_down", 20., "text_3"));
	let rows: Vec<View> = options
		.iter()
		.map(|option| {
			let (pick, chosen, close) = (values.clone(), values.clone(), runtime_open.clone());
			let value = option.value.clone();
			let marked = value.clone();
			kit::menu_row(option.label.clone(), div(), "text_3")
				.menu_checked_when(move || chosen.with(|values| values.contains(&marked)))
				.on_click(move || {
					let mut current = pick.with_untracked(Clone::clone);
					match current.iter().position(|picked| *picked == value) {
						Some(at) => {
							current.remove(at);
						}
						None if multi => current.push(value.clone()),
						None => current = vec![value.clone()],
					}
					pick.set(current);
					if !multi {
						close.set(false);
					}
				})
		})
		.collect();
	kit::menu_panel(
		menu(format!("{} options", field.label), runtime_open, anchor)
			.placement(Placement {
				side: PlacementSide::Bottom,
				align: PlacementEdge::Start,
				offset: 4.,
			})
			.match_anchor_width(),
	)
	.w_full()
	.children(rows)
}
