use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use bridge::{ToastKind, Toasts};
use zgx::compose::prelude::*;
use zgx::compose::{ModalPlacement, PlacementEdge, TaskRunner, Tasks, modal};
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use crate::motion::{self, Tween};
use crate::timeline::{icon, tok};

const WIDTH: f32 = 360.;
const GAP: f32 = 8.;
const ABOVE_AREA: f32 = 8.;
const ABOVE_WINDOW: f32 = 63.;
const BADGE: f32 = 20.;
const GLYPH: f32 = 14.;
const CLOSE: f32 = 20.;
const CLOSE_ICON: f32 = 12.;

thread_local! {
	static LAYER: RefCell<Weak<ToastLayer>> = RefCell::default();
}

pub fn show(kind: ToastKind, text: &str) {
	let layer = LAYER.with(|layer| layer.borrow().upgrade());
	match layer {
		Some(layer) => layer.push(kind, text),
		None => tracing::debug!(?kind, "no toast layer to show a toast"),
	}
}

pub fn copy(text: &str, copied: &str) {
	match platform::clipboard::write_text(text) {
		Ok(()) => show(ToastKind::Success, copied),
		Err(error) => {
			tracing::warn!(%error, "the clipboard refused the text");
			show(ToastKind::Error, "Could not copy to the clipboard.");
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
struct Shown {
	id: u64,
	kind: ToastKind,
	text: String,
	leaving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

pub struct ToastLayer {
	toasts: RefCell<Toasts>,
	shown: Signal<Vec<Shown>>,
	window: Signal<(f32, f32)>,
	area: Signal<Option<Area>>,
	tasks: RefCell<Option<Tasks>>,
	timer: Cell<u64>,
	raised: Signal<u64>,
	entered: RefCell<HashSet<u64>>,
}

impl ToastLayer {
	pub fn new(runtime: &Runtime) -> Rc<Self> {
		Rc::new(Self {
			toasts: RefCell::default(),
			shown: runtime.signal(Vec::new()),
			window: runtime.signal((0., 0.)),
			area: runtime.signal(None),
			tasks: RefCell::new(None),
			timer: Cell::new(0),
			raised: runtime.signal(0),
			entered: RefCell::default(),
		})
	}

	pub fn install(self: &Rc<Self>) {
		LAYER.with(|layer| *layer.borrow_mut() = Rc::downgrade(self));
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn set_area(&self, area: Option<Area>) {
		if self.area.with_untracked(|current| *current != area) {
			self.area.set(area);
		}
	}

	fn anchor(&self) -> (f32, f32) {
		let (width, height) = self.window.get();
		match self.area.get() {
			Some(area) if area.width > 0. && area.height > 0. => {
				(area.x + area.width / 2., area.y + area.height - ABOVE_AREA)
			}
			_ => (width / 2., height - ABOVE_WINDOW),
		}
	}

	pub fn push(self: &Rc<Self>, kind: ToastKind, text: &str) {
		if text.trim().is_empty() {
			return;
		}
		self.toasts.borrow_mut().push(kind, text, Instant::now());
		self.changed();
		let next = self.raised.with_untracked(|raised| raised + 1);
		self.raised.set(next);
	}

	pub fn new_rejection(&self, state: &state::State) -> Option<String> {
		self.toasts.borrow_mut().new_rejection(state)
	}

	pub fn forget_rejections(&self) {
		self.toasts.borrow_mut().forget_rejections();
	}

	pub fn showing(&self) -> Vec<(ToastKind, String)> {
		self.shown.with_untracked(|shown| {
			shown
				.iter()
				.filter(|toast| !toast.leaving)
				.map(|toast| (toast.kind, toast.text.clone()))
				.collect()
		})
	}

	fn dismiss(self: &Rc<Self>, id: u64) {
		if self.toasts.borrow_mut().dismiss(id, Instant::now()) {
			self.changed();
		}
	}

	fn hovered(self: &Rc<Self>, inside: bool) {
		let now = Instant::now();
		match inside {
			true => self.toasts.borrow_mut().pause(now),
			false => self.toasts.borrow_mut().resume(now),
		}
		self.schedule();
	}

	fn tick(self: &Rc<Self>) {
		if self.toasts.borrow_mut().tick(Instant::now()) {
			self.publish();
		}
		self.schedule();
	}

	fn changed(self: &Rc<Self>) {
		self.publish();
		self.schedule();
	}

	fn publish(&self) {
		let shown: Vec<Shown> = self
			.toasts
			.borrow()
			.items()
			.iter()
			.map(|toast| Shown {
				id: toast.id,
				kind: toast.kind,
				text: toast.text.clone(),
				leaving: toast.leaving,
			})
			.collect();
		self.entered
			.borrow_mut()
			.retain(|id| shown.iter().any(|toast| toast.id == *id));
		if self.shown.with_untracked(|current| *current != shown) {
			self.shown.set(shown);
		}
	}

	fn schedule(self: &Rc<Self>) {
		let timer = self.timer.get() + 1;
		self.timer.set(timer);
		let Some(deadline) = self.toasts.borrow().next_deadline() else {
			return;
		};
		let Some(tasks) = self.tasks.borrow().clone() else {
			return;
		};
		let layer = Rc::downgrade(self);
		tasks.spawn(async move {
			let wait = deadline.saturating_duration_since(Instant::now());
			zgx::timer::sleep(wait.max(Duration::from_millis(1))).await;
			if let Some(layer) = layer.upgrade()
				&& layer.timer.get() == timer
			{
				layer.tick();
			}
		});
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (this, raised) = (self.clone(), self.raised.clone());
		div().child_when(
			move || Some(raised.get()),
			move |_, cx| {
				modal("Notifications", cx.runtime().signal(true))
					.layer()
					.autofocus(false)
					.modal_placement(ModalPlacement {
						x: PlacementEdge::Start,
						y: PlacementEdge::Start,
						insets: [0.; 4],
					})
					.w(0.)
					.h(0.)
					.p(0.)
					.bg(zgx::scene::Color(0, 0, 0, 0))
					.border(0.)
					.shadow_none()
					.child(this.stack())
			},
		)
	}

	fn stack(self: &Rc<Self>) -> View {
		let this = self.clone();
		component(move |cx| {
			if cx.try_service::<TaskRunner>().is_some() {
				*this.tasks.borrow_mut() = Some(cx.tasks());
			}
			let (sized, placed, listed) = (this.shown.clone(), this.clone(), this.shown.clone());
			let (hover, rows) = (Rc::downgrade(&this), this.clone());
			column()
				.gap(GAP)
				.reactive_style(move || {
					if sized.with(Vec::is_empty) {
						return Styles::new().absolute().left(0.).top(0.).w(0.).h(0.);
					}
					let (center, bottom) = placed.anchor();
					Styles::new()
						.absolute()
						.left((center - WIDTH / 2.).max(0.))
						.bottom(-bottom)
						.w(WIDTH)
				})
				.on_event(move |event| {
					let inside = match event.event {
						InputEvent::PointerEnter => true,
						InputEvent::PointerLeave => false,
						_ => return,
					};
					if let Some(layer) = hover.upgrade() {
						layer.hovered(inside);
					}
				})
				.child(keyed(
					move || listed.with(|shown| shown.iter().map(|toast| toast.id).collect()),
					move |id: u64, cx| rows.row(id, cx),
				))
		})
	}

	fn row(self: &Rc<Self>, id: u64, cx: &mut Context) -> View {
		let Some(toast) = self
			.shown
			.with_untracked(|shown| shown.iter().find(|toast| toast.id == id).cloned())
		else {
			return div();
		};
		let entering = self.entered.borrow_mut().insert(id);
		let tween = Tween::new(cx, if entering { 0. } else { 1. });
		if entering {
			tween.run(0., 1., motion::TOAST_ENTER);
		}
		let (leaving, fading) = (self.shown.clone(), tween.clone());
		let mut left = false;
		let effect = cx.runtime().effect(move || {
			let now = leaving.with(|shown| {
				shown
					.iter()
					.find(|toast| toast.id == id)
					.is_some_and(|toast| toast.leaving)
			});
			if now && !left {
				fading.run(1., 0., motion::TOAST_EXIT);
			}
			left = now;
		});
		cx.retain(effect);
		let hovered = cx.state(false);
		let (hover, shown, layer) = (hovered.clone(), hovered.clone(), Rc::downgrade(self));
		row()
			.id(format!("toast-{id}"))
			.w_full()
			.items_center()
			.gap(tok("space_10"))
			.px(tok("space_12"))
			.py(tok("space_10"))
			.rounded(tok("radius_sm"))
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("active"))
			.reactive_style(move || tween.styles(motion::TOAST_ENTER, Styles::new()))
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					hover.set(true);
				}
				InputEvent::PointerLeave => {
					hover.set(false);
				}
				_ => {}
			})
			.child(kind_badge(toast.kind))
			.child(
				text(toast.text)
					.grow()
					.min_w(0.)
					.text_wrap(true)
					.text_size(tok("font_size_sm"))
					.line_height(tok("line_height_sm"))
					.font_weight(500)
					.text_color(tok("text_2")),
			)
			.child_when(
				move || shown.get().then_some(()),
				move |(), _| close_button(layer.clone(), id),
			)
	}
}

fn kind_badge(kind: ToastKind) -> View {
	let (glyph, color) = match kind {
		ToastKind::Info => ("info", "blue_2"),
		ToastKind::Success => ("check", "green_2"),
		ToastKind::Warning => ("warning", "yellow_2"),
		ToastKind::Error => ("close", "red_2"),
	};
	div()
		.size(BADGE, BADGE)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(BADGE)
		.bg(tok(color))
		.child(icon(glyph, GLYPH, "text_0"))
}

fn close_button(layer: Weak<ToastLayer>, id: u64) -> View {
	div()
		.size(CLOSE, CLOSE)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_xs"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
		.on_click(move || {
			if let Some(layer) = layer.upgrade() {
				layer.dismiss(id);
			}
		})
		.child(icon("close", CLOSE_ICON, "text_4"))
		.tooltip("Dismiss".to_owned())
}
