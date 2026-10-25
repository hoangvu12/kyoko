mod add_server;
mod forward;
mod invite;
mod leave;
mod notifications;
mod onboarding;
mod parts;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use media::ImageName;
use protocol::Snowflake;
use session::Command;
use state::{SharedState, State};
use zgx::compose::prelude::*;
use zgx::compose::{ModalPlacement, PlacementEdge};
use zgx::reactive::{Effect, Runtime, Signal};

use crate::timeline::Pictures;

const TOP: f32 = 80.;
const WIDTH: f32 = 440.;
const WIDE: f32 = 540.;

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;

#[derive(Debug)]
pub enum Request {
	Command(Command),
	Copy(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
	AddServer,
	Forward {
		channel_id: Snowflake,
		message_id: Snowflake,
		preview: String,
	},
	Notifications(Snowflake),
	Invite(Snowflake),
	Leave {
		guild_id: Snowflake,
		name: String,
	},
	Onboarding(Snowflake),
	Screening(Snowflake),
}

pub struct CommunityLayer {
	runtime: Runtime,
	pictures: Rc<Pictures>,
	state: RefCell<Option<SharedState>>,
	revision: Signal<u64>,
	open: Signal<bool>,
	typing: Signal<bool>,
	plain: Signal<bool>,
	dialog: Signal<Option<(u64, Dialog)>>,
	serials: Cell<u64>,
	invite: invite::Waiting,
	images: RefCell<Vec<ImageName>>,
	requests: Hook<Request>,
	images_changed: Hook<()>,
	watchers: RefCell<Vec<Effect>>,
}

impl CommunityLayer {
	pub fn new(runtime: &Runtime, pictures: Rc<Pictures>) -> Rc<Self> {
		let layer = Rc::new(Self {
			runtime: runtime.clone(),
			pictures,
			state: RefCell::new(None),
			revision: runtime.signal(0),
			open: runtime.signal(false),
			typing: runtime.signal(false),
			plain: runtime.signal(false),
			dialog: runtime.signal(None),
			serials: Cell::new(0),
			invite: invite::Waiting::new(runtime),
			images: RefCell::default(),
			requests: RefCell::new(None),
			images_changed: RefCell::new(None),
			watchers: RefCell::default(),
		});
		let weak = Rc::downgrade(&layer);
		let (typing, plain) = (layer.typing.clone(), layer.plain.clone());
		let dismissed = runtime.effect(move || {
			let shown = typing.get() || plain.get();
			if !shown && let Some(layer) = weak.upgrade() {
				layer.runtime.untracked(|| {
					if layer.is_open() {
						layer.open.set(false);
					}
				});
			}
		});
		layer.watchers.borrow_mut().push(dismissed);
		let weak = Rc::downgrade(&layer);
		let open = layer.open.clone();
		let closed = runtime.effect(move || {
			if !open.get()
				&& let Some(layer) = weak.upgrade()
			{
				layer.runtime.untracked(|| {
					layer.dialog.set(None);
					layer.set_images(Vec::new());
				});
			}
		});
		layer.watchers.borrow_mut().push(closed);
		layer
	}

	pub fn on_request(&self, hook: impl Fn(Request) + 'static) {
		*self.requests.borrow_mut() = Some(Box::new(hook));
	}

	pub fn on_images_changed(&self, hook: impl Fn(()) + 'static) {
		*self.images_changed.borrow_mut() = Some(Box::new(hook));
	}

	pub fn attach(&self, state: Option<SharedState>) {
		*self.state.borrow_mut() = state;
	}

	fn request(&self, request: Request) {
		if let Some(hook) = self.requests.borrow().as_ref() {
			hook(request);
		}
	}

	fn send(&self, command: Command) {
		self.request(Request::Command(command));
	}

	fn read<T>(&self, read: impl FnOnce(&State) -> T) -> Option<T> {
		let shared = self.state.borrow().clone()?;
		let state = shared.read();
		Some(read(&state))
	}

	pub fn open(&self, dialog: Dialog) {
		let serial = self.serials.get() + 1;
		self.serials.set(serial);
		self.invite.reset();
		let typing = matches!(dialog, Dialog::AddServer | Dialog::Forward { .. });
		self.runtime.batch(|| {
			self.dialog.set(Some((serial, dialog)));
			self.open.set(true);
			self.typing.set(typing);
			self.plain.set(!typing);
		});
		self.refresh();
	}

	pub fn close(&self) {
		self.runtime.batch(|| {
			self.typing.set(false);
			self.plain.set(false);
			self.open.set(false);
		});
	}

	pub fn is_open(&self) -> bool {
		self.open.with_untracked(|open| *open)
	}

	pub fn current(&self) -> Option<Dialog> {
		if !self.is_open() {
			return None;
		}
		self.dialog
			.with_untracked(|dialog| dialog.as_ref().map(|(_, dialog)| dialog.clone()))
	}

	pub fn escape(&self) -> bool {
		let open = self.is_open();
		self.close();
		open
	}

	pub fn refresh(&self) {
		if !self.is_open() {
			return;
		}
		if let Some(Dialog::Invite(channel_id)) = self.current() {
			let code = self.read(|state| state.invite_created(channel_id).map(|invite| invite.code.clone()));
			self.invite.arrived(code.flatten());
		}
		self.revision
			.set(self.revision.with_untracked(|revision| revision + 1));
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
		self.is_open()
			&& self
				.images
				.borrow()
				.iter()
				.all(|name| self.pictures.is_settled(name))
	}

	pub fn view(self: &Rc<Self>) -> View {
		div()
			.absolute()
			.left(0.)
			.top(0.)
			.w(0.)
			.h(0.)
			.child(self.modal(self.typing.clone(), true))
			.child(self.modal(self.plain.clone(), false))
	}

	fn modal(self: &Rc<Self>, open: Signal<bool>, typing: bool) -> View {
		let dialog = self.dialog.clone();
		let sized = self.dialog.clone();
		let shown = open.clone();
		let this = self.clone();
		modal("Community dialog", open)
			.modal_placement(ModalPlacement {
				x: PlacementEdge::Center,
				y: PlacementEdge::Start,
				insets: [TOP, 0., 0., 0.],
			})
			.autofocus(typing)
			.p(0.)
			.border(0.)
			.bg(parts::clear())
			.reactive_style(move || {
				let wide = sized
					.with(|dialog| matches!(dialog, Some((_, Dialog::Onboarding(_) | Dialog::Screening(_)))));
				Styles::new().w(if wide { WIDE } else { WIDTH })
			})
			.child_when(
				move || if shown.get() { dialog.get() } else { None },
				move |(_, dialog): (u64, Dialog), _| this.build(dialog),
			)
	}

	fn build(self: &Rc<Self>, dialog: Dialog) -> View {
		match dialog {
			Dialog::AddServer => add_server::view(self),
			Dialog::Forward {
				channel_id,
				message_id,
				preview,
			} => forward::view(self, channel_id, message_id, preview),
			Dialog::Notifications(guild_id) => notifications::view(self, guild_id),
			Dialog::Invite(channel_id) => invite::view(self, channel_id),
			Dialog::Leave { guild_id, name } => leave::view(self, guild_id, &name),
			Dialog::Onboarding(guild_id) => onboarding::view(self, guild_id),
			Dialog::Screening(guild_id) => onboarding::screening(self, guild_id),
		}
	}
}
