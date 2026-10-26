use std::cell::RefCell;
use std::rc::{Rc, Weak};

use zgx::compose::prelude::*;
use zgx::reactive::{Runtime, Signal};

use crate::timeline::tok;

struct Entry {
	alive: Weak<()>,
	open: Signal<bool>,
}

thread_local! {
	static ENTRIES: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
	static CHANGED: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
}

pub fn track(open: &Signal<bool>) -> Rc<()> {
	let alive = Rc::new(());
	ENTRIES.with(|entries| {
		let mut entries = entries.borrow_mut();
		entries.retain(|entry| entry.alive.strong_count() > 0);
		entries.push(Entry {
			alive: Rc::downgrade(&alive),
			open: open.clone(),
		});
	});
	CHANGED.with(|changed| {
		if let Some(changed) = changed.borrow().as_ref() {
			changed.update(|count| *count += 1);
		}
	});
	alive
}

fn any_open() -> bool {
	CHANGED.with(|changed| {
		if let Some(changed) = changed.borrow().as_ref() {
			changed.get();
		}
	});
	let open: Vec<Signal<bool>> = ENTRIES.with(|entries| {
		entries
			.borrow()
			.iter()
			.filter(|entry| entry.alive.strong_count() > 0)
			.map(|entry| entry.open.clone())
			.collect()
	});
	open.iter().any(Signal::get)
}

fn size(name: &str) -> f32 {
	bridge::themes::SIZES
		.iter()
		.find(|(key, _)| *key == name)
		.map_or(0., |(_, value)| *value)
}

pub struct Scrim {
	window: Signal<(f32, f32)>,
}

impl Scrim {
	pub fn new(runtime: &Runtime) -> Rc<Self> {
		let changed = runtime.signal(0);
		CHANGED.with(|slot| *slot.borrow_mut() = Some(changed));
		Rc::new(Self {
			window: runtime.signal((0., 0.)),
		})
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		let window = self.window.clone();
		div().child_when(
			move || {
				let (width, height) = window.get();
				any_open().then_some((width, height))
			},
			|(width, height): (f32, f32), _| around_controls(width, height),
		)
	}
}

fn around_controls(width: f32, height: f32) -> View {
	let hole_width = size("window_controls_width");
	let hole_top = size("window_controls_top");
	let hole_height = size("window_control_hit");
	let hole_left = (width - size("window_controls_right") - hole_width).max(0.);
	let hole_bottom = hole_top + hole_height;
	let band = |left: f32, top: f32, w: f32, h: f32| {
		div()
			.absolute()
			.left(left)
			.top(top)
			.w(w.max(0.))
			.h(h.max(0.))
			.bg(tok("overlay"))
	};
	div()
		.child(band(0., 0., width, hole_top))
		.child(band(0., hole_bottom, width, height - hole_bottom))
		.child(band(0., hole_top, hole_left, hole_height))
		.child(band(
			hole_left + hole_width,
			hole_top,
			width - hole_left - hole_width,
			hole_height,
		))
}
