use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bridge::LinkAction;
use store::Store;
use zgx::compose::prelude::*;
use zgx::platform::PlatformService;
use zgx::reactive::{Runtime, Signal};

use super::rows::tok;

const DIALOG_WIDTH: f32 = 460.;

#[derive(Clone, PartialEq)]
struct Pending {
	url: String,
	host: String,
}

pub struct Links {
	pending: Signal<Option<Pending>>,
	open: Signal<bool>,
	trusted: RefCell<Vec<String>>,
	platform: RefCell<Option<Rc<PlatformService>>>,
	store: Option<Arc<Store>>,
}

impl Links {
	pub fn new(runtime: &Runtime, store: Option<Arc<Store>>) -> Self {
		let trusted = store
			.as_ref()
			.map(|store| store.settings.read().trusted_link_hosts.clone())
			.unwrap_or_default();
		Self {
			pending: runtime.signal(None),
			open: runtime.signal(false),
			trusted: RefCell::new(trusted),
			platform: RefCell::new(None),
			store,
		}
	}

	pub fn attach(&self, platform: Option<Rc<PlatformService>>) {
		if platform.is_some() {
			*self.platform.borrow_mut() = platform;
		}
	}

	pub fn open(&self, url: &str) {
		match bridge::link_action(url, &self.trusted.borrow()) {
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
		let pending = self.pending.clone();
		let host = self.pending.clone();
		let (cancel, trust, open) = (self.clone(), self.clone(), self.clone());
		modal("Leave Kyoko?", self.open.clone())
			.w(DIALOG_WIDTH)
			.p(16.)
			.gap(12.)
			.rounded(tok("radius_md"))
			.bg(tok("background_primary"))
			.border(1.)
			.border_color(tok("divider"))
			.child(
				text("Leave Kyoko?")
					.text_size(tok("font_lg"))
					.font_weight(700)
					.text_color(tok("header_primary")),
			)
			.child(
				column()
					.gap(8.)
					.text_size(tok("font_sm"))
					.text_color(tok("text_normal"))
					.child(text("This link goes to an external site:"))
					.child(
						div()
							.p(8.)
							.rounded(tok("radius_sm"))
							.bg(tok("background_tertiary"))
							.text_color(tok("link"))
							.child(
								text_signal(move || {
									pending.with(|pending| {
										pending
											.as_ref()
											.map(|pending| pending.url.clone())
											.unwrap_or_default()
									})
								})
								.text_wrap(true),
							),
					),
			)
			.child(
				row()
					.justify_end()
					.gap(8.)
					.child(dialog_button(text("Cancel"), "button_secondary", move || {
						cancel.close()
					}))
					.child(dialog_button(
						text_signal(move || {
							let host = host.with(|pending| pending.as_ref().map(|p| p.host.clone()));
							format!("Always trust {}", host.unwrap_or_default())
						}),
						"button_secondary",
						move || trust.trust(),
					))
					.child(dialog_button(text("Open link"), "brand", move || open.confirm())),
			)
	}
}

fn dialog_button(label: View, background: &str, action: impl FnMut() + 'static) -> View {
	button()
		.h(32.)
		.px(16.)
		.rounded(tok("radius_sm"))
		.bg(tok(background))
		.text_size(tok("font_sm"))
		.font_weight(500)
		.text_color(tok("text_on_brand"))
		.cursor(Cursor::Pointer)
		.child(label)
		.on_click(action)
}
