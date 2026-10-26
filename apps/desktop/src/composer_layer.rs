use std::cell::RefCell;
use std::rc::Rc;

use bridge::{EmojiPicker, FileEdit};
use media::ImageName;
use protocol::PartialEmoji;
use session::PollDraft;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::{Runtime, Signal};

use crate::kit::{self, Variant};
use crate::motion;
use crate::pickers::{CELL_IMAGE, ExpressionPicker, Mode, Tab};
use crate::poll_dialog::PollForm;
use crate::timeline::{Pictures, icon, tok};

const MENU_ICON: f32 = 20.;
const MENU_OFFSET: f32 = 8.;
const DROP_ICON: f32 = 40.;
const DROP_DASH: f32 = 8.;
const DROP_BORDER: f32 = 2.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
	Main,
	Thread,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
	Upload(Pane),
	CreateThread,
	SaveFile(Pane, u64, FileEdit),
	SendPoll(Pane, PollDraft),
	PollEmoji,
	ImagesChanged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlusMenu {
	pub pane: Pane,
	pub at: (f32, f32),
	pub thread: bool,
	pub poll: bool,
}

#[derive(Clone)]
struct Editing {
	pane: Pane,
	id: u64,
	title: String,
	name: Signal<String>,
	description: Signal<String>,
	spoiler: Signal<bool>,
}

impl PartialEq for Editing {
	fn eq(&self, other: &Self) -> bool {
		std::ptr::eq(self, other)
	}
}

type Handler = RefCell<Option<Box<dyn Fn(Request)>>>;

pub struct ComposerLayer {
	runtime: Runtime,
	menu_open: Signal<bool>,
	menu: Signal<Option<PlusMenu>>,
	edit_open: Signal<bool>,
	editing: Signal<Option<Rc<Editing>>>,
	drop: Signal<Option<String>>,
	window: Signal<(f32, f32)>,
	requests: Handler,
	pictures: Rc<Pictures>,
	poll: Rc<PollForm>,
	poll_pane: std::cell::Cell<Pane>,
	picker: Rc<ExpressionPicker>,
}

impl ComposerLayer {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let layer = Rc::new(Self {
			runtime: runtime.clone(),
			menu_open: runtime.signal(false),
			menu: runtime.signal(None),
			edit_open: runtime.signal(false),
			editing: runtime.signal(None),
			drop: runtime.signal(None),
			window: runtime.signal((0., 0.)),
			requests: RefCell::new(None),
			poll: PollForm::new(runtime, pictures.clone()),
			picker: ExpressionPicker::new(runtime, pictures.clone(), Mode::Reaction),
			poll_pane: std::cell::Cell::new(Pane::Main),
			pictures,
		});
		let weak = Rc::downgrade(&layer);
		layer.poll.on_post(move |draft| {
			if let Some(layer) = weak.upgrade() {
				layer.picker.close();
				layer.request(Request::SendPoll(layer.poll_pane.get(), draft));
			}
		});
		let weak = Rc::downgrade(&layer);
		layer.poll.on_pick_emoji(move |_| {
			if let Some(layer) = weak.upgrade() {
				layer.request(Request::PollEmoji);
			}
		});
		let weak = Rc::downgrade(&layer);
		layer.picker.on_images_changed(move || {
			if let Some(layer) = weak.upgrade() {
				layer.request(Request::ImagesChanged);
			}
		});
		let weak = Rc::downgrade(&layer);
		layer.picker.on_emoji(move |cell| {
			let Some(layer) = weak.upgrade() else {
				return;
			};
			let emoji = match (&cell.glyph, &cell.custom) {
				(Some(glyph), _) => PartialEmoji {
					id: None,
					name: Some(glyph.clone()),
					animated: false,
				},
				(None, Some(custom)) => PartialEmoji {
					id: Some(custom.id),
					name: None,
					animated: custom.animated,
				},
				(None, None) => return,
			};
			layer.poll.set_emoji(emoji);
			layer.picker.close();
			layer.request(Request::ImagesChanged);
		});
		layer
	}

	pub fn poll_form(&self) -> &Rc<PollForm> {
		&self.poll
	}

	pub fn open_poll(&self, pane: Pane) {
		self.poll_pane.set(pane);
		self.picker.close();
		self.poll.open();
	}

	pub fn pick_poll_emoji(&self, model: EmojiPicker) {
		let (width, _) = self.window.with_untracked(|window| *window);
		let (_, y) = self.poll.spot();
		let right = (width + crate::poll_dialog::DIALOG_WIDTH) / 2.;
		self.picker.set_emoji(model);
		self.picker.open(Tab::Emoji, (right, y));
	}

	pub fn images(&self) -> Vec<ImageName> {
		let mut names = self.picker.images();
		names.extend(
			self.poll
				.custom_emoji()
				.into_iter()
				.map(|(id, animated)| self.pictures.emoji(id, animated, f64::from(CELL_IMAGE))),
		);
		names
	}

	pub fn on_request(&self, handler: impl Fn(Request) + 'static) {
		*self.requests.borrow_mut() = Some(Box::new(handler));
	}

	fn request(&self, request: Request) {
		if let Some(handler) = self.requests.borrow().as_ref() {
			handler(request);
		}
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn open_menu(&self, menu: PlusMenu) {
		self.menu.set(Some(menu));
		self.menu_open.set(true);
	}

	pub fn menu_shown(&self) -> bool {
		self.menu_open.with_untracked(|open| *open)
	}

	pub fn edit_file(&self, pane: Pane, id: u64, edit: FileEdit) {
		self.editing.set(Some(Rc::new(Editing {
			pane,
			id,
			title: edit.name.clone(),
			name: self.runtime.signal(edit.name),
			description: self.runtime.signal(edit.description),
			spoiler: self.runtime.signal(edit.spoiler),
		})));
		self.edit_open.set(true);
	}

	pub fn show_drop(&self, target: Option<String>) {
		if self.drop.with_untracked(|shown| *shown != target) {
			self.drop.set(target);
		}
	}

	pub fn view(self: &Rc<Self>) -> View {
		div()
			.child(self.menu_view())
			.child(self.edit_view())
			.child(self.drop_view())
			.child(self.poll.view())
			.child(self.picker.view())
	}

	fn menu_view(self: &Rc<Self>) -> View {
		let (shown, this) = (self.menu.clone(), self.clone());
		let open = self.menu_open.clone();
		div().child_when(
			move || shown.get(),
			move |menu: PlusMenu, _| this.menu_panel(menu, open.clone()),
		)
	}

	fn menu_panel(self: &Rc<Self>, menu: PlusMenu, open: Signal<bool>) -> View {
		let (x, y) = menu.at;
		let spot = div().absolute().left(x).top(y).w(0.).h(0.);
		let (upload, thread, poll) = (self.clone(), self.clone(), self.clone());
		let pane = menu.pane;
		let mut items = vec![
			entry("Upload a File", "upload")
				.id("composer-menu-upload")
				.on_click(move || upload.choose(Request::Upload(pane))),
		];
		if menu.thread {
			items.push(
				entry("Create Thread", "thread")
					.id("composer-menu-thread")
					.on_click(move || thread.choose(Request::CreateThread)),
			);
		}
		if menu.poll {
			items.push(
				entry("Create Poll", "poll")
					.id("composer-menu-poll")
					.on_click(move || {
						poll.menu_open.set(false);
						poll.open_poll(pane);
					}),
			);
		}
		kit::menu_panel(
			zgx::compose::menu("Composer menu", open, spot).placement(Placement {
				side: PlacementSide::Top,
				align: PlacementEdge::Start,
				offset: MENU_OFFSET,
			}),
		)
		.children(items)
	}

	fn choose(&self, request: Request) {
		self.menu_open.set(false);
		self.request(request);
	}

	fn edit_view(self: &Rc<Self>) -> View {
		let (shown, open, this) = (self.editing.clone(), self.edit_open.clone(), self.clone());
		kit::dialog(motion::modal("Edit attachment", self.edit_open.clone()))
			.w(tok("modal_width_sm"))
			.child_when(
				move || if open.get() { shown.get() } else { None },
				move |editing: Rc<Editing>, _| this.edit_content(&editing),
			)
	}

	fn edit_content(self: &Rc<Self>, editing: &Rc<Editing>) -> View {
		let (close, cancel, save) = (self.clone(), self.clone(), self.clone());
		let saved = editing.clone();
		let spoiler = editing.spoiler.clone();
		let (checked, flip) = (spoiler.clone(), spoiler);
		column()
			.w_full()
			.child(kit::header(kit::title(editing.title.clone()), None, move || {
				close.edit_open.set(false);
			}))
			.child(
				kit::body()
					.child(kit::field(
						"Filename",
						kit::input("Filename", editing.name.clone(), "").id("attachment-name"),
					))
					.child(kit::field(
						"Description (alt text)",
						kit::input("Description", editing.description.clone(), "Add a description")
							.id("attachment-description"),
					))
					.child(
						row()
							.id("attachment-spoiler")
							.items_center()
							.gap(tok("space_8"))
							.cursor(Cursor::Pointer)
							.on_click(move || {
								flip.set(!flip.with_untracked(|on| *on));
							})
							.child(kit::checkbox(move || checked.get()))
							.child(
								text("Mark as spoiler")
									.text_size(tok("font_size_md"))
									.line_height(tok("line_height_label"))
									.text_color(tok("text_3")),
							),
					)
					.child(kit::footer(vec![
						kit::action("Cancel", Variant::Link, move || {
							cancel.edit_open.set(false);
						}),
						kit::action("Save", Variant::Primary, move || save.save(&saved))
							.id("attachment-save"),
					])),
			)
	}

	fn save(&self, editing: &Editing) {
		self.edit_open.set(false);
		self.request(Request::SaveFile(
			editing.pane,
			editing.id,
			FileEdit {
				name: editing.name.with_untracked(Clone::clone),
				description: editing.description.with_untracked(Clone::clone),
				spoiler: editing.spoiler.with_untracked(|on| *on),
			},
		));
	}

	fn drop_view(&self) -> View {
		let (target, window) = (self.drop.clone(), self.window.clone());
		div().child_when(
			move || {
				let (width, height) = window.get();
				target.get().map(|target| (target, width, height))
			},
			|(target, width, height): (String, f32, f32), _| drop_card(&target, width, height),
		)
	}
}

fn entry(label: &str, glyph: &'static str) -> View {
	menu_item(label.to_owned())
		.h(tok("menu_item_height"))
		.px(tok("space_8"))
		.py(0.)
		.rounded(tok("radius_xs"))
		.child(
			row()
				.w_full()
				.items_center()
				.gap(tok("space_8"))
				.child(icon(glyph, MENU_ICON, "text_4"))
				.child(
					text(label.to_owned())
						.grow()
						.min_w(0.)
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.font_weight(500)
						.text_color(tok("text_3")),
				),
		)
}

fn drop_card(target: &str, width: f32, height: f32) -> View {
	div()
		.id("composer-drop")
		.absolute()
		.left(0.)
		.top(0.)
		.w(width)
		.h(height)
		.items_center()
		.justify_center()
		.bg(tok("overlay"))
		.pointer_events(false)
		.child(
			column()
				.w(tok("drop_card_width"))
				.items_center()
				.gap(tok("space_8"))
				.px(tok("space_24"))
				.py(tok("space_32"))
				.rounded(tok("radius_lg"))
				.bg(tok("bg_4"))
				.border(DROP_BORDER)
				.border_style(BorderStyle::Dashed {
					length: DROP_DASH,
					gap: DROP_DASH,
				})
				.border_color(tok("accent_3"))
				.child(icon("upload", DROP_ICON, "text_3"))
				.child(
					text(format!("Upload to {target}"))
						.mt(tok("space_8"))
						.text_size(tok("font_size_lg"))
						.line_height(tok("line_height_lg"))
						.font_weight(600)
						.text_color(tok("text_2")),
				)
				.child(
					text("You can add comments before uploading. Hold shift to upload directly.")
						.text_wrap(true)
						.text_align(TextAlign::Center)
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.text_color(tok("text_4")),
				),
		)
}
