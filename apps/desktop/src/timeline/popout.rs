use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{
	Area, ExpressionDetails, PopoutServer, PopoutSubject, STICKER_SIDE, members_label, place_popout,
};
use media::ImageName;
use protocol::StickerItem;
use state::Expression;
use zgx::compose::NodeHandle;
use zgx::compose::prelude::*;
use zgx::reactive::{Effect, Runtime, Signal};
use zgx::scene::Rect;

use super::attachments::picture;
use super::pictures::{JUMBO_EMOJI, Pictures};
use super::rows::tok;
use crate::emoji;

const WIDTH: f32 = 300.;
const GAP: f32 = 8.;
const MARGIN: f32 = 8.;
const PADDING: f32 = 16.;
const SERVER_ICON: f32 = 40.;
const TITLE_LINE: f32 = 20.;
const TEXT_LINE: f32 = 18.;
const SMALL_LINE: f32 = 16.;
const ANCHORS: usize = 256;

type Source = RefCell<Option<Box<dyn Fn(&PopoutSubject) -> Option<ExpressionDetails>>>>;

#[derive(Clone, PartialEq)]
struct Shown {
	serial: u64,
	subject: PopoutSubject,
	details: ExpressionDetails,
	item: Area,
}

pub struct ExpressionPopout {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	shown: Signal<Option<Shown>>,
	height: Signal<f32>,
	bounds: Signal<(f32, f32)>,
	root: RefCell<Option<NodeHandle>>,
	source: Source,
	images: RefCell<Vec<ImageName>>,
	measure: RefCell<Option<Effect>>,
	serial: Cell<u64>,
	changed: RefCell<Option<Box<dyn Fn()>>>,
	anchors: RefCell<Vec<(PopoutSubject, NodeHandle)>>,
	placed: Cell<Area>,
}

fn bounds_of(handle: &NodeHandle) -> Option<Rect> {
	let scene = handle.scene()?;
	let scene = scene.borrow();
	scene.contains(handle.node()).then(|| scene.bounds(handle.node()))
}

pub fn opens(view: View, subject: PopoutSubject, popout: &Rc<ExpressionPopout>) -> View {
	let handle: Rc<RefCell<Option<NodeHandle>>> = Rc::default();
	let (kept, popout, remembered, known) = (handle.clone(), popout.clone(), popout.clone(), subject.clone());
	view.cursor(Cursor::Pointer)
		.on_mount(move |node| {
			remembered.remember(known.clone(), node.clone());
			*kept.borrow_mut() = Some(node);
		})
		.on_click(move || {
			let rect = handle.borrow().as_ref().and_then(bounds_of);
			if let Some(rect) = rect {
				popout.open(subject.clone(), rect);
			}
		})
}

impl ExpressionPopout {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Self {
		Self {
			runtime: runtime.clone(),
			pictures,
			shown: runtime.signal(None),
			height: runtime.signal(0.),
			bounds: runtime.signal((0., 0.)),
			root: RefCell::new(None),
			source: RefCell::new(None),
			images: RefCell::default(),
			measure: RefCell::new(None),
			serial: Cell::new(0),
			changed: RefCell::new(None),
			anchors: RefCell::default(),
			placed: Cell::new(Area {
				x: 0.,
				y: 0.,
				width: 0.,
				height: 0.,
			}),
		}
	}

	pub fn on_details(&self, source: impl Fn(&PopoutSubject) -> Option<ExpressionDetails> + 'static) {
		*self.source.borrow_mut() = Some(Box::new(source));
	}

	pub fn on_images_changed(&self, changed: impl Fn() + 'static) {
		*self.changed.borrow_mut() = Some(Box::new(changed));
	}

	fn remember(&self, subject: PopoutSubject, node: NodeHandle) {
		let mut anchors = self.anchors.borrow_mut();
		anchors.retain(|(_, handle)| bounds_of(handle).is_some());
		if anchors.len() >= ANCHORS {
			anchors.remove(0);
		}
		anchors.push((subject, node));
	}

	pub fn anchor(&self, wanted: impl Fn(&PopoutSubject) -> bool) -> Option<(PopoutSubject, Rect)> {
		self.anchors
			.borrow()
			.iter()
			.rev()
			.filter(|(subject, _)| wanted(subject))
			.find_map(|(subject, handle)| Some((subject.clone(), bounds_of(handle)?)))
	}

	pub fn set_root(&self, root: NodeHandle) {
		*self.root.borrow_mut() = Some(root);
	}

	pub fn set_bounds(&self, width: f32, height: f32) {
		self.bounds.set((width, height));
	}

	pub fn images(&self) -> Vec<ImageName> {
		if self.is_open() {
			self.images.borrow().clone()
		} else {
			Vec::new()
		}
	}

	pub fn is_open(&self) -> bool {
		self.shown.with_untracked(Option::is_some)
	}

	pub fn close(&self) -> bool {
		let open = self.is_open();
		if open {
			self.shown.set(None);
			self.set_images(Vec::new());
		}
		open
	}

	pub fn pressed(&self, x: f32, y: f32) {
		if !self.is_open() {
			return;
		}
		let origin = self.root.borrow().as_ref().and_then(bounds_of);
		let (left, top) = origin.map_or((0., 0.), |origin| (origin.x, origin.y));
		let press = Area {
			x: x - left,
			y: y - top,
			width: 0.,
			height: 0.,
		};
		let card = self.placed.get();
		let inside =
			press.x >= card.x && press.x <= card.right() && press.y >= card.y && press.y <= card.bottom();
		if !inside {
			self.close();
		}
	}

	pub fn open(&self, subject: PopoutSubject, at: Rect) {
		let origin = self.root.borrow().as_ref().and_then(bounds_of);
		let (left, top) = origin.map_or((0., 0.), |origin| (origin.x, origin.y));
		let item = Area {
			x: at.x - left,
			y: at.y - top,
			width: at.width,
			height: at.height,
		};
		let details = self.source.borrow().as_ref().and_then(|source| source(&subject));
		let Some(details) = details else {
			return;
		};
		self.set_images(self.wanted_images(&subject, &details));
		let serial = self.serial.get() + 1;
		self.serial.set(serial);
		self.height.set(0.);
		self.shown.set(Some(Shown {
			serial,
			subject,
			details,
			item,
		}));
	}

	fn set_images(&self, names: Vec<ImageName>) {
		if *self.images.borrow() == names {
			return;
		}
		*self.images.borrow_mut() = names;
		if let Some(changed) = self.changed.borrow().as_ref() {
			changed();
		}
	}

	fn art_image(&self, subject: &PopoutSubject) -> Option<ImageName> {
		match subject {
			PopoutSubject::Custom(Expression::Emoji { id, animated, .. }) => {
				Some(self.pictures.emoji(id.0, *animated, JUMBO_EMOJI))
			}
			PopoutSubject::Custom(Expression::Sticker { id, name, format }) => {
				self.pictures.sticker(&StickerItem {
					id: *id,
					name: name.clone(),
					format_type: Some(*format),
				})
			}
			PopoutSubject::Unicode(_) => None,
		}
	}

	fn server_icon(&self, server: &PopoutServer) -> Option<ImageName> {
		let hash = server.icon.as_deref()?;
		Some(self.pictures.guild_icon(server.guild_id, hash, SERVER_ICON))
	}

	fn wanted_images(&self, subject: &PopoutSubject, details: &ExpressionDetails) -> Vec<ImageName> {
		self.art_image(subject)
			.into_iter()
			.chain(
				details
					.server
					.as_ref()
					.and_then(|server| self.server_icon(server)),
			)
			.collect()
	}

	pub fn view(self: &Rc<Self>) -> View {
		let this = self.clone();
		let shown = self.shown.clone();
		div().child_when(move || shown.get(), move |shown: Shown, _| this.card(&shown))
	}

	fn card(self: &Rc<Self>, shown: &Shown) -> View {
		let (height, bounds, item) = (self.height.clone(), self.bounds.clone(), shown.item);
		let measured = self.height.clone();
		let watch = Rc::downgrade(self);
		let placing = Rc::downgrade(self);
		let body = match &shown.subject {
			PopoutSubject::Custom(Expression::Sticker { .. }) => self.sticker_header(shown),
			_ => self.emoji_header(shown),
		};
		column()
			.id(format!("expression-popout-{}", shown.serial))
			.absolute()
			.w(WIDTH)
			.p(PADDING)
			.gap(16.)
			.rounded(tok("radius_lg"))
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.reactive_style(move || {
				let measured = height.get();
				let (x, y) = place_popout(item, (WIDTH, measured), bounds.get(), GAP, MARGIN);
				if let Some(popout) = placing.upgrade() {
					popout.placed.set(Area {
						x,
						y,
						width: WIDTH,
						height: measured,
					});
				}
				let styles = Styles::new().left(x).top(y);
				if measured > 0. {
					styles.visible()
				} else {
					styles.invisible()
				}
			})
			.on_mount(move |node| {
				let Some(popout) = watch.upgrade() else {
					return;
				};
				let Some(size) = node.observe_size() else {
					return;
				};
				let measured = measured.clone();
				let effect = popout.runtime.effect(move || {
					let (_, height) = size.get();
					if height > 0. {
						measured.set(height);
					}
				});
				*popout.measure.borrow_mut() = Some(effect);
			})
			.child(body)
			.when_some(
				shown.details.server.clone().zip(shown.details.source_heading),
				|card, (server, heading)| {
					card.child(div().h(tok("border_thickness")).bg(tok("border")))
						.child(self.server_section(&server, heading))
				},
			)
	}

	fn art(&self, subject: &PopoutSubject, side: f32) -> View {
		match (subject, self.art_image(subject)) {
			(PopoutSubject::Unicode(glyph), _) => emoji::glyph(glyph.clone(), side),
			(_, Some(name)) => div().size(side, side).shrink_0().child(picture(
				&self.pictures,
				Some(name),
				None,
				ObjectFit::Contain,
			)),
			(_, None) => div().size(side, side).shrink_0(),
		}
	}

	fn emoji_header(&self, shown: &Shown) -> View {
		row()
			.items_center()
			.gap(12.)
			.child(self.art(&shown.subject, JUMBO_EMOJI as f32))
			.child(
				column()
					.grow()
					.min_w(0.)
					.child(title(&shown.details.title))
					.when_some(shown.details.description.clone(), |column, description| {
						column.child(
							text(description)
								.mt(2.)
								.text_wrap(true)
								.text_size(tok("font_size_sm"))
								.line_height(TEXT_LINE)
								.text_color(tok("text_4")),
						)
					}),
			)
	}

	fn sticker_header(&self, shown: &Shown) -> View {
		column()
			.child(
				row()
					.justify_center()
					.child(self.art(&shown.subject, STICKER_SIDE)),
			)
			.child(title(&shown.details.title).mt(16.))
			.when_some(shown.details.description.clone(), |column, description| {
				column.child(
					text(description)
						.mt(4.)
						.text_wrap(true)
						.text_size(tok("font_size_sm"))
						.line_height(TEXT_LINE)
						.text_color(tok("text_3")),
				)
			})
			.when_some(shown.details.note.clone(), |column, note| {
				column.child(
					text(note)
						.mt(8.)
						.text_wrap(true)
						.text_size(tok("font_size_sm"))
						.line_height(TEXT_LINE)
						.text_color(tok("text_4")),
				)
			})
	}

	fn server_section(&self, server: &PopoutServer, heading: &str) -> View {
		let icon = self.server_icon(server).map(|name| self.pictures.picture(&name));
		let acronym: String = server
			.name
			.split_whitespace()
			.filter_map(|word| word.chars().next())
			.take(3)
			.collect();
		let tile = div()
			.size(SERVER_ICON, SERVER_ICON)
			.shrink_0()
			.rounded(tok("radius_md"))
			.overflow_hidden()
			.bg(tok("accent_4"))
			.items_center()
			.justify_center()
			.child(
				text(acronym)
					.text_size(14.)
					.font_weight(700)
					.text_color(tok("text_0")),
			);
		let tile = match icon {
			Some(picture) => tile.reactive_style(move || match picture.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			}),
			None => tile,
		};
		column()
			.gap(12.)
			.child(
				text(heading.to_uppercase())
					.text_size(tok("font_size_xs"))
					.line_height(SMALL_LINE)
					.font_weight(700)
					.text_color(tok("text_4")),
			)
			.child(
				row().items_center().gap(12.).child(tile).child(
					column()
						.min_w(0.)
						.child(
							text(server.name.clone())
								.truncate()
								.text_size(tok("font_size_md"))
								.line_height(TITLE_LINE)
								.font_weight(600)
								.text_color(tok("text_2")),
						)
						.when_some(server.members, |column, members| {
							column.child(
								text(members_label(members))
									.text_size(tok("font_size_xs"))
									.line_height(SMALL_LINE)
									.text_color(tok("text_4")),
							)
						}),
				),
			)
	}
}

fn title(label: &str) -> View {
	text(label.to_owned())
		.text_wrap(true)
		.text_size(tok("font_size_lg"))
		.line_height(TITLE_LINE)
		.font_weight(600)
		.text_color(tok("text_2"))
}
