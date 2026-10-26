use std::cell::RefCell;
use std::rc::Rc;

use bridge::{Leading, QuickSwitcher, SwitcherCategory, SwitcherEntry, SwitcherTarget};
use media::ImageName;
use zgx::compose::prelude::*;
use zgx::compose::{ModalPlacement, PlacementEdge};
use zgx::input::{InputEvent, Key};
use zgx::reactive::{Effect, Runtime, Signal};
use zgx::style::ObjectFit;

use crate::motion;
use crate::timeline::{Pictures, icon, tok};

const TOP: f32 = 180.;
const LIST_MAX: f32 = 400.;
const ICON: f32 = 20.;
const AVATAR: f64 = 20.;
const PLACEHOLDER: &str = "Where would you like to go?";
const EMPTY: &str = "No results match your search.";
const TIP_LEAD: &str = "PROTIP: ";
const TIP: &str = "Start searches with @, #, !, or * to narrow down results.";
const ENTER: &str = "↵";

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Clone, PartialEq)]
enum Item {
	Header(SwitcherCategory),
	Entry {
		position: usize,
		entry: SwitcherEntry,
		selected: bool,
	},
}

pub struct SwitcherView {
	runtime: Runtime,
	open: Signal<bool>,
	query: Signal<String>,
	items: Signal<Rc<Vec<Item>>>,
	model: RefCell<QuickSwitcher>,
	pictures: Rc<Pictures>,
	images: RefCell<Vec<ImageName>>,
	pick: Hook<SwitcherTarget>,
	images_changed: Hook<()>,
	watchers: RefCell<Vec<Effect>>,
}

impl SwitcherView {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let view = Rc::new(Self {
			runtime: runtime.clone(),
			open: runtime.signal(false),
			query: runtime.signal(String::new()),
			items: runtime.signal(Rc::new(Vec::new())),
			model: RefCell::new(QuickSwitcher::default()),
			pictures,
			images: RefCell::default(),
			pick: RefCell::new(None),
			images_changed: RefCell::new(None),
			watchers: RefCell::default(),
		});
		let weak = Rc::downgrade(&view);
		let query = view.query.clone();
		let effect = runtime.effect(move || {
			let text = query.get();
			if let Some(view) = weak.upgrade() {
				view.runtime.untracked(|| {
					view.model.borrow_mut().set_query(&text);
					view.refresh();
				});
			}
		});
		let weak = Rc::downgrade(&view);
		let open = view.open.clone();
		let closed = runtime.effect(move || {
			if !open.get()
				&& let Some(view) = weak.upgrade()
			{
				view.runtime.untracked(|| view.set_images(Vec::new()));
			}
		});
		*view.watchers.borrow_mut() = vec![effect, closed];
		view
	}

	pub fn on_pick(&self, pick: impl Fn(SwitcherTarget) + 'static) {
		*self.pick.borrow_mut() = Some(Box::new(pick));
	}

	pub fn on_images_changed(&self, changed: impl Fn(()) + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn open(&self, model: QuickSwitcher) {
		*self.model.borrow_mut() = model;
		self.runtime.batch(|| {
			self.query.set(String::new());
			self.refresh();
			self.open.set(true);
		});
	}

	pub fn close(&self) {
		self.open.set(false);
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

	fn refresh(&self) {
		let model = self.model.borrow();
		let selected = model.selected();
		let mut items = Vec::new();
		let mut previous = None;
		let mut images = Vec::new();
		for (position, shown) in model.results().iter().enumerate() {
			if previous != Some(shown.category) {
				previous = Some(shown.category);
				items.push(Item::Header(shown.category));
			}
			let entry = model.entry(shown).clone();
			images.extend(self.image_of(&entry.leading));
			items.push(Item::Entry {
				position,
				entry,
				selected: position == selected,
			});
		}
		drop(model);
		self.items.set(Rc::new(items));
		self.set_images(images);
	}

	fn set_images(&self, images: Vec<ImageName>) {
		if *self.images.borrow() == images {
			return;
		}
		*self.images.borrow_mut() = images;
		if let Some(changed) = self.images_changed.borrow().as_ref() {
			changed(());
		}
	}

	fn image_of(&self, leading: &Leading) -> Option<ImageName> {
		match leading {
			Leading::Icon(_) => None,
			Leading::User(user) => self.pictures.avatar(user, AVATAR),
			Leading::GuildIcon { guild_id, hash } => {
				Some(self.pictures.guild_icon(*guild_id, hash, AVATAR as f32))
			}
		}
	}

	fn step(&self, forward: bool) {
		self.model.borrow_mut().step(forward);
		self.refresh();
	}

	fn choose(&self, position: usize) {
		self.model.borrow_mut().select(position);
		let picked = self.model.borrow().picked();
		if let Some(target) = picked {
			self.close();
			if let Some(pick) = self.pick.borrow().as_ref() {
				pick(target);
			}
		}
	}

	fn choose_selected(&self) {
		let selected = self.model.borrow().selected();
		self.choose(selected);
	}

	pub fn type_query(&self, text: &str) {
		self.query.set(text.to_owned());
	}

	pub fn view(self: &Rc<Self>) -> View {
		let keys = self.clone();
		let input = text_input(PLACEHOLDER, self.query.clone())
			.placeholder(PLACEHOLDER)
			.w_full()
			.h(tok("switcher_input_height"))
			.shrink_0()
			.px(tok("space_16"))
			.rounded(tok("radius_sm"))
			.border(tok("border_thickness"))
			.border_color(tok("accent_2"))
			.bg(tok("bg_3"))
			.text_color(tok("text_2"))
			.text_size(tok("font_size_xl"))
			.line_height(tok("line_height_xl"))
			.focus(|style| style.border_color(tok("accent_2")).outline_width(0.))
			.on_event(move |event| {
				let InputEvent::KeyDown { key, .. } = &event.event else {
					return;
				};
				match key {
					Key::ArrowDown => keys.step(true),
					Key::ArrowUp => keys.step(false),
					Key::Enter => keys.choose_selected(),
					_ => return,
				}
				event.prevent_default();
				event.stop_propagation();
			});
		let this = self.clone();
		let list = switch(
			{
				let items = self.items.clone();
				move || items.get()
			},
			move |items: Rc<Vec<Item>>, _| this.list(&items),
		)
		.w_full();
		div().child(
			motion::modal("Quick switcher", self.open.clone())
				.modal_placement(ModalPlacement {
					x: PlacementEdge::Center,
					y: PlacementEdge::Start,
					insets: [TOP, 0., 0., 0.],
				})
				.w(tok("modal_width_md"))
				.pt(tok("space_24"))
				.px(tok("space_24"))
				.pb(tok("space_16"))
				.gap(tok("space_16"))
				.rounded(tok("radius_lg"))
				.bg(tok("bg_4"))
				.border(tok("border_thickness"))
				.border_color(tok("border"))
				.text_color(tok("text_3"))
				.child(input)
				.child(list)
				.child(tip()),
		)
	}

	fn list(self: &Rc<Self>, items: &[Item]) -> View {
		let list = column()
			.w_full()
			.gap(tok("space_2"))
			.max_h(LIST_MAX)
			.overflow_hidden();
		if items.is_empty() {
			return list.child(
				text(EMPTY)
					.p(tok("space_8"))
					.text_size(tok("font_size_sm"))
					.line_height(tok("line_height_sm"))
					.text_color(tok("text_4")),
			);
		}
		list.children(items.iter().enumerate().map(|(index, item)| {
			match item {
				Item::Header(category) => text(category.title())
					.px(tok("space_8"))
					.when(index > 0, |header| header.pt(tok("space_8")))
					.pb(tok("space_4"))
					.text_size(tok("font_size_xs"))
					.line_height(tok("line_height_xs"))
					.font_weight(700)
					.text_color(tok("text_4")),
				Item::Entry {
					position,
					entry,
					selected,
				} => self.row(*position, entry, *selected),
			}
		}))
	}

	fn row(self: &Rc<Self>, position: usize, entry: &SwitcherEntry, selected: bool) -> View {
		let this = self.clone();
		row()
			.w_full()
			.h(tok("switcher_row_height"))
			.shrink_0()
			.items_center()
			.gap(tok("space_8"))
			.px(tok("space_8"))
			.rounded(tok("radius_xs"))
			.cursor(Cursor::Pointer)
			.when(selected, |row| row.bg(tok("active")))
			.when(!selected, |row| row.hover(|style| style.bg(tok("hover"))))
			.on_click(move || this.choose(position))
			.child(self.leading(entry))
			.child(
				text(entry.name.clone())
					.grow()
					.min_w(0.)
					.truncate()
					.text_size(tok("font_size_md"))
					.line_height(tok("line_height_label"))
					.font_weight(500)
					.text_color(tok(if selected { "text_2" } else { "text_3" })),
			)
			.when_some(entry.context.clone(), |row, context| {
				row.child(
					text(context.to_uppercase())
						.shrink_0()
						.text_size(tok("font_size_xs"))
						.line_height(tok("line_height_xs"))
						.font_weight(600)
						.text_color(tok("text_4")),
				)
			})
			.when(selected, |row| row.child(enter_key()))
	}

	fn leading(&self, entry: &SwitcherEntry) -> View {
		let Some(name) = self.image_of(&entry.leading) else {
			let glyph = match entry.leading {
				Leading::Icon(name) => name,
				_ => "at_sign",
			};
			return icon(glyph, ICON, "text_4");
		};
		let picture = self.pictures.picture(&name);
		let size = AVATAR as f32;
		let initial = entry.name.chars().next().map(String::from).unwrap_or_default();
		let shown = picture.clone();
		div()
			.size(size, size)
			.rounded(size / 2.)
			.overflow_hidden()
			.shrink_0()
			.bg(tok("bg_2"))
			.items_center()
			.justify_center()
			.reactive_style(move || match shown.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			})
			.child(
				text(initial)
					.text_size(size * 0.42)
					.font_weight(600)
					.text_color(tok("text_0"))
					.reactive_style(move || {
						if picture.with(Option::is_some) {
							Styles::new().invisible()
						} else {
							Styles::new().visible()
						}
					}),
			)
	}
}

fn tip() -> View {
	row()
		.w_full()
		.shrink_0()
		.pt(tok("space_12"))
		.border_edges(zgx::scene::Insets {
			top: 1.,
			..Default::default()
		})
		.border_color(tok("border"))
		.child(
			text(TIP_LEAD)
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.font_weight(700)
				.text_color(tok("green_2")),
		)
		.child(
			text(TIP)
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.text_color(tok("text_4")),
		)
}

fn enter_key() -> View {
	text(ENTER)
		.shrink_0()
		.px(tok("space_4"))
		.rounded(tok("radius_xs"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.text_size(tok("font_size_xs"))
		.line_height(tok("line_height_xs"))
		.font_weight(600)
		.text_color(tok("text_4"))
}
