use std::cell::RefCell;
use std::rc::Rc;

use bridge::LightboxImage;
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::{Runtime, Signal};

use super::icons::icon;
use super::links::Links;
use super::pictures::Pictures;
use super::rows::tok;

const TOOLBAR: f32 = 64.;
const SIDE_ROOM: f32 = 56.;
const TOP_ROOM: f32 = 24.;
const BOTTOM_ROOM: f32 = 56.;
const BUTTON: f32 = 38.;
const BUTTON_ICON: f32 = 20.;

#[derive(Clone, PartialEq)]
struct Shown {
	image: LightboxImage,
	name: Option<ImageName>,
	size: (f32, f32),
}

pub struct Lightbox {
	pictures: Rc<Pictures>,
	links: Rc<Links>,
	shown: Signal<Option<Rc<Shown>>>,
	window: Signal<(f32, f32)>,
	changed: RefCell<Option<Box<dyn Fn()>>>,
}

impl Lightbox {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>, links: Rc<Links>) -> Self {
		Self {
			pictures,
			links,
			shown: runtime.signal(None),
			window: runtime.signal((0., 0.)),
			changed: RefCell::new(None),
		}
	}

	pub fn on_changed(&self, changed: impl Fn() + 'static) {
		*self.changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn open(&self, image: LightboxImage) {
		let window = self.window.with_untracked(|window| *window);
		let name = self.pictures.lightbox(&image, window);
		let size = bridge::lightbox_box(&image, window);
		self.shown.set(Some(Rc::new(Shown { image, name, size })));
		self.notify();
	}

	pub fn close(&self) -> bool {
		let open = self.is_open();
		if open {
			self.shown.set(None);
			self.notify();
		}
		open
	}

	pub fn is_open(&self) -> bool {
		self.shown.with_untracked(Option::is_some)
	}

	pub fn images(&self) -> Vec<ImageName> {
		self.shown
			.with_untracked(|shown| shown.as_ref().and_then(|shown| shown.name.clone()))
			.into_iter()
			.collect()
	}

	pub fn settled(&self) -> bool {
		self.is_open() && self.images().iter().all(|name| self.pictures.is_settled(name))
	}

	fn notify(&self) {
		if let Some(changed) = self.changed.borrow().as_ref() {
			changed();
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (shown, sized, window) = (self.shown.clone(), self.shown.clone(), self.window.clone());
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
	}

	fn overlay(self: &Rc<Self>, shown: &Shown) -> View {
		let closer = self.clone();
		let window = self.window.clone();
		column()
			.relative()
			.reactive_style(move || {
				let (width, height) = window.get();
				Styles::new().w(width).h(height)
			})
			.bg(tok("background_scrim_lightbox"))
			.on_click(move || {
				closer.close();
			})
			.child(self.toolbar(shown))
			.child(
				column()
					.w_full()
					.grow()
					.min_h(0.)
					.items_center()
					.justify_center()
					.pt(TOP_ROOM)
					.pb(BOTTOM_ROOM)
					.px(SIDE_ROOM)
					.child(self.picture(shown)),
			)
	}

	fn toolbar(self: &Rc<Self>, shown: &Shown) -> View {
		let (links, url) = (self.links.clone(), shown.image.original_url().map(str::to_owned));
		let closer = self.clone();
		let (width, height) = (shown.image.width, shown.image.height);
		row()
			.w_full()
			.h(TOOLBAR)
			.shrink_0()
			.items_center()
			.gap(tok("space_8"))
			.pt(tok("space_12"))
			.px(tok("space_16"))
			.child(
				column()
					.grow()
					.min_w(0.)
					.on_event(swallow)
					.child(
						text(shown.image.name.clone())
							.w_full()
							.truncate()
							.text_size(tok("font_size_md"))
							.line_height(tok("line_height_label"))
							.font_weight(600)
							.text_color(tok("text_1")),
					)
					.when(width > 0 && height > 0, |column| {
						column.child(
							text(format!("{width} \u{d7} {height}"))
								.text_size(tok("font_size_xs"))
								.line_height(tok("line_height_xs"))
								.text_color(tok("text_3")),
						)
					}),
			)
			.when_some(url, |buttons, url| {
				buttons.child(button("external_link", "Open original", move || {
					links.download(&url)
				}))
			})
			.child(button("close", "Close", move || {
				closer.close();
			}))
	}

	fn picture(&self, shown: &Shown) -> View {
		let (window_width, window_height) = self.window.with_untracked(|window| *window);
		let room = (
			(window_width - 2. * SIDE_ROOM).max(1.),
			(window_height - TOOLBAR - TOP_ROOM - BOTTOM_ROOM).max(1.),
		);
		let (width, height) = shown.size;
		let fit = (room.0 / width).min(room.1 / height).min(1.);
		let (width, height) = (width * fit, height * fit);
		let picture = shown.name.as_ref().map(|name| self.pictures.picture(name));
		let frame = div()
			.size(width, height)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.items_center()
			.justify_center()
			.on_event(swallow);
		match picture {
			Some(picture) => {
				let loading = picture.clone();
				frame
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
			None => frame.child(text("Loading image...").text_color(tok("text_4"))),
		}
	}
}

fn swallow(event: &mut zgx::input::EventContext) {
	if event.phase == EventPhase::Capture {
		return;
	}
	if matches!(event.event, InputEvent::Activate) {
		event.stop_propagation();
	}
}

fn button(name: &'static str, tip: &str, action: impl Fn() + 'static) -> View {
	div()
		.size(BUTTON, BUTTON)
		.shrink_0()
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(tok("overlay_button_background"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("active_2")))
		.on_event(swallow)
		.on_click(action)
		.child(icon(name, BUTTON_ICON, "text_1"))
		.tooltip(tip.to_owned())
}
