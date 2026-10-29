use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use bridge::{FrameHost, FrameRect, Played, ProviderFrames, ProviderPlayer, Sight, StandInFrames};
use zgx::compose::prelude::*;
use zgx::compose::{NodeHandle, Tasks};
use zgx::reactive::{Runtime, Signal};
use zgx::scene::{Rect, Scene};

use super::icons::icon;
use super::links::Links;
use super::rows::tok;

const FOLLOW_EVERY: Duration = Duration::from_millis(16);
const GONE_AFTER: u32 = 8;
const SAMPLES: (usize, usize) = (6, 4);
const PILL_BUTTON: f32 = 40.;
const PILL_ICON: f32 = 20.;
const PILL_PAD: f32 = 4.;
const CLOSE_BUTTON: f32 = 24.;
const CLOSE_ICON: f32 = 16.;
const CLOSE_INSET: f32 = 6.;
const BUTTON_HOVER: u32 = 0xffff_ff1f;
const BLACK: u32 = 0x000000;
const WIDGET_GAP: f32 = 12.;
const NO_WEBVIEW: &str = "Opens in your browser: WebView2 isn't installed";

pub struct Players {
	frames: ProviderFrames,
	live: Signal<Option<String>>,
	available: Signal<bool>,
	anchors: RefCell<HashMap<String, NodeHandle>>,
	scale: Cell<f64>,
	misses: Cell<u32>,
	tasks: RefCell<Option<Tasks>>,
	following: Cell<bool>,
}

impl Players {
	pub fn new(runtime: &Runtime) -> Self {
		Self {
			frames: ProviderFrames::new(Box::new(StandInFrames::new(false))),
			live: runtime.signal(None),
			available: runtime.signal(false),
			anchors: RefCell::default(),
			scale: Cell::new(1.),
			misses: Cell::new(0),
			tasks: RefCell::new(None),
			following: Cell::new(false),
		}
	}

	pub fn set_host(&self, host: Box<dyn FrameHost>) {
		self.frames.set_host(host);
		self.settle();
		let available = self.frames.available();
		if self.available.with_untracked(|shown| *shown != available) {
			self.available.set(available);
		}
	}

	pub fn attach(&self, tasks: &Tasks) {
		self.tasks.borrow_mut().get_or_insert_with(|| tasks.clone());
	}

	pub fn set_scale(&self, scale: f64) {
		self.scale.set(scale);
	}

	pub fn live(&self) -> Option<String> {
		self.frames.live()
	}

	pub fn anchor(&self, key: &str) -> Option<Rect> {
		let anchors = self.anchors.borrow();
		let handle = anchors.get(key)?;
		let scene = handle.scene()?;
		let scene = scene.borrow();
		scene.contains(handle.node()).then(|| scene.bounds(handle.node()))
	}

	pub fn play(self: &Rc<Self>, key: &str, player: &ProviderPlayer, links: &Links) {
		match self.frames.play(key, player) {
			Played::Opened => {
				self.misses.set(0);
				self.settle();
				self.follow();
			}
			Played::Browser => {
				self.settle();
				if let Some(link) = &player.link {
					links.open(link);
				}
			}
		}
	}

	pub fn close(&self) {
		self.frames.close();
		self.settle();
	}

	pub fn step(&self) {
		let Some(key) = self.frames.live() else {
			return;
		};
		self.frames.follow(self.sight(&key));
		self.settle();
	}

	fn settle(&self) {
		let live = self.frames.live();
		if self.live.with_untracked(|shown| *shown != live) {
			self.live.set(live);
		}
	}

	fn follow(self: &Rc<Self>) {
		if self.following.replace(true) {
			return;
		}
		let Some(tasks) = self.tasks.borrow().clone() else {
			self.following.set(false);
			return;
		};
		let players: Weak<Self> = Rc::downgrade(self);
		tasks.spawn(async move {
			loop {
				zgx::timer::sleep(FOLLOW_EVERY).await;
				let Some(this) = players.upgrade() else {
					return;
				};
				this.step();
				if this.frames.live().is_none() {
					this.following.set(false);
					return;
				}
			}
		});
	}

	fn remember(&self, key: &str, handle: NodeHandle) {
		self.anchors.borrow_mut().insert(key.to_owned(), handle);
	}

	fn sight(&self, key: &str) -> Sight {
		let mounted = self.anchors.borrow().get(key).and_then(|handle| {
			let scene = handle.scene()?;
			let scene = scene.borrow();
			scene.contains(handle.node()).then(|| self.seen(&scene, handle))
		});
		match mounted {
			Some(sight) => {
				self.misses.set(0);
				sight
			}
			None => {
				let misses = self.misses.get() + 1;
				self.misses.set(misses);
				if misses > GONE_AFTER {
					self.anchors.borrow_mut().remove(key);
					Sight::Gone
				} else {
					Sight::Hidden
				}
			}
		}
	}

	fn seen(&self, scene: &Scene, handle: &NodeHandle) -> Sight {
		let node = handle.node();
		let Some(visible) = scene.visible_bounds(node) else {
			return Sight::Hidden;
		};
		if covered(scene, handle, visible) {
			return Sight::Hidden;
		}
		let scale = self.scale.get();
		Sight::Shown {
			rect: device(scene.bounds(node), scale),
			clip: device(visible, scale),
		}
	}
}

fn covered(scene: &Scene, handle: &NodeHandle, visible: Rect) -> bool {
	let (columns, rows) = SAMPLES;
	let node = handle.node();
	(0..columns).any(|column| {
		(0..rows).any(|row| {
			let x = visible.x + visible.width * (column as f32 + 0.5) / columns as f32;
			let y = visible.y + visible.height * (row as f32 + 0.5) / rows as f32;
			!scene.hit_test(x, y).is_some_and(|hit| scene.is_within(hit, node))
		})
	})
}

fn device(rect: Rect, scale: f64) -> FrameRect {
	let pixels = |points: f32| (f64::from(points) * scale).round() as i32;
	let (left, top) = (pixels(rect.x), pixels(rect.y));
	FrameRect {
		x: left,
		y: top,
		width: pixels(rect.x + rect.width) - left,
		height: pixels(rect.y + rect.height) - top,
	}
}

pub struct Card {
	pub key: String,
	pub player: ProviderPlayer,
	pub links: Rc<Links>,
}

impl Card {
	pub fn video(self, players: &Rc<Players>, poster: impl Fn() -> View + 'static) -> View {
		let (width, height) = self.player.size;
		let players = players.clone();
		let live = players.live.clone();
		let available = players.available.clone();
		let key = self.key.clone();
		column()
			.mt(12.)
			.gap(4.)
			.child(switch(
				move || live.get().as_deref() == Some(key.as_str()),
				move |playing: bool, _| {
					if playing {
						return frame(&players, &self.key, &self.player);
					}
					div()
						.relative()
						.size(width, height)
						.rounded(tok("radius_sm"))
						.overflow_hidden()
						.bg(rgb(BLACK))
						.child(poster())
						.child(
							div()
								.absolute()
								.left(0.)
								.top(0.)
								.w_full()
								.h_full()
								.items_center()
								.justify_center()
								.child(pill(&players, &self.key, &self.player, &self.links)),
						)
				},
			))
			.child(note(available))
	}

	pub fn widget(
		self,
		players: &Rc<Players>,
		art: Option<impl Fn() -> View + 'static>,
		title: String,
	) -> View {
		let (width, height) = self.player.size;
		let side = height.min(width / 3.);
		let players = players.clone();
		let live = players.live.clone();
		let available = players.available.clone();
		let key = self.key.clone();
		column()
			.mt(8.)
			.gap(4.)
			.child(switch(
				move || live.get().as_deref() == Some(key.as_str()),
				move |playing: bool, _| {
					if playing {
						return frame(&players, &self.key, &self.player);
					}
					row()
						.w(width)
						.max_w_percent(100.)
						.h(height)
						.items_center()
						.gap(WIDGET_GAP)
						.pr(WIDGET_GAP)
						.rounded(tok("radius_md"))
						.overflow_hidden()
						.bg(tok("bg_4"))
						.when_some(art.as_ref(), |row, art| {
							row.child(div().size(side, side).shrink_0().child(art()))
						})
						.child(
							text(title.clone())
								.grow()
								.min_w(0.)
								.text_size(tok("font_size_md"))
								.line_height(tok("line_height_label"))
								.font_weight(600)
								.text_color(tok("header_primary")),
						)
						.child(pill(&players, &self.key, &self.player, &self.links))
				},
			))
			.child(note(available))
	}
}

fn note(available: Signal<bool>) -> View {
	switch(
		move || available.get(),
		move |available: bool, _| {
			if available {
				return div();
			}
			text(NO_WEBVIEW)
				.text_size(tok("font_size_xs"))
				.line_height(16.)
				.text_color(tok("text_4"))
		},
	)
}

fn pill(players: &Rc<Players>, key: &str, player: &ProviderPlayer, links: &Rc<Links>) -> View {
	let (players, played, opener) = (players.clone(), player.clone(), links.clone());
	let id = format!("provider-play-{key}");
	let (anchors, anchor) = (players.clone(), id.clone());
	let play = round_button(id, "play_fill")
		.on_mount(move |handle| anchors.remember(&anchor, handle))
		.on_click({
			let (key, links) = (key.to_owned(), links.clone());
			move || players.play(&key, &played, &links)
		});
	let open = player.link.clone().map(|link| {
		round_button(format!("provider-open-{key}"), "external_link").on_click(move || opener.open(&link))
	});
	row()
		.shrink_0()
		.items_center()
		.gap(PILL_PAD)
		.p(PILL_PAD)
		.rounded(PILL_BUTTON)
		.bg(tok("overlay"))
		.child(play)
		.when_some(open, |pill, open| pill.child(open))
}

fn round_button(id: String, name: &'static str) -> View {
	div()
		.id(id)
		.size(PILL_BUTTON, PILL_BUTTON)
		.rounded(PILL_BUTTON / 2.)
		.items_center()
		.justify_center()
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(rgba(BUTTON_HOVER)))
		.child(icon(name, PILL_ICON, "overlay_text"))
}

fn frame(players: &Rc<Players>, key: &str, player: &ProviderPlayer) -> View {
	let (width, height) = player.size;
	let (anchors, anchor) = (players.clone(), key.to_owned());
	div()
		.id(format!("provider-frame-{key}"))
		.on_mount(move |handle| anchors.remember(&anchor, handle))
		.size(width, height)
		.max_w_percent(100.)
		.rounded(tok("radius_sm"))
		.overflow_hidden()
		.bg(rgb(BLACK))
		.items_center()
		.justify_center()
		.child(
			row()
				.items_center()
				.gap(8.)
				.child(icon("spinner", PILL_ICON, "overlay_text"))
				.child(
					text(player.provider.name())
						.text_size(tok("font_size_sm"))
						.text_color(tok("overlay_text")),
				),
		)
}

pub fn close_button(players: &Rc<Players>, key: &str) -> View {
	let live = players.live.clone();
	let (players, key, shown) = (players.clone(), key.to_owned(), key.to_owned());
	switch(
		move || live.get().as_deref() == Some(shown.as_str()),
		move |playing: bool, _| {
			if !playing {
				return div();
			}
			let closer = players.clone();
			div()
				.id(format!("provider-close-{key}"))
				.absolute()
				.top(CLOSE_INSET)
				.right(CLOSE_INSET)
				.size(CLOSE_BUTTON, CLOSE_BUTTON)
				.rounded(CLOSE_BUTTON / 2.)
				.items_center()
				.justify_center()
				.cursor(Cursor::Pointer)
				.bg(tok("bg_4"))
				.hover(|style| style.bg(tok("hover")))
				.on_click(move || closer.close())
				.child(icon("close", CLOSE_ICON, "text_3"))
		},
	)
}
