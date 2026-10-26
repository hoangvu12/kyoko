mod popouts;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use bridge::{PinEntry, ThreadSection};
use media::ImageName;
use protocol::Snowflake;
use zgx::compose::Tasks;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::platform::{FilePicker, PlatformService};
use zgx::reactive::{Runtime, Signal};

use crate::profile::parts;
use crate::timeline::Pictures;

const ANCHOR_RIGHT: f32 = 24.;
const SNAP_MARGIN: f32 = 8.;
const PIN_AVATAR: f64 = 40.;
const ROW_AVATAR: f64 = 16.;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug)]
pub enum Request {
	OpenThread(Snowflake),
	NewThread(Snowflake),
	Jump {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	Unpin {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	ReadFiles(Vec<PathBuf>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoutKind {
	Threads,
	Pins,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Popout {
	serial: u64,
	pub kind: PopoutKind,
	pub channel_id: Snowflake,
	at: (f32, f32),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadsShown {
	pub sections: Vec<ThreadSection>,
	pub avatars: Vec<(Snowflake, ImageName)>,
	pub can_create: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PinsShown {
	pub loaded: bool,
	pub entries: Vec<(PinEntry, Option<ImageName>)>,
	pub can_manage: bool,
}

pub struct ThreadsLayer {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	window: Signal<(f32, f32)>,
	popout: Signal<Option<Popout>>,
	threads: Signal<Rc<ThreadsShown>>,
	search: Signal<String>,
	pins: Signal<Rc<PinsShown>>,
	serials: Cell<u64>,
	images: RefCell<Vec<ImageName>>,
	requests: Hook<Request>,
	images_changed: Hook<()>,
	platform: RefCell<Option<Rc<PlatformService>>>,
	tasks: RefCell<Option<Tasks>>,
	picking: Cell<bool>,
}

impl ThreadsLayer {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		Rc::new(Self {
			runtime: runtime.clone(),
			pictures,
			window: runtime.signal((0., 0.)),
			popout: runtime.signal(None),
			threads: runtime.signal(Rc::default()),
			search: runtime.signal(String::new()),
			pins: runtime.signal(Rc::default()),
			serials: Cell::new(0),
			images: RefCell::default(),
			requests: RefCell::new(None),
			images_changed: RefCell::new(None),
			platform: RefCell::new(None),
			tasks: RefCell::new(None),
			picking: Cell::new(false),
		})
	}

	pub fn on_request(&self, hook: impl Fn(Request) + 'static) {
		*self.requests.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_images_changed(&self, hook: impl Fn(()) + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(hook));
	}

	fn request(&self, request: Request) {
		if let Some(hook) = self.requests.borrow().as_ref() {
			hook(request);
		}
	}

	fn serial(&self) -> u64 {
		let serial = self.serials.get() + 1;
		self.serials.set(serial);
		serial
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn popout(&self) -> Option<Popout> {
		self.popout.with_untracked(|popout| *popout)
	}

	pub fn open_threads(&self, channel_id: Snowflake, at: (f32, f32)) {
		self.search.set(String::new());
		self.threads.set(Rc::default());
		self.popout.set(Some(Popout {
			serial: self.serial(),
			kind: PopoutKind::Threads,
			channel_id,
			at,
		}));
	}

	pub fn open_pins(&self, channel_id: Snowflake, at: (f32, f32)) {
		self.pins.set(Rc::default());
		self.popout.set(Some(Popout {
			serial: self.serial(),
			kind: PopoutKind::Pins,
			channel_id,
			at,
		}));
	}

	pub fn close_popout(&self) {
		if self.popout.with_untracked(Option::is_some) {
			self.popout.set(None);
			self.set_images(Vec::new());
		}
	}

	pub fn show_threads(&self, shown: ThreadsShown) {
		if self.threads.with_untracked(|current| **current == shown) {
			return;
		}
		let images = shown.avatars.iter().map(|(_, name)| name.clone()).collect();
		self.threads.set(Rc::new(shown));
		self.set_images(images);
	}

	pub fn show_pins(&self, shown: PinsShown) {
		if self.pins.with_untracked(|current| **current == shown) {
			return;
		}
		let images = shown
			.entries
			.iter()
			.filter_map(|(_, avatar)| avatar.clone())
			.collect();
		self.pins.set(Rc::new(shown));
		self.set_images(images);
	}

	pub fn pin_avatar(&self, user: &protocol::User) -> Option<ImageName> {
		self.pictures.avatar(user, PIN_AVATAR)
	}

	pub fn row_avatar(&self, user: &protocol::User) -> Option<ImageName> {
		self.pictures.avatar(user, ROW_AVATAR)
	}

	fn set_images(&self, images: Vec<ImageName>) {
		if *self.images.borrow() == images {
			return;
		}
		*self.images.borrow_mut() = images;
		if let Some(hook) = self.images_changed.borrow().as_ref() {
			hook(());
		}
	}

	pub fn images(&self) -> Vec<ImageName> {
		self.images.borrow().clone()
	}

	pub fn settled(&self) -> bool {
		self.images
			.borrow()
			.iter()
			.all(|name| self.pictures.is_settled(name))
	}

	pub fn is_open(&self) -> bool {
		self.popout.with_untracked(Option::is_some)
	}

	pub fn escape(&self) -> bool {
		if self.popout.with_untracked(Option::is_some) {
			self.close_popout();
			return true;
		}
		false
	}

	pub fn pick_files(self: &Rc<Self>) {
		if self.picking.replace(true) {
			return;
		}
		let (Some(platform), Some(tasks)) = (self.platform.borrow().clone(), self.tasks.borrow().clone())
		else {
			self.picking.set(false);
			return;
		};
		let picker = FilePicker {
			title: "Upload a File".to_owned(),
			multiple: true,
			..FilePicker::default()
		};
		let pending = platform.0.open_files(picker);
		let layer = Rc::downgrade(self);
		tasks.spawn(async move {
			let picked = pending.await;
			let Some(layer) = layer.upgrade() else {
				return;
			};
			layer.picking.set(false);
			match picked {
				Ok(paths) if !paths.is_empty() => layer.request(Request::ReadFiles(paths)),
				Ok(_) => {}
				Err(error) => tracing::debug!(%error, "the file picker failed"),
			}
		});
	}

	fn place(at: (f32, f32), width: f32, window: (f32, f32)) -> f32 {
		(at.0 + ANCHOR_RIGHT - width)
			.min(window.0 - width - SNAP_MARGIN)
			.max(SNAP_MARGIN)
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (popout, window) = (self.popout.clone(), self.window.clone());
		let outside = self.clone();
		let this = self.clone();
		component(move |cx| {
			*this.tasks.borrow_mut() = Some(cx.tasks());
			if let Some(platform) = cx.try_service::<PlatformService>() {
				*this.platform.borrow_mut() = Some(platform);
			}
			div().absolute().left(0.).top(0.).w(0.).h(0.).child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.reactive_style({
						let (popout, window) = (popout.clone(), window.clone());
						move || {
							let (width, height) = if popout.with(Option::is_some) {
								window.get()
							} else {
								(0., 0.)
							};
							Styles::new().w(width).h(height)
						}
					})
					.on_event({
						let outside = outside.clone();
						move |event| {
							if event.phase == EventPhase::Target
								&& matches!(event.event, InputEvent::PointerDown { .. })
							{
								outside.close_popout();
								event.stop_propagation();
							}
						}
					})
					.child(this.popout_view()),
			)
		})
	}

	fn popout_view(self: &Rc<Self>) -> View {
		let popout = self.popout.clone();
		let this = self.clone();
		div().child_when(
			move || popout.get(),
			move |shown: Popout, _| {
				let window = this.window.clone();
				let at = shown.at;
				let (panel, width) = match shown.kind {
					PopoutKind::Threads => (popouts::threads(&this, shown.channel_id), "thread_popout_width"),
					PopoutKind::Pins => (popouts::pins(&this, shown.channel_id), "pins_popout_width"),
				};
				let side = parts::size(&this.runtime, width);
				let top = parts::size(&this.runtime, "header_popout_top");
				panel
					.absolute()
					.w(side)
					.top(top)
					.reactive_style(move || Styles::new().left(ThreadsLayer::place(at, side, window.get())))
					.on_event(|event| {
						if event.phase != EventPhase::Capture
							&& matches!(event.event, InputEvent::PointerDown { .. })
						{
							event.stop_propagation();
						}
					})
			},
		)
	}
}
