use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use bridge::LinkAction;
use store::Store;
use zgx::compose::prelude::*;
use zgx::platform::PlatformService;
use zgx::reactive::{Runtime, Signal};

use super::rows::tok;
use crate::kit::{self, Variant};
use crate::motion;

#[derive(Clone, PartialEq)]
struct Pending {
	url: String,
	host: String,
}

pub struct Links {
	pending: Signal<Option<Pending>>,
	open: Signal<bool>,
	trusted: RefCell<Vec<String>>,
	warn: Cell<bool>,
	platform: RefCell<Option<Rc<PlatformService>>>,
	store: Option<Arc<Store>>,
}

impl Links {
	pub fn new(runtime: &Runtime, store: Option<Arc<Store>>) -> Self {
		let (trusted, warn) = store
			.as_ref()
			.map(|store| {
				let settings = store.settings.read();
				(settings.trusted_link_hosts.clone(), settings.chat.link_warning)
			})
			.unwrap_or((Vec::new(), true));
		Self {
			pending: runtime.signal(None),
			open: runtime.signal(false),
			trusted: RefCell::new(trusted),
			warn: Cell::new(warn),
			platform: RefCell::new(None),
			store,
		}
	}

	pub fn set_trusted(&self, hosts: &[String], warn: bool) {
		*self.trusted.borrow_mut() = hosts.to_vec();
		self.warn.set(warn);
	}

	pub fn attach(&self, platform: Option<Rc<PlatformService>>) {
		if platform.is_some() {
			*self.platform.borrow_mut() = platform;
		}
	}

	pub fn open(&self, url: &str) {
		match bridge::link_action(url, &self.trusted.borrow()).warned(self.warn.get()) {
			LinkAction::Open => self.launch(url),
			LinkAction::Confirm { host } => {
				self.pending.set(Some(Pending {
					url: url.to_owned(),
					host,
				}));
				self.open.set(true);
			}
			LinkAction::Ignore => {}
		}
	}

	pub fn download(&self, url: &str) {
		self.launch(url);
	}

	fn launch(&self, url: &str) {
		let opened = self
			.platform
			.borrow()
			.as_ref()
			.is_some_and(|platform| platform.0.open_url(url));
		if !opened {
			tracing::warn!("could not open the link");
		}
	}

	fn close(&self) {
		self.open.set(false);
	}

	fn confirm(&self) {
		if let Some(pending) = self.pending.with_untracked(Clone::clone) {
			self.launch(&pending.url);
		}
		self.close();
	}

	fn trust(&self) {
		let Some(pending) = self.pending.with_untracked(Clone::clone) else {
			return;
		};
		let host = pending.host.clone();
		if !self.trusted.borrow().contains(&host) {
			self.trusted.borrow_mut().push(host.clone());
		}
		if let Some(store) = &self.store {
			let saved = store.update_settings(|settings| {
				if !settings.trusted_link_hosts.contains(&host) {
					settings.trusted_link_hosts.push(host.clone());
				}
			});
			if let Err(error) = saved {
				tracing::warn!(%error, "could not save the trusted link host");
			}
		}
		self.confirm();
	}

	pub fn view(self: &Rc<Self>) -> View {
		let (shown, open) = (self.pending.clone(), self.open.clone());
		let this = self.clone();
		kit::dialog(motion::modal("Leave Kyoko?", self.open.clone()))
			.w(tok("modal_width_sm"))
			.child_when(
				move || if open.get() { shown.get() } else { None },
				move |pending: Pending, _| this.content(pending),
			)
	}

	fn content(self: &Rc<Self>, pending: Pending) -> View {
		let (close, cancel, trust, open) = (self.clone(), self.clone(), self.clone(), self.clone());
		column()
			.w_full()
			.child(kit::header(kit::title("Leave Kyoko?"), None, move || {
				close.close()
			}))
			.child(
				kit::body()
					.child(kit::paragraph("This link goes to an external site:"))
					.child(
						div()
							.w_full()
							.px(tok("space_12"))
							.py(tok("space_10"))
							.rounded(tok("radius_sm"))
							.bg(tok("bg_3"))
							.border(tok("border_thickness"))
							.border_color(tok("border"))
							.child(
								text(pending.url)
									.w_full()
									.text_wrap(true)
									.text_size(tok("font_size_md"))
									.line_height(tok("line_height_md"))
									.text_color(tok("accent_1")),
							),
					)
					.child(kit::footer(vec![
						kit::action("Cancel", Variant::Link, move || cancel.close()),
						kit::action(
							format!("Always trust {}", pending.host),
							Variant::Secondary,
							move || trust.trust(),
						),
						kit::action("Open link", Variant::Primary, move || open.confirm()),
					])),
			)
	}
}
