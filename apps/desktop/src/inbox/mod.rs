mod panel;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;

use bridge::{ForYouEntry, InboxTab, MentionGroup, UnreadGroup};
use media::ImageName;
use protocol::Snowflake;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};
use zgx::reactive::{Runtime, Signal};

use crate::profile::parts;
use crate::timeline::Pictures;

const ANCHOR_RIGHT: f32 = 16.;
const SNAP_MARGIN: f32 = 8.;
const CARD_AVATAR: f64 = 40.;
const GUILD_ICON: f32 = 24.;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filter {
	pub everyone: bool,
	pub roles: bool,
	pub this_server: bool,
}

impl Default for Filter {
	fn default() -> Self {
		Self {
			everyone: true,
			roles: true,
			this_server: false,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
	Load(InboxTab, Filter),
	JumpTo {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	OpenChannel {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	},
	Remove(Vec<Snowflake>),
	MarkRead(Snowflake),
	MarkGuildRead(Snowflake),
	Accept(Snowflake),
	Ignore(Snowflake),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InboxShown {
	pub mentions: Option<Vec<MentionGroup>>,
	pub unreads: Vec<UnreadGroup>,
	pub for_you: Vec<ForYouEntry>,
	pub avatars: Vec<(Snowflake, ImageName)>,
	pub icons: Vec<(Snowflake, ImageName)>,
}

impl InboxShown {
	fn avatar(&self, user_id: Snowflake) -> Option<&ImageName> {
		self.avatars
			.iter()
			.find(|(owner, _)| *owner == user_id)
			.map(|(_, name)| name)
	}

	fn icon(&self, guild_id: Option<Snowflake>) -> Option<&ImageName> {
		let guild_id = guild_id?;
		self.icons
			.iter()
			.find(|(owner, _)| *owner == guild_id)
			.map(|(_, name)| name)
	}
}

pub struct InboxLayer {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	window: Signal<(f32, f32)>,
	anchor: Signal<Option<f32>>,
	tab: Signal<InboxTab>,
	filter: Signal<Filter>,
	filter_open: Signal<bool>,
	shown: Signal<Rc<InboxShown>>,
	images: RefCell<Vec<ImageName>>,
	requests: Hook<Request>,
	images_changed: Hook<()>,
}

impl InboxLayer {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		Rc::new(Self {
			runtime: runtime.clone(),
			pictures,
			window: runtime.signal((0., 0.)),
			anchor: runtime.signal(None),
			tab: runtime.signal(InboxTab::default()),
			filter: runtime.signal(Filter::default()),
			filter_open: runtime.signal(false),
			shown: runtime.signal(Rc::default()),
			images: RefCell::default(),
			requests: RefCell::new(None),
			images_changed: RefCell::new(None),
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

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
	}

	pub fn is_open(&self) -> bool {
		self.anchor.with_untracked(Option::is_some)
	}

	pub fn tab(&self) -> InboxTab {
		self.tab.with_untracked(|tab| *tab)
	}

	pub fn filter(&self) -> Filter {
		self.filter.with_untracked(|filter| *filter)
	}

	pub fn open(&self, at: (f32, f32)) {
		self.filter_open.set(false);
		self.anchor.set(Some(at.0));
		self.request(Request::Load(self.tab(), self.filter()));
	}

	pub fn toggle(&self, at: (f32, f32)) {
		if self.is_open() {
			self.close();
		} else {
			self.open(at);
		}
	}

	pub fn close(&self) {
		if self.is_open() {
			self.filter_open.set(false);
			self.anchor.set(None);
			self.set_images(Vec::new());
		}
	}

	pub fn escape(&self) -> bool {
		if self.filter_open.with_untracked(|open| *open) {
			self.filter_open.set(false);
			return true;
		}
		if self.is_open() {
			self.close();
			return true;
		}
		false
	}

	pub fn select_tab(&self, tab: InboxTab) {
		self.filter_open.set(false);
		if self.tab() != tab {
			self.tab.set(tab);
		}
		self.request(Request::Load(tab, self.filter()));
	}

	fn set_filter(&self, filter: Filter) {
		if self.filter() == filter {
			return;
		}
		self.filter.set(filter);
		self.request(Request::Load(InboxTab::Mentions, filter));
	}

	pub fn show(&self, shown: InboxShown) {
		if self.shown.with_untracked(|current| **current == shown) {
			return;
		}
		let images = shown
			.avatars
			.iter()
			.chain(shown.icons.iter())
			.map(|(_, name)| name.clone())
			.collect();
		self.shown.set(Rc::new(shown));
		self.set_images(images);
	}

	pub fn card_avatar(&self, user: &protocol::User) -> Option<ImageName> {
		self.pictures.avatar(user, CARD_AVATAR)
	}

	pub fn guild_icon(&self, guild_id: Snowflake, hash: &str) -> ImageName {
		self.pictures.guild_icon(guild_id, hash, GUILD_ICON)
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

	fn place(anchor: f32, width: f32, window: (f32, f32)) -> f32 {
		(anchor + ANCHOR_RIGHT - width)
			.min(window.0 - width - SNAP_MARGIN)
			.max(SNAP_MARGIN)
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (anchor, window) = (self.anchor.clone(), self.window.clone());
		let outside = self.clone();
		div().absolute().left(0.).top(0.).w(0.).h(0.).child(
			div()
				.absolute()
				.left(0.)
				.top(0.)
				.reactive_style(move || {
					let (width, height) = if anchor.with(Option::is_some) {
						window.get()
					} else {
						(0., 0.)
					};
					Styles::new().w(width).h(height)
				})
				.on_event(move |event| {
					if event.phase == EventPhase::Target
						&& matches!(event.event, InputEvent::PointerDown { .. })
					{
						outside.close();
						event.stop_propagation();
					}
				})
				.child(self.popout_view()),
		)
	}

	fn popout_view(self: &Rc<Self>) -> View {
		let anchor = self.anchor.clone();
		let this = self.clone();
		div().child_when(
			move || anchor.get(),
			move |at: f32, _| {
				let window = this.window.clone();
				let width = parts::size(&this.runtime, "inbox_popout_width");
				let top = parts::size(&this.runtime, "header_popout_top");
				panel::popout(&this)
					.id("inbox-popout")
					.absolute()
					.w(width)
					.h(popout_height(&this.runtime, window.clone(), top))
					.top(top)
					.reactive_style(move || Styles::new().left(InboxLayer::place(at, width, window.get())))
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

fn popout_height(runtime: &Runtime, window: Signal<(f32, f32)>, top: f32) -> f32 {
	let height = parts::size(runtime, "inbox_popout_height");
	let room = window.with_untracked(|window| window.1) - top - SNAP_MARGIN;
	if room > 0. { height.min(room) } else { height }
}
