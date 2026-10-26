use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use zgx::compose::prelude::*;
use zgx::compose::{TaskRunner, Tasks};
use zgx::frame::FrameClock;
use zgx::reactive::Signal;

const DIALOG_TRAVEL: f32 = 80.;
const POPOVER_TRAVEL: f32 = 8.;
const TOAST_TRAVEL: f32 = 24.;
const SOLVER_STEPS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
	Dialog,
	OutCubic,
	Standard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
	pub duration: Duration,
	pub ease: Ease,
	pub travel: f32,
}

pub const DIALOG: Transition = Transition {
	duration: Duration::from_millis(250),
	ease: Ease::Dialog,
	travel: -DIALOG_TRAVEL,
};

pub const POPOVER: Transition = Transition {
	duration: Duration::from_millis(150),
	ease: Ease::OutCubic,
	travel: -POPOVER_TRAVEL,
};

pub const TOAST_ENTER: Transition = Transition {
	duration: Duration::from_millis(400),
	ease: Ease::Standard,
	travel: TOAST_TRAVEL,
};

pub const TOAST_EXIT: Transition = Transition {
	duration: bridge::TOAST_EXIT,
	ease: Ease::Standard,
	travel: TOAST_TRAVEL,
};

impl Ease {
	pub fn at(self, t: f32) -> f32 {
		let t = t.clamp(0., 1.);
		match self {
			Ease::Dialog => bezier_y(0.72, 1., t),
			Ease::OutCubic => 1. - (1. - t).powi(3),
			Ease::Standard => cubic_bezier((0.25, 0.1), (0.25, 1.), t),
		}
	}
}

fn bezier_y(y1: f32, y2: f32, s: f32) -> f32 {
	let r = 1. - s;
	3. * r * r * s * y1 + 3. * r * s * s * y2 + s * s * s
}

fn cubic_bezier((x1, y1): (f32, f32), (x2, y2): (f32, f32), x: f32) -> f32 {
	let (mut low, mut high) = (0f32, 1f32);
	for _ in 0..SOLVER_STEPS {
		let middle = (low + high) / 2.;
		if bezier_y(x1, x2, middle) < x {
			low = middle;
		} else {
			high = middle;
		}
	}
	bezier_y(y1, y2, (low + high) / 2.)
}

#[derive(Clone)]
pub struct Motion {
	reduced: Signal<bool>,
}

impl Motion {
	pub fn new(reduced: Signal<bool>) -> Self {
		Self { reduced }
	}

	pub fn set_reduced(&self, reduced: bool) {
		if self.reduced.with_untracked(|current| *current != reduced) {
			self.reduced.set(reduced);
		}
	}
}

#[derive(Clone)]
pub struct Tween {
	value: Signal<f32>,
	reduced: Option<Signal<bool>>,
	clock: Option<(FrameClock, Tasks)>,
	run: Rc<Cell<u64>>,
}

impl Tween {
	pub fn new(cx: &mut Context, at: f32) -> Self {
		let reduced = cx.try_service::<Motion>().map(|motion| motion.reduced.clone());
		let clock = match (cx.try_service::<FrameClock>(), cx.try_service::<TaskRunner>()) {
			(Some(frames), Some(_)) => Some(((*frames).clone(), cx.tasks())),
			_ => None,
		};
		Self {
			value: cx.state(at),
			reduced,
			clock,
			run: Rc::default(),
		}
	}

	pub fn run(&self, from: f32, to: f32, transition: Transition) {
		let run = self.run.get() + 1;
		self.run.set(run);
		let Some((frames, tasks)) = self.clock.clone() else {
			self.value.set(to);
			return;
		};
		self.value.set(from);
		let (value, current) = (self.value.clone(), self.run.clone());
		let length = transition.duration.as_secs_f32().max(f32::EPSILON);
		tasks.spawn(async move {
			let start = frames.next().await.time;
			loop {
				let frame = frames.next().await;
				if current.get() != run {
					return;
				}
				let t = (frame.time.saturating_duration_since(start).as_secs_f32() / length).min(1.);
				value.set(from + (to - from) * transition.ease.at(t));
				if t >= 1. {
					return;
				}
			}
		});
	}

	pub fn styles(&self, transition: Transition, styles: Styles) -> Styles {
		let shown = self.value.get();
		let still = self.reduced.as_ref().is_some_and(Signal::get);
		let styles = styles.opacity(shown);
		if still {
			styles
		} else {
			styles.translate(0., transition.travel * (1. - shown))
		}
	}
}

pub fn modal(label: impl Into<String>, open: Signal<bool>) -> View {
	entering(zgx::compose::modal(label, open.clone()), open, DIALOG)
}

pub fn popover(label: impl Into<String>, open: Signal<bool>, anchor: View) -> View {
	entering(zgx::compose::popover(label, open.clone(), anchor), open, POPOVER)
}

fn entering(view: View, open: Signal<bool>, transition: Transition) -> View {
	component(move |cx| {
		let mut was = open.with_untracked(|open| *open);
		let tween = Tween::new(cx, 1.);
		let opened = tween.clone();
		let tracked = crate::scrim::track(&open);
		let effect = cx.runtime().effect(move || {
			let _ = &tracked;
			let now = open.get();
			if now && !was {
				opened.run(0., 1., transition);
			}
			was = now;
		});
		cx.retain(effect);
		view.reactive_style(move || tween.styles(transition, Styles::new()))
	})
}
