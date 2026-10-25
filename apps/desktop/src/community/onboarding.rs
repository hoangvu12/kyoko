use std::cell::RefCell;
use std::rc::Rc;

use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use state::{FieldKind, OnboardingOption, OnboardingPrompt};
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::CommunityLayer;
use super::parts::{Tone, action_button, body, caption, enabled_when, footer, hint, panel};
use crate::timeline::{TypeSize, icon, tok};

const WIDTH: f32 = 540.;
const CHECKBOX: f32 = 20.;
const LIST_MAX: f32 = 360.;
const ELSEWHERE: &str =
	"This server asks questions kyoko can't show yet. Answer them in the official client.";

struct Flow {
	layer: Rc<CommunityLayer>,
	guild_id: Snowflake,
	prompts: Vec<OnboardingPrompt>,
	step: Signal<usize>,
	chosen: Signal<Vec<Snowflake>>,
	prompts_seen: RefCell<Vec<(Snowflake, i64)>>,
	options_seen: RefCell<Vec<(Snowflake, i64)>>,
}

impl Flow {
	fn saw(&self, prompt: &OnboardingPrompt) {
		let now = Utc::now().timestamp_millis();
		let mut prompts = self.prompts_seen.borrow_mut();
		if prompts.iter().any(|(id, _)| *id == prompt.id) {
			return;
		}
		prompts.push((prompt.id, now));
		self.options_seen
			.borrow_mut()
			.extend(prompt.options.iter().map(|option| (option.id, now)));
	}

	fn answered(&self, prompt: &OnboardingPrompt, chosen: &[Snowflake]) -> bool {
		!prompt.required || prompt.options.iter().any(|option| chosen.contains(&option.id))
	}

	fn pick(&self, prompt: &OnboardingPrompt, option: Snowflake) {
		let mut chosen = self.chosen.with_untracked(Clone::clone);
		if let Some(index) = chosen.iter().position(|id| *id == option) {
			chosen.remove(index);
		} else {
			if prompt.single_select {
				chosen.retain(|id| prompt.options.iter().all(|other| other.id != *id));
			}
			chosen.push(option);
		}
		self.chosen.set(chosen);
	}

	fn finish(&self) {
		let chosen = self.chosen.with_untracked(Clone::clone);
		if !self.prompts.iter().all(|prompt| self.answered(prompt, &chosen)) {
			return;
		}
		self.layer.send(Command::SubmitOnboarding {
			guild_id: self.guild_id,
			chosen,
			prompts_seen: self.prompts_seen.borrow().clone(),
			options_seen: self.options_seen.borrow().clone(),
		});
		self.layer.close();
	}
}

fn guild_name(layer: &CommunityLayer, guild_id: Snowflake) -> String {
	layer
		.read(|state| state.guild(guild_id).map(|guild| guild.name.clone()))
		.flatten()
		.unwrap_or_default()
}

pub fn view(layer: &Rc<CommunityLayer>, guild_id: Snowflake) -> View {
	let onboarding = layer.read(|state| state.onboarding(guild_id).cloned()).flatten();
	let close = layer.clone();
	let title = format!("Welcome to {}", guild_name(layer, guild_id));
	let Some(onboarding) = onboarding.filter(|onboarding| !onboarding.prompts.is_empty()) else {
		return panel(
			WIDTH,
			title,
			move || close.close(),
			body().child(hint("This server has no questions to answer.")),
		);
	};
	let offered: Vec<Snowflake> = onboarding
		.prompts
		.iter()
		.flat_map(|prompt| prompt.options.iter().map(|option| option.id))
		.collect();
	let flow = Rc::new(Flow {
		layer: layer.clone(),
		guild_id,
		chosen: layer.runtime.signal(
			onboarding
				.chosen
				.iter()
				.copied()
				.filter(|id| offered.contains(id))
				.collect(),
		),
		prompts: onboarding.prompts,
		step: layer.runtime.signal(0),
		prompts_seen: RefCell::default(),
		options_seen: RefCell::default(),
	});
	let shown = flow.clone();
	panel(
		WIDTH,
		title,
		move || close.close(),
		switch(
			{
				let step = flow.step.clone();
				move || step.get()
			},
			move |step: usize, _| step_view(&shown, step),
		)
		.w_full(),
	)
}

fn step_view(flow: &Rc<Flow>, step: usize) -> View {
	let prompt = flow.prompts[step].clone();
	flow.saw(&prompt);
	let last = step + 1 == flow.prompts.len();
	let note = match (prompt.required, prompt.single_select) {
		(true, true) => "Required. Pick one.",
		(true, false) => "Required. Pick as many as you like.",
		(false, true) => "Pick one, or skip.",
		(false, false) => "Pick as many as you like, or skip.",
	};
	let options = prompt
		.options
		.iter()
		.map(|option| option_view(flow, &prompt, option))
		.collect::<Vec<_>>();
	let next = {
		let flow = flow.clone();
		let prompt = prompt.clone();
		move || {
			let chosen = flow.chosen.with_untracked(Clone::clone);
			if !flow.answered(&prompt, &chosen) {
				return;
			}
			if last {
				flow.finish();
			} else {
				flow.step.set(step + 1);
			}
		}
	};
	let ready = {
		let flow = flow.clone();
		let prompt = prompt.clone();
		move || flow.chosen.with(|chosen| flow.answered(&prompt, chosen))
	};
	let confirm = enabled_when(
		action_button(if last { "Finish" } else { "Next" }, Tone::Primary, next),
		ready,
	);
	let back = flow.clone();
	let cancel = flow.layer.clone();
	let actions = if step > 0 {
		row()
			.w_full()
			.items_center()
			.justify_between()
			.child(action_button("Back", Tone::Ghost, move || {
				back.step.set(step - 1);
			}))
			.child(confirm)
	} else {
		footer("Not now", confirm, move || cancel.close())
	};
	body()
		.child(
			column()
				.w_full()
				.gap(4.)
				.child(caption(&format!(
					"Question {} of {}",
					step + 1,
					flow.prompts.len()
				)))
				.child(
					text(prompt.title.clone())
						.w_full()
						.text_wrap(true)
						.type_size("font_lg")
						.font_weight(600)
						.text_color(tok("header_primary")),
				)
				.child(hint(note)),
		)
		.child(
			scroll(flow.layer.runtime.signal(0.))
				.w_full()
				.max_h(LIST_MAX)
				.child(column().w_full().gap(8.).children(options)),
		)
		.child(actions)
}

fn option_view(flow: &Rc<Flow>, prompt: &OnboardingPrompt, option: &OnboardingOption) -> View {
	let id = option.id;
	let (chosen, pick, prompt) = (flow.chosen.clone(), flow.clone(), prompt.clone());
	let glyph = option
		.emoji
		.as_ref()
		.filter(|emoji| emoji.id.is_none())
		.and_then(|emoji| emoji.name.clone());
	row()
		.w_full()
		.items_center()
		.gap(12.)
		.p(12.)
		.rounded(tok("radius_md"))
		.border(1.)
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if chosen.with(|chosen| chosen.contains(&id)) {
				Styles::new().border_color(tok("brand")).bg(tok("brand_wash"))
			} else {
				Styles::new()
					.border_color(tok("divider"))
					.bg(tok("background_secondary"))
			}
		})
		.hover(|style| style.bg(tok("modifier_hover")))
		.on_click(move || pick.pick(&prompt, id))
		.when_some(glyph, |row, glyph| {
			row.child(text(glyph).shrink_0().type_size("font_lg"))
		})
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					text(option.title.clone())
						.w_full()
						.text_wrap(true)
						.type_size("font_md")
						.font_weight(600)
						.text_color(tok("header_primary")),
				)
				.when_some(option.description.clone(), |column, description| {
					column.child(hint(description))
				}),
		)
}

pub fn screening(layer: &Rc<CommunityLayer>, guild_id: Snowflake) -> View {
	let verification = layer
		.read(|state| state.member_verification(guild_id).cloned())
		.flatten();
	let close = layer.clone();
	let title = format!("Before you talk in {}", guild_name(layer, guild_id));
	let Some(verification) = verification else {
		return panel(
			WIDTH,
			title,
			move || close.close(),
			body().child(hint("Loading the server rules...")),
		);
	};
	let mut content = body();
	if let Some(description) = verification.description.clone() {
		content = content.child(hint(description));
	}
	for field in &verification.fields {
		let mut rules = column().w_full().gap(8.).child(caption(&field.label));
		for (index, rule) in field.rules.iter().enumerate() {
			rules = rules.child(
				text(format!("{}. {rule}", index + 1))
					.w_full()
					.text_wrap(true)
					.type_size("font_sm")
					.text_color(tok("text_normal")),
			);
		}
		if field.kind != FieldKind::Terms {
			rules = rules.child(hint(ELSEWHERE));
		}
		content = content.child(rules);
	}
	let cancel = layer.clone();
	if !verification.only_terms() {
		return panel(
			WIDTH,
			title,
			move || close.close(),
			content.child(footer(
				"Close",
				action_button("Got it", Tone::Primary, {
					let layer = layer.clone();
					move || layer.close()
				}),
				move || cancel.close(),
			)),
		);
	}
	let agreed = layer.runtime.signal(false);
	let submit = {
		let (layer, agreed) = (layer.clone(), agreed.clone());
		move || {
			if agreed.with_untracked(|agreed| *agreed) {
				layer.send(Command::AcceptMemberVerification { guild_id });
				layer.close();
			}
		}
	};
	let ready = {
		let agreed = agreed.clone();
		move || agreed.get()
	};
	panel(
		WIDTH,
		title,
		move || close.close(),
		content.child(agree_box(agreed)).child(footer(
			"Cancel",
			enabled_when(action_button("Submit", Tone::Primary, submit), ready),
			move || cancel.close(),
		)),
	)
}

fn agree_box(agreed: Signal<bool>) -> View {
	let (shown, ticked, flip) = (agreed.clone(), agreed.clone(), agreed);
	row()
		.w_full()
		.items_center()
		.gap(8.)
		.cursor(Cursor::Pointer)
		.on_click(move || {
			flip.set(!flip.with_untracked(|agreed| *agreed));
		})
		.child(
			div()
				.size(CHECKBOX, CHECKBOX)
				.shrink_0()
				.rounded(tok("radius_sm"))
				.border(1.)
				.items_center()
				.justify_center()
				.reactive_style(move || {
					if shown.get() {
						Styles::new().bg(tok("brand")).border_color(tok("brand"))
					} else {
						Styles::new()
							.bg(tok("input_background"))
							.border_color(tok("interactive_muted"))
					}
				})
				.child(
					div()
						.child(icon("check", 14., "text_on_brand"))
						.reactive_style(move || {
							if ticked.get() {
								Styles::new().visible()
							} else {
								Styles::new().invisible()
							}
						}),
				),
		)
		.child(
			text("I have read and agree to the rules".to_owned())
				.type_size("font_sm")
				.text_color(tok("text_normal")),
		)
}
