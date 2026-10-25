mod custom_status;
mod full;
mod parts;
mod popout;
mod status_menu;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::{EmojiPicker, PanelUser, ProfileCard};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::{Effect, Runtime, Signal};

use self::custom_status::CustomStatusForm;
pub use self::full::FullTab;
use crate::timeline::picker::ReactionPicker;
use crate::timeline::pictures::Pictures;

const POPOUT_AVATAR: f64 = 80.;
const FULL_AVATAR: f64 = 120.;
const MENU_AVATAR: f64 = 40.;
const STATUS_EMOJI: f64 = 16.;
const LIST_ICON: f64 = 40.;
const SNAP_MARGIN: f32 = 8.;
const MENU_RAISE: f32 = 20.;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
	TopLeft,
	TopRight,
	BottomLeft,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opening {
	pub user_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub at: (f32, f32),
	pub anchor: Anchor,
}

#[derive(Debug)]
pub enum Request {
	Command(Command),
	Message(Snowflake),
	FullProfile(Snowflake),
	EditProfile,
	SwitchAccounts,
	SignOut,
	Picker,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Surface {
	Popout {
		serial: u64,
		at: (f32, f32),
		anchor: Anchor,
	},
	Full {
		serial: u64,
	},
}

#[derive(Default, Clone, PartialEq)]
pub struct CardImages {
	pub avatar: Option<ImageName>,
	pub banner: Option<ImageName>,
	pub badges: Vec<ImageName>,
	pub emoji: Option<ImageName>,
	pub guilds: Vec<Option<ImageName>>,
	pub friends: Vec<Option<ImageName>>,
}

impl CardImages {
	fn names(&self) -> Vec<ImageName> {
		self.avatar
			.iter()
			.chain(&self.banner)
			.chain(&self.badges)
			.chain(&self.emoji)
			.chain(self.guilds.iter().flatten())
			.chain(self.friends.iter().flatten())
			.cloned()
			.collect()
	}
}

#[derive(Clone)]
pub struct Shown {
	serial: u64,
	pub card: Rc<ProfileCard>,
	pub images: Rc<CardImages>,
}

impl PartialEq for Shown {
	fn eq(&self, other: &Self) -> bool {
		self.serial == other.serial
	}
}

#[derive(Clone)]
pub struct MenuShown {
	serial: u64,
	at: (f32, f32),
	pub panel: Rc<PanelUser>,
	pub avatar: Option<ImageName>,
}

impl PartialEq for MenuShown {
	fn eq(&self, other: &Self) -> bool {
		self.serial == other.serial
	}
}

pub struct ProfileLayer {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	picker: Rc<ReactionPicker>,
	window: Signal<(f32, f32)>,
	surface: Signal<Option<Surface>>,
	shown: Signal<Option<Shown>>,
	tab: Signal<FullTab>,
	target: Cell<Option<(Snowflake, Option<Snowflake>)>>,
	menu: Signal<Option<MenuShown>>,
	panel: RefCell<Option<Rc<PanelUser>>>,
	custom: Rc<CustomStatusForm>,
	serials: Cell<u64>,
	popout_size: Signal<(f32, f32)>,
	menu_size: Signal<(f32, f32)>,
	effects: RefCell<Vec<Effect>>,
	images: RefCell<Vec<ImageName>>,
	requests: Hook<Request>,
	images_changed: Hook<()>,
}

impl ProfileLayer {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let picker = Rc::new(ReactionPicker::new(runtime, pictures.clone()));
		let layer = Rc::new(Self {
			runtime: runtime.clone(),
			custom: Rc::new(CustomStatusForm::new(runtime)),
			pictures,
			picker,
			window: runtime.signal((0., 0.)),
			surface: runtime.signal(None),
			shown: runtime.signal(None),
			tab: runtime.signal(FullTab::Info),
			target: Cell::new(None),
			menu: runtime.signal(None),
			panel: RefCell::new(None),
			serials: Cell::new(0),
			popout_size: runtime.signal((0., 0.)),
			menu_size: runtime.signal((0., 0.)),
			effects: RefCell::default(),
			images: RefCell::default(),
			requests: RefCell::new(None),
			images_changed: RefCell::new(None),
		});
		let weak = Rc::downgrade(&layer);
		layer.picker.on_images_changed(move || {
			if let Some(layer) = weak.upgrade() {
				layer.images_changed();
			}
		});
		layer
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

	fn images_changed(&self) {
		if let Some(hook) = self.images_changed.borrow().as_ref() {
			hook(());
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

	pub fn target(&self) -> Option<(Snowflake, Option<Snowflake>)> {
		self.target.get()
	}

	pub fn is_open(&self) -> bool {
		self.surface.with_untracked(Option::is_some)
			|| self.menu.with_untracked(Option::is_some)
			|| self.custom.is_open()
	}

	pub fn has_full_profile(&self) -> bool {
		self.surface
			.with_untracked(|surface| matches!(surface, Some(Surface::Full { .. })))
	}

	pub fn open_popout(&self, opening: Opening) {
		self.close_menus();
		self.target.set(Some((opening.user_id, opening.guild_id)));
		self.shown.set(None);
		self.popout_size.set((0., 0.));
		self.surface.set(Some(Surface::Popout {
			serial: self.serial(),
			at: opening.at,
			anchor: opening.anchor,
		}));
	}

	pub fn open_full(&self, user_id: Snowflake, guild_id: Option<Snowflake>) {
		self.close_menus();
		self.shown.set(None);
		self.target.set(Some((user_id, guild_id)));
		self.tab.set(FullTab::Info);
		self.surface.set(Some(Surface::Full {
			serial: self.serial(),
		}));
	}

	pub fn open_status_menu(&self, at: (f32, f32)) {
		if self.menu.with_untracked(Option::is_some) {
			self.menu.set(None);
			return;
		}
		let Some(panel) = self.panel.borrow().clone() else {
			return;
		};
		self.close_profile();
		self.custom.close();
		let avatar = bridge::avatar_request(
			self.pictures.cdn(),
			&panel.user,
			MENU_AVATAR,
			self.pictures.scale(),
			false,
		)
		.map(|request| self.pictures.request(request));
		self.menu_size.set((0., 0.));
		self.menu.set(Some(MenuShown {
			serial: self.serial(),
			at: (at.0, at.1 - MENU_RAISE),
			panel,
			avatar,
		}));
		self.refresh_images();
	}

	pub fn open_custom_status(&self) {
		self.close_menus();
		self.close_profile();
		let current = self
			.panel
			.borrow()
			.as_ref()
			.map(|panel| panel.custom_status.clone());
		self.custom.open(current.unwrap_or_default());
	}

	pub fn set_panel(&self, panel: Option<PanelUser>) {
		*self.panel.borrow_mut() = panel.map(Rc::new);
	}

	pub fn close_profile(&self) {
		self.surface.set(None);
		self.target.set(None);
		self.shown.set(None);
		self.refresh_images();
	}

	fn close_menus(&self) {
		self.menu.set(None);
		self.picker.close();
	}

	pub fn close_all(&self) {
		self.close_menus();
		self.custom.close();
		self.close_profile();
	}

	pub fn escape(&self) -> bool {
		if self.picker.is_open() {
			self.picker.close();
			return true;
		}
		let open = self.is_open();
		self.close_all();
		open
	}

	pub fn show_card(&self, card: Option<ProfileCard>) {
		let Some(card) = card else {
			if self.shown.with_untracked(Option::is_some) {
				self.shown.set(None);
			}
			return;
		};
		let same = self
			.shown
			.with_untracked(|shown| shown.as_ref().is_some_and(|shown| *shown.card == card));
		if same {
			return;
		}
		let images = self.card_images(&card);
		self.shown.set(Some(Shown {
			serial: self.serial(),
			card: Rc::new(card),
			images: Rc::new(images),
		}));
		self.refresh_images();
	}

	fn card_images(&self, card: &ProfileCard) -> CardImages {
		let (cdn, scale) = (self.pictures.cdn().to_owned(), self.pictures.scale());
		let request =
			|request: Option<media::ImageRequest>| request.map(|request| self.pictures.request(request));
		let points = if self.has_full_profile() {
			FULL_AVATAR
		} else {
			POPOUT_AVATAR
		};
		CardImages {
			avatar: request(card.avatar_request(&cdn, points, scale)),
			banner: request(card.banner_request()),
			badges: card
				.badge_requests()
				.into_iter()
				.map(|request| self.pictures.request(request))
				.collect(),
			emoji: request(card.custom_status.emoji_request(&cdn, STATUS_EMOJI, scale)),
			guilds: card
				.mutual_guilds
				.iter()
				.map(|guild| request(ProfileCard::guild_icon_request(guild, &cdn, LIST_ICON, scale)))
				.collect(),
			friends: card
				.mutual_friends
				.iter()
				.map(|friend| {
					request(
						friend
							.user
							.as_ref()
							.and_then(|user| bridge::avatar_request(&cdn, user, LIST_ICON, scale, false)),
					)
				})
				.collect(),
		}
	}

	fn refresh_images(&self) {
		let mut names: Vec<ImageName> = Vec::new();
		if self.surface.with_untracked(Option::is_some) {
			names.extend(
				self.shown
					.with_untracked(|shown| shown.as_ref().map(|shown| shown.images.names()))
					.unwrap_or_default(),
			);
		}
		names.extend(
			self.menu
				.with_untracked(|menu| menu.as_ref().and_then(|menu| menu.avatar.clone())),
		);
		if *self.images.borrow() != names {
			*self.images.borrow_mut() = names;
			self.images_changed();
		}
	}

	pub fn images(&self) -> Vec<ImageName> {
		let mut names = self.images.borrow().clone();
		names.extend(self.picker.images());
		names
	}

	pub fn settled(&self) -> bool {
		self.unsettled() == 0
	}

	pub fn unsettled(&self) -> usize {
		let waiting = match self.surface.with_untracked(|surface| *surface) {
			Some(_) => self.shown.with_untracked(Option::is_none),
			None => false,
		};
		let images = self
			.images
			.borrow()
			.iter()
			.filter(|name| !self.pictures.is_settled(name))
			.count();
		images + usize::from(waiting)
	}

	fn open_picker(self: &Rc<Self>, at: (f32, f32), model: EmojiPicker) {
		let form = self.custom.clone();
		self.picker.open(model, at, move |cell| {
			if cell.custom.is_none()
				&& let Some(glyph) = cell.glyph
			{
				form.set_emoji(Some(glyph));
			}
		});
	}

	pub fn pick_emoji(self: &Rc<Self>, model: EmojiPicker) {
		let at = self.custom.emoji_spot();
		self.open_picker(at, model);
	}

	fn place(at: (f32, f32), anchor: Anchor, size: (f32, f32), window: (f32, f32)) -> (f32, f32) {
		let (x, y) = match anchor {
			Anchor::TopLeft => at,
			Anchor::TopRight => (at.0 - size.0, at.1),
			Anchor::BottomLeft => (at.0, at.1 - size.1),
		};
		let clamp =
			|value: f32, extent: f32, room: f32| value.min(room - extent - SNAP_MARGIN).max(SNAP_MARGIN);
		(clamp(x, size.0, window.0), clamp(y, size.1, window.1))
	}

	fn measured(&self, node: &zgx::compose::NodeHandle, into: &Signal<(f32, f32)>) {
		let Some(observed) = node.observe_size() else {
			return;
		};
		let into = into.clone();
		let effect = self.runtime.effect(move || {
			let size = observed.get();
			if into.with_untracked(|current| *current != size) {
				into.set(size);
			}
		});
		self.effects.borrow_mut().push(effect);
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (surface, menu, window) = (self.surface.clone(), self.menu.clone(), self.window.clone());
		let custom = self.custom.clone();
		let outside = self.clone();
		div()
			.absolute()
			.left(0.)
			.top(0.)
			.reactive_style(move || {
				let open =
					surface.with(Option::is_some) || menu.with(Option::is_some) || custom.open_signal().get();
				let (width, height) = if open { window.get() } else { (0., 0.) };
				Styles::new().w(width).h(height)
			})
			.on_event(move |event| {
				if event.phase == EventPhase::Target && matches!(event.event, InputEvent::PointerDown { .. })
				{
					outside.close_menus();
					if !outside.has_full_profile() {
						outside.close_profile();
					}
					event.stop_propagation();
				}
			})
			.child(self.full_view())
			.child(self.popout_view())
			.child(self.menu_view())
			.child(self.custom_view())
			.child(self.picker.view())
	}

	fn popout_view(self: &Rc<Self>) -> View {
		let (surface, shown) = (self.surface.clone(), self.shown.clone());
		let this = self.clone();
		div().child_when(
			move || match (surface.get(), shown.get()) {
				(Some(Surface::Popout { serial, at, anchor }), Some(shown)) => {
					Some((serial, at, anchor, shown))
				}
				_ => None,
			},
			move |(_, at, anchor, shown): (u64, (f32, f32), Anchor, Shown), _| {
				let (size, window) = (this.popout_size.clone(), this.window.clone());
				let measured = this.clone();
				let target = this.popout_size.clone();
				popout::card(&this, &shown)
					.absolute()
					.reactive_style(move || {
						let (x, y) = ProfileLayer::place(at, anchor, size.get(), window.get());
						Styles::new().left(x).top(y)
					})
					.on_mount(move |node| measured.measured(&node, &target))
			},
		)
	}

	fn full_view(self: &Rc<Self>) -> View {
		let (surface, shown) = (self.surface.clone(), self.shown.clone());
		let this = self.clone();
		div().child_when(
			move || match (surface.get(), shown.get()) {
				(Some(Surface::Full { serial }), Some(shown)) => Some((serial, shown)),
				_ => None,
			},
			move |(_, shown): (u64, Shown), _| full::scrim(&this, &shown),
		)
	}

	fn menu_view(self: &Rc<Self>) -> View {
		let menu = self.menu.clone();
		let this = self.clone();
		div().child_when(
			move || menu.get(),
			move |shown: MenuShown, _| {
				let (size, window) = (this.menu_size.clone(), this.window.clone());
				let measured = this.clone();
				let target = this.menu_size.clone();
				let at = shown.at;
				status_menu::menu(&this, &shown)
					.absolute()
					.reactive_style(move || {
						let (x, y) = ProfileLayer::place(at, Anchor::BottomLeft, size.get(), window.get());
						Styles::new().left(x).top(y)
					})
					.on_mount(move |node| measured.measured(&node, &target))
			},
		)
	}

	fn custom_view(self: &Rc<Self>) -> View {
		custom_status::dialog(self)
	}

	fn choose_status(&self, choice: bridge::StatusChoice) {
		self.menu.set(None);
		self.request(Request::Command(Command::SetStatus(choice.kind())));
	}
}
