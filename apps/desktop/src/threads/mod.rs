mod dialogs;
mod popouts;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use bridge::{NewPostForm, PinEntry, ThreadEntry, ThreadTab};
use media::ImageName;
use protocol::Snowflake;
use rest::UploadFile;
use session::Command;
use zgx::compose::Tasks;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::platform::{FilePicker, PlatformService};
use zgx::reactive::{Runtime, Signal};

use crate::timeline::Pictures;

const POPOVER_GAP: f32 = 12.;
const SNAP_MARGIN: f32 = 8.;
const POPOVER_WIDTH: f32 = 400.;
const POPOVER_HEIGHT: f32 = 440.;
const PIN_AVATAR: f64 = 32.;
const MAX_FILES: usize = 10;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug)]
pub enum Request {
	Command(Command),
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
	Archived(Snowflake),
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
	pub entries: Vec<ThreadEntry>,
	pub can_create: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PinsShown {
	pub loaded: bool,
	pub entries: Vec<(PinEntry, Option<ImageName>)>,
	pub can_manage: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateShown {
	serial: u64,
	pub channel_id: Snowflake,
	pub message_id: Option<Snowflake>,
	pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostShown {
	serial: u64,
	pub channel_id: Snowflake,
	pub form: Rc<NewPostForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChip {
	pub name: String,
	pub size: String,
}

pub struct ThreadsLayer {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	window: Signal<(f32, f32)>,
	popout: Signal<Option<Popout>>,
	threads: Signal<Rc<ThreadsShown>>,
	tab: Signal<ThreadTab>,
	pins: Signal<Rc<PinsShown>>,
	create: Signal<Option<CreateShown>>,
	create_open: Signal<bool>,
	name: Signal<String>,
	archive: Signal<usize>,
	post: Signal<Option<PostShown>>,
	post_open: Signal<bool>,
	title: Signal<String>,
	body: Signal<String>,
	chosen: Signal<BTreeSet<Snowflake>>,
	files: Signal<Rc<Vec<FileChip>>>,
	uploads: RefCell<Vec<UploadFile>>,
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
			tab: runtime.signal(ThreadTab::Active),
			pins: runtime.signal(Rc::default()),
			create: runtime.signal(None),
			create_open: runtime.signal(false),
			name: runtime.signal(String::new()),
			archive: runtime.signal(bridge::DEFAULT_ARCHIVE_CHOICE),
			post: runtime.signal(None),
			post_open: runtime.signal(false),
			title: runtime.signal(String::new()),
			body: runtime.signal(String::new()),
			chosen: runtime.signal(BTreeSet::new()),
			files: runtime.signal(Rc::default()),
			uploads: RefCell::default(),
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

	pub fn tab(&self) -> ThreadTab {
		self.tab.with_untracked(|tab| *tab)
	}

	pub fn open_threads(&self, channel_id: Snowflake, at: (f32, f32)) {
		self.tab.set(ThreadTab::Active);
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
		if self.threads.with_untracked(|current| **current != shown) {
			self.threads.set(Rc::new(shown));
		}
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

	pub fn open_create(&self, channel_id: Snowflake, message_id: Option<Snowflake>, preview: Option<String>) {
		self.close_popout();
		self.name.set(preview.clone().unwrap_or_default());
		self.archive.set(bridge::DEFAULT_ARCHIVE_CHOICE);
		self.create.set(Some(CreateShown {
			serial: self.serial(),
			channel_id,
			message_id,
			preview,
		}));
		self.create_open.set(true);
	}

	pub fn open_post(&self, channel_id: Snowflake, form: NewPostForm) {
		self.close_popout();
		self.title.set(String::new());
		self.body.set(String::new());
		self.chosen.set(BTreeSet::new());
		self.uploads.borrow_mut().clear();
		self.files.set(Rc::default());
		self.post.set(Some(PostShown {
			serial: self.serial(),
			channel_id,
			form: Rc::new(form),
		}));
		self.post_open.set(true);
	}

	pub fn has_dialog(&self) -> bool {
		self.create_open.with_untracked(|open| *open) || self.post_open.with_untracked(|open| *open)
	}

	pub fn is_open(&self) -> bool {
		self.popout.with_untracked(Option::is_some) || self.has_dialog()
	}

	pub fn close_dialogs(&self) {
		self.create_open.set(false);
		self.post_open.set(false);
	}

	pub fn escape(&self) -> bool {
		if self.has_dialog() {
			self.close_dialogs();
			return true;
		}
		if self.popout.with_untracked(Option::is_some) {
			self.close_popout();
			return true;
		}
		false
	}

	pub fn add_upload(&self, file: UploadFile) {
		let mut uploads = self.uploads.borrow_mut();
		if uploads.len() >= MAX_FILES {
			return;
		}
		uploads.push(file);
		let chips = uploads
			.iter()
			.map(|file| FileChip {
				name: file.name.clone(),
				size: bridge::file_size(file.bytes.len() as u64),
			})
			.collect();
		drop(uploads);
		self.files.set(Rc::new(chips));
	}

	fn remove_upload(&self, index: usize) {
		let mut uploads = self.uploads.borrow_mut();
		if index < uploads.len() {
			uploads.remove(index);
		}
		drop(uploads);
		self.files.update(|files| {
			let mut list = (**files).clone();
			if index < list.len() {
				list.remove(index);
			}
			*files = Rc::new(list);
		});
	}

	fn pick_files(self: &Rc<Self>) {
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

	fn submit_create(&self) {
		let Some(shown) = self.create.with_untracked(Clone::clone) else {
			return;
		};
		let name = self.name.with_untracked(|name| bridge::trimmed_name(name));
		if name.is_empty() {
			return;
		}
		let (minutes, _) = bridge::ARCHIVE_CHOICES[self.archive.with_untracked(|choice| *choice)];
		self.request(Request::Command(Command::CreateThread {
			channel_id: shown.channel_id,
			message_id: shown.message_id,
			name,
			auto_archive_minutes: minutes,
		}));
		self.create_open.set(false);
	}

	fn can_post(&self) -> bool {
		let titled = self.title.with(|title| !bridge::trimmed_name(title).is_empty());
		let tagged = self.post.with(|post| {
			post.as_ref()
				.is_none_or(|post| !post.form.require_tag || self.chosen.with(|chosen| !chosen.is_empty()))
		});
		titled && tagged
	}

	fn submit_post(&self) {
		let Some(shown) = self.post.with_untracked(Clone::clone) else {
			return;
		};
		if !self.runtime.untracked(|| self.can_post()) {
			return;
		}
		let name = self.title.with_untracked(|title| bridge::trimmed_name(title));
		let content = self.body.with_untracked(|body| body.trim().to_owned());
		let tags = self
			.chosen
			.with_untracked(|chosen| chosen.iter().copied().collect());
		let files = std::mem::take(&mut *self.uploads.borrow_mut());
		self.request(Request::Command(Command::CreateForumPost {
			channel_id: shown.channel_id,
			name,
			content,
			tags,
			files,
		}));
		self.post_open.set(false);
	}

	fn place(at: (f32, f32), window: (f32, f32)) -> (f32, f32) {
		let below = at.1 <= window.1 / 2.;
		let x = at.0 - POPOVER_WIDTH;
		let y = if below {
			at.1 + POPOVER_GAP
		} else {
			at.1 - POPOVER_GAP - POPOVER_HEIGHT
		};
		let clamp =
			|value: f32, extent: f32, room: f32| value.min(room - extent - SNAP_MARGIN).max(SNAP_MARGIN);
		(
			clamp(x, POPOVER_WIDTH, window.0),
			clamp(y, POPOVER_HEIGHT, window.1),
		)
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
			div()
				.absolute()
				.left(0.)
				.top(0.)
				.w(0.)
				.h(0.)
				.child(
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
				.child(dialogs::create_dialog(&this))
				.child(dialogs::post_dialog(&this))
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
				let panel = match shown.kind {
					PopoutKind::Threads => popouts::threads(&this, shown.channel_id),
					PopoutKind::Pins => popouts::pins(&this, shown.channel_id),
				};
				panel
					.absolute()
					.w(POPOVER_WIDTH)
					.h(POPOVER_HEIGHT)
					.reactive_style(move || {
						let (x, y) = ThreadsLayer::place(at, window.get());
						Styles::new().left(x).top(y)
					})
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
