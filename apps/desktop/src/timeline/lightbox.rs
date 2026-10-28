use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{LightboxImage, ToastKind, Viewing, Zoom};
use media::ImageName;
use protocol::Snowflake;
use zgx::compose::Tasks;
use zgx::compose::prelude::*;
use zgx::input::{EventContext, EventPhase, InputEvent, Key, PointerButton};
use zgx::platform::FilePicker;
use zgx::reactive::{Runtime, Signal};

use super::icons::icon;
use super::links::Links;
use super::pictures::Pictures;
use super::rows::tok;
use crate::kit::{self, menu_panel, menu_row};
use crate::toasts;

const TOOLBAR: f32 = 64.;
const SIDE_ROOM: f32 = 56.;
const TOP_ROOM: f32 = 24.;
const BOTTOM_ROOM: f32 = 56.;
const BUTTON: f32 = 40.;
const BUTTON_ICON: f32 = 20.;
const AVATAR: f32 = 40.;
const ARROW: f32 = 40.;
const ARROW_INSET: f32 = 16.;

type Forward = RefCell<Option<Box<dyn Fn((Snowflake, Snowflake))>>>;

#[derive(Clone, PartialEq)]
struct Shown {
	viewing: Viewing,
	name: Option<ImageName>,
	avatar: Option<ImageName>,
	size: (f32, f32),
}

pub struct Lightbox {
	pictures: Rc<Pictures>,
	links: Rc<Links>,
	shown: Signal<Option<Rc<Shown>>>,
	zoom: Signal<Zoom>,
	window: Signal<(f32, f32)>,
	more_open: Signal<bool>,
	dragging: Cell<Option<(f32, f32)>>,
	tasks: RefCell<Option<Tasks>>,
	changed: RefCell<Option<Box<dyn Fn()>>>,
	forward: Forward,
}

impl Lightbox {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>, links: Rc<Links>) -> Self {
		Self {
			pictures,
			links,
			shown: runtime.signal(None),
			zoom: runtime.signal(Zoom::FIT),
			window: runtime.signal((0., 0.)),
			more_open: runtime.signal(false),
			dragging: Cell::new(None),
			tasks: RefCell::new(None),
			changed: RefCell::new(None),
			forward: RefCell::new(None),
		}
	}

	pub fn on_changed(&self, changed: impl Fn() + 'static) {
		*self.changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn on_forward(&self, forward: impl Fn((Snowflake, Snowflake)) + 'static) {
		*self.forward.borrow_mut() = Some(Box::new(forward));
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn open(&self, image: LightboxImage) {
		self.show(Viewing::alone(image));
	}

	pub fn show(&self, viewing: Viewing) {
		let window = self.window.with_untracked(|window| *window);
		let image = viewing.current();
		let name = self.pictures.lightbox(image, window);
		let size = self.fitted(bridge::lightbox_box(image, window));
		let avatar = viewing.author.as_ref().map(|author| {
			self.pictures
				.member_avatar(&author.avatar, f64::from(AVATAR), false)
		});
		self.dragging.set(None);
		self.more_open.set(false);
		self.zoom.set(Zoom::FIT);
		self.shown.set(Some(Rc::new(Shown {
			viewing,
			name,
			avatar,
			size,
		})));
		self.notify();
	}

	pub fn close(&self) -> bool {
		let open = self.is_open();
		if open {
			self.more_open.set(false);
			self.shown.set(None);
			self.notify();
		}
		open
	}

	pub fn is_open(&self) -> bool {
		self.shown.with_untracked(Option::is_some)
	}

	pub fn viewing(&self) -> Option<Viewing> {
		self.shown
			.with_untracked(|shown| shown.as_ref().map(|shown| shown.viewing.clone()))
	}

	pub fn zoom(&self) -> Zoom {
		self.zoom.with_untracked(|zoom| *zoom)
	}

	pub fn step(&self, by: isize) -> bool {
		match self.viewing().and_then(|viewing| viewing.stepped(by)) {
			Some(viewing) => {
				self.show(viewing);
				true
			}
			None => false,
		}
	}

	pub fn key(&self, key: &Key) -> bool {
		if !self.is_open() {
			return false;
		}
		match key {
			Key::ArrowLeft => self.step(-1),
			Key::ArrowRight => self.step(1),
			_ => false,
		}
	}

	pub fn toggle_zoom(&self) {
		let (fitted, room) = self.extent();
		self.zoom.set(self.zoom().toggled(fitted, room));
	}

	pub fn reset_zoom(&self) {
		self.zoom.set(Zoom::FIT);
	}

	pub fn wheel(&self, delta_y: f32, at: (f32, f32)) {
		let (fitted, room) = self.extent();
		let centre = self.centre();
		let about = (at.0 - centre.0, at.1 - centre.1);
		self.zoom.set(self.zoom().wheeled(delta_y, about, fitted, room));
	}

	fn pan(&self, by: (f32, f32)) {
		let (fitted, room) = self.extent();
		self.zoom.set(self.zoom().panned(by, fitted, room));
	}

	pub fn images(&self) -> Vec<ImageName> {
		self.shown
			.with_untracked(|shown| {
				shown
					.as_ref()
					.map(|shown| shown.name.iter().chain(&shown.avatar).cloned().collect())
			})
			.unwrap_or_default()
	}

	pub fn settled(&self) -> bool {
		self.is_open() && self.images().iter().all(|name| self.pictures.is_settled(name))
	}

	fn notify(&self) {
		if let Some(changed) = self.changed.borrow().as_ref() {
			changed();
		}
	}

	fn room(&self) -> (f32, f32) {
		let (width, height) = self.window.with_untracked(|window| *window);
		(
			(width - 2. * SIDE_ROOM).max(1.),
			(height - TOOLBAR - TOP_ROOM - BOTTOM_ROOM).max(1.),
		)
	}

	fn fitted(&self, (width, height): (f32, f32)) -> (f32, f32) {
		let room = self.room();
		let fit = (room.0 / width).min(room.1 / height).min(1.);
		(width * fit, height * fit)
	}

	fn centre(&self) -> (f32, f32) {
		let width = self.window.with_untracked(|window| window.0);
		(width / 2., TOOLBAR + TOP_ROOM + self.room().1 / 2.)
	}

	fn extent(&self) -> ((f32, f32), (f32, f32)) {
		let fitted = self
			.shown
			.with_untracked(|shown| shown.as_ref().map(|shown| shown.size))
			.unwrap_or((1., 1.));
		(fitted, self.window.with_untracked(|window| *window))
	}

	fn current(&self) -> Option<LightboxImage> {
		self.viewing().map(|viewing| viewing.current().clone())
	}

	pub fn forward(&self) {
		let Some((channel_id, message_id)) = self.viewing().and_then(|viewing| viewing.message) else {
			return;
		};
		self.close();
		if let Some(forward) = self.forward.borrow().as_ref() {
			forward((channel_id, message_id));
		}
	}

	pub fn open_in_browser(&self) {
		if let Some(url) = self.current().as_ref().and_then(LightboxImage::original_url) {
			self.links.open(url);
		}
	}

	pub fn copy_link(&self) {
		if let Some(url) = self.current().as_ref().and_then(LightboxImage::original_url) {
			toasts::copy(url, "Link copied");
		}
	}

	pub fn copy_image(&self) {
		let pixels = self
			.shown
			.with_untracked(|shown| shown.as_ref().and_then(|shown| shown.name.clone()))
			.and_then(|name| self.pictures.pixels(&name));
		let Some(pixels) = pixels else {
			toasts::show(ToastKind::Warning, "The image is still loading");
			return;
		};
		let image = platform::clipboard::RgbaImage {
			width: pixels.width as usize,
			height: pixels.height as usize,
			bytes: pixels.rgba.to_vec(),
		};
		match platform::clipboard::write_image(&image) {
			Ok(()) => toasts::show(ToastKind::Success, "Image copied"),
			Err(error) => {
				tracing::warn!(%error, "the clipboard refused the image");
				toasts::show(ToastKind::Error, "Could not copy to the clipboard.");
			}
		}
	}

	pub fn save(&self) {
		let Some(image) = self.current() else {
			return;
		};
		let Some(url) = image.original_url().map(str::to_owned) else {
			return;
		};
		let (Some(platform), Some(tasks)) = (self.links.platform(), self.tasks.borrow().clone()) else {
			tracing::debug!("no platform to ask where to save");
			return;
		};
		let picker = FilePicker {
			title: "Save Image".to_owned(),
			file_name: Some(bridge::viewer_file_name(&image)),
			..FilePicker::default()
		};
		let chosen = platform.0.save_file(picker);
		let pictures = self.pictures.clone();
		tasks.spawn(async move {
			let path = match chosen.await {
				Ok(Some(path)) => path,
				Ok(None) => return,
				Err(error) => {
					tracing::debug!(%error, "the save dialog failed");
					return;
				}
			};
			match pictures.save_original(&url, path).await {
				Ok(Ok(())) => toasts::show(ToastKind::Success, "Image saved"),
				Ok(Err(error)) => {
					tracing::warn!(%error, "the image was not saved");
					toasts::show(ToastKind::Error, "Could not save the image.");
				}
				Err(_) => tracing::debug!("the save was dropped"),
			}
		});
	}

	pub fn view(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (shown, sized, window) = (self.shown.clone(), self.shown.clone(), self.window.clone());
		component(move |cx| {
			*this.tasks.borrow_mut() = Some(cx.tasks());
			let this = this.clone();
			let (shown, sized, window) = (shown.clone(), sized.clone(), window.clone());
			div()
				.reactive_style(move || {
					if sized.with(Option::is_some) {
						let (width, height) = window.get();
						Styles::new().absolute().left(0.).top(0.).w(width).h(height)
					} else {
						Styles::new().absolute().left(0.).top(0.).w(0.).h(0.)
					}
				})
				.child(switch(
					move || shown.get(),
					move |shown: Option<Rc<Shown>>, _| match shown {
						Some(shown) => this.overlay(&shown),
						None => div(),
					},
				))
		})
	}

	fn overlay(self: &Rc<Self>, shown: &Shown) -> View {
		let (closer, wheeler) = (self.clone(), self.clone());
		let window = self.window.clone();
		let several = shown.viewing.images.len() > 1;
		div()
			.id("viewer")
			.relative()
			.reactive_style(move || {
				let (width, height) = window.get();
				Styles::new().w(width).h(height)
			})
			.overflow_hidden()
			.bg(tok("background_scrim_lightbox"))
			.on_click(move || {
				closer.close();
			})
			.on_event(move |event| {
				if event.phase == EventPhase::Capture {
					return;
				}
				if let InputEvent::Scroll { x, y, delta_y, .. } = event.event {
					wheeler.wheel(delta_y, (x, y));
					event.prevent_default();
					event.stop_propagation();
				}
			})
			.child(self.picture(shown))
			.child(self.header(shown))
			.when(several, |overlay| {
				overlay
					.child(self.arrow("viewer-previous", "chevron_left", "Previous", -1))
					.child(self.arrow("viewer-next", "chevron_right", "Next", 1))
			})
	}

	fn header(self: &Rc<Self>, shown: &Shown) -> View {
		row()
			.absolute()
			.left(0.)
			.top(0.)
			.w_full()
			.h(TOOLBAR)
			.items_center()
			.gap(tok("space_8"))
			.pt(tok("space_12"))
			.px(tok("space_16"))
			.child(self.author(shown))
			.child(self.toolbar(shown))
			.child(group(vec![button("viewer-close", "close", "Close", {
				let closer = self.clone();
				move || {
					closer.close();
				}
			})]))
	}

	fn author(&self, shown: &Shown) -> View {
		let Some(author) = shown.viewing.author.as_ref() else {
			return div().grow().min_w(0.);
		};
		let time = bridge::message_time(author.sent_at, bridge::today());
		row()
			.grow()
			.min_w(0.)
			.items_center()
			.gap(tok("space_12"))
			.child(
				div()
					.id("viewer-author")
					.on_event(swallow)
					.child(match &shown.avatar {
						Some(name) => image_box(&self.pictures, name, AVATAR, AVATAR / 2.),
						None => div().size(AVATAR, AVATAR),
					}),
			)
			.child(
				column()
					.min_w(0.)
					.on_event(swallow)
					.child(
						text(author.name.clone())
							.truncate()
							.text_size(tok("font_size_md"))
							.line_height(tok("line_height_label"))
							.font_weight(600)
							.text_color(tok("text_1")),
					)
					.child(
						text(time)
							.text_size(tok("font_size_xs"))
							.line_height(tok("line_height_xs"))
							.text_color(tok("text_3")),
					),
			)
	}

	fn toolbar(self: &Rc<Self>, shown: &Shown) -> View {
		let image = shown.viewing.current();
		let linked = image.original_url().is_some();
		let zoom = self.zoom.clone();
		let zoomer = self.clone();
		let zooming = div()
			.id("viewer-zoom")
			.size(BUTTON, BUTTON)
			.shrink_0()
			.items_center()
			.justify_center()
			.rounded(tok("radius_sm"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("hover")))
			.on_event(swallow)
			.on_click(move || zoomer.toggle_zoom())
			.child(switch(
				move || zoom.with(Zoom::zoomed),
				|zoomed: bool, _| icon(if zoomed { "zoom_out" } else { "zoom_in" }, BUTTON_ICON, "text_3"),
			))
			.tooltip("Zoom".to_owned());
		let mut buttons = vec![zooming];
		if shown.viewing.message.is_some() {
			let forwarder = self.clone();
			buttons.push(button("viewer-forward", "forward", "Forward", move || {
				forwarder.forward()
			}));
		}
		if linked {
			let (saver, opener) = (self.clone(), self.clone());
			buttons.push(button("viewer-save", "download", "Download", move || {
				saver.save()
			}));
			buttons.push(button(
				"viewer-open",
				"external_link",
				"Open in Browser",
				move || opener.open_in_browser(),
			));
			buttons.push(self.more());
		}
		group(buttons)
	}

	fn more(self: &Rc<Self>) -> View {
		let toggle = self.more_open.clone();
		let anchor = button("viewer-more", "more", "More", move || {
			toggle.update(|open| *open = !*open);
		});
		let (copier, linker) = (self.clone(), self.clone());
		menu_panel(menu("Image menu", self.more_open.clone(), anchor))
			.child(
				menu_row("Copy Image".to_owned(), kit::menu_icon("image", false), "text_3")
					.id("viewer-copy-image")
					.on_click(move || copier.copy_image()),
			)
			.child(
				menu_row("Copy Link".to_owned(), kit::menu_icon("link", false), "text_3")
					.id("viewer-copy-link")
					.on_click(move || linker.copy_link()),
			)
	}

	fn arrow(self: &Rc<Self>, id: &'static str, name: &'static str, tip: &str, by: isize) -> View {
		let stepper = self.clone();
		let window = self.window.clone();
		let centre = self.centre().1;
		div()
			.id(id)
			.absolute()
			.reactive_style(move || {
				let width = window.with(|window| window.0);
				let left = if by < 0 {
					ARROW_INSET
				} else {
					width - ARROW_INSET - ARROW
				};
				Styles::new().left(left).top(centre - ARROW / 2.)
			})
			.size(ARROW, ARROW)
			.items_center()
			.justify_center()
			.rounded(ARROW / 2.)
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.cursor(Cursor::Pointer)
			.hover(|style| style.bg(tok("bg_2")))
			.on_event(swallow)
			.on_click(move || {
				stepper.step(by);
			})
			.child(icon(name, BUTTON_ICON, "text_2"))
			.tooltip(tip.to_owned())
	}

	fn picture(self: &Rc<Self>, shown: &Shown) -> View {
		let fitted = shown.size;
		let centre = self.centre();
		let zoom = self.zoom.clone();
		let grabber = self.clone();
		let picture = shown.name.as_ref().map(|name| self.pictures.picture(name));
		let frame = div()
			.id("viewer-image")
			.absolute()
			.reactive_style(move || {
				let zoom = zoom.get();
				let (width, height) = zoom.size(fitted);
				Styles::new()
					.left(centre.0 + zoom.pan.0 - width / 2.)
					.top(centre.1 + zoom.pan.1 - height / 2.)
					.w(width)
					.h(height)
			})
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.on_event(move |event| grabber.grab(event));
		let canvas = div().w_full().h_full().items_center().justify_center();
		let canvas = match picture {
			Some(picture) => {
				let loading = picture.clone();
				canvas
					.reactive_style(move || match picture.get() {
						Some(image) => Styles::new().bg_image(image, ObjectFit::Contain),
						None => Styles::new(),
					})
					.child(div().child(switch(
						move || loading.with(Option::is_none),
						|waiting: bool, _| {
							if waiting {
								text("Loading image...").text_color(tok("text_4"))
							} else {
								div()
							}
						},
					)))
			}
			None => canvas.child(text("Loading image...").text_color(tok("text_4"))),
		};
		frame.child(canvas)
	}

	fn grab(&self, event: &mut EventContext) {
		if event.phase == EventPhase::Capture {
			return;
		}
		match event.event {
			InputEvent::Activate => event.stop_propagation(),
			InputEvent::PointerDown {
				x,
				y,
				button: PointerButton::Primary,
			} => {
				if event.click_count() >= 2 {
					self.reset_zoom();
				} else if self.zoom().zoomed() {
					self.dragging.set(Some((x, y)));
					event.capture_pointer();
				}
				event.stop_propagation();
			}
			InputEvent::PointerMove { x, y } if let Some(from) = self.dragging.get() => {
				self.dragging.set(Some((x, y)));
				self.pan((x - from.0, y - from.1));
			}
			InputEvent::PointerUp { .. } | InputEvent::PointerCancel if self.dragging.take().is_some() => {
				event.release_pointer();
			}
			_ => {}
		}
	}
}

fn swallow(event: &mut EventContext) {
	if event.phase == EventPhase::Capture {
		return;
	}
	if matches!(event.event, InputEvent::Activate) {
		event.stop_propagation();
	}
}

fn image_box(pictures: &Pictures, name: &ImageName, side: f32, radius: f32) -> View {
	let picture = pictures.picture(name);
	div()
		.size(side, side)
		.shrink_0()
		.rounded(radius)
		.overflow_hidden()
		.bg(tok("bg_3"))
		.reactive_style(move || match picture.get() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
			None => Styles::new(),
		})
}

fn group(buttons: Vec<View>) -> View {
	row()
		.shrink_0()
		.items_center()
		.p(tok("space_4"))
		.gap(tok("space_2"))
		.rounded(tok("radius_md"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.on_event(swallow)
		.children(buttons)
}

fn button(id: &'static str, name: &'static str, tip: &str, action: impl Fn() + 'static) -> View {
	div()
		.id(id)
		.size(BUTTON, BUTTON)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
		.on_event(swallow)
		.on_click(action)
		.child(icon(name, BUTTON_ICON, "text_3"))
		.tooltip(tip.to_owned())
}
