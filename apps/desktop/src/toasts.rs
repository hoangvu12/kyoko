use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use bridge::{ToastKind, Toasts};
use zgx::compose::prelude::*;
use zgx::compose::{TaskRunner, Tasks};
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use crate::motion::{self, Tween};
use crate::timeline::{TypeSize, icon, tok};

const TOP: f32 = 50.;
const RIGHT: f32 = 16.;
const WIDTH: f32 = 382.;
const GAP: f32 = 12.;
const PADDING_X: f32 = 16.;
const PADDING_Y: f32 = 14.;
const ICON: f32 = 16.;
const ICON_TOP: f32 = 18.;
const TEXT_INSET: f32 = 24.;
const CLOSE: f32 = 20.;
const CLOSE_ICON: f32 = 12.;
const CLOSE_EDGE: f32 = 4.;

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

pub struct ToastLayer {
	toasts: RefCell<Toasts>,
	shown: Signal<Vec<Shown>>,
	window: Signal<(f32, f32)>,
	tasks: RefCell<Option<Tasks>>,
	timer: Cell<u64>,
}

impl ToastLayer {
	pub fn new(runtime: &Runtime) -> Rc<Self> {
		Rc::new(Self {
			toasts: RefCell::default(),
			shown: runtime.signal(Vec::new()),
			window: runtime.signal((0., 0.)),
			tasks: RefCell::new(None),
			timer: Cell::new(0),
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

	pub fn push(self: &Rc<Self>, kind: ToastKind, text: &str) {
		if text.trim().is_empty() {
			return;
		}
		self.toasts.borrow_mut().push(kind, text, Instant::now());
		self.changed();
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
		let this = self.clone();
		component(move |cx| {
			if cx.try_service::<TaskRunner>().is_some() {
				*this.tasks.borrow_mut() = Some(cx.tasks());
			}
			let (sized, window, listed) = (this.shown.clone(), this.window.clone(), this.shown.clone());
			let (hover, rows) = (Rc::downgrade(&this), this.clone());
			column()
				.gap(GAP)
				.reactive_style(move || {
					if sized.with(Vec::is_empty) {
						return Styles::new().absolute().left(0.).top(0.).w(0.).h(0.);
					}
					let (width, _) = window.get();
					Styles::new()
						.absolute()
						.left((width - RIGHT - WIDTH).max(0.))
						.top(TOP)
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
		let tween = Tween::new(cx, 0.);
		tween.run(0., 1., motion::TOAST_ENTER);
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
		div()
			.relative()
			.w_full()
			.px(PADDING_X)
			.py(PADDING_Y)
			.rounded(tok("radius_md"))
			.bg(tok("background_floating"))
			.border(1.)
			.border_color(tok("divider"))
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
			.child(
				div()
					.absolute()
					.top(ICON_TOP)
					.left(PADDING_X)
					.child(kind_icon(toast.kind)),
			)
			.child(
				text(toast.text)
					.w_full()
					.pl(TEXT_INSET)
					.text_wrap(true)
					.type_size("font_sm")
					.text_color(tok("text_normal")),
			)
			.child_when(
				move || shown.get().then_some(()),
				move |(), _| close_button(layer.clone(), id),
			)
	}
}

fn kind_icon(kind: ToastKind) -> View {
	match kind {
		ToastKind::Info => icon("info", ICON, "brand"),
		ToastKind::Success => icon("circle_check", ICON, "success"),
		ToastKind::Warning => icon("warning", ICON, "warning"),
		ToastKind::Error => icon("circle_x", ICON, "danger"),
	}
}

fn close_button(layer: Weak<ToastLayer>, id: u64) -> View {
	div()
		.absolute()
		.top(CLOSE_EDGE)
		.right(CLOSE_EDGE)
		.size(CLOSE, CLOSE)
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("modifier_hover")))
		.on_click(move || {
			if let Some(layer) = layer.upgrade() {
				layer.dismiss(id);
			}
		})
		.child(icon("close", CLOSE_ICON, "interactive_normal"))
		.tooltip("Dismiss".to_owned())
}
