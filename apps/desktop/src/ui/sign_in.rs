use std::cell::Cell;
use std::rc::Rc;

use bridge::{Checked, Flow, Inbox, Screen, Status};
use session::Session;
use state::{Changes, SecretStorage};
use tokio::runtime::Handle;
use zgx_script::{ScriptHost, Shared};

use super::{Link, Slot, text};
use crate::capture;

pub struct SignIn {
	pub flow: Flow,
	pub runtime: Handle,
	pub storage_warnings: bool,
}

enum Event {
	Checked(Box<Checked>),
	SignedOut,
}

pub(super) struct Account {
	runtime: Handle,
	events: Inbox<Event>,
	generation: Cell<u64>,
	storage_warnings: bool,
	screen: Shared,
	acknowledged: Shared,
	checking: Shared,
	message: Shared,
	error: Shared,
	unsaved: Shared,
	storage_warning: Shared,
}

impl Account {
	pub(super) fn install(host: &ScriptHost, runtime: Handle, storage_warnings: bool) -> Account {
		Account {
			runtime,
			events: Inbox::new(),
			generation: Cell::new(0),
			storage_warnings,
			screen: host.shared("login.screen", "login"),
			acknowledged: host.shared("login.acknowledged", false),
			checking: host.shared("login.checking", false),
			message: host.shared("login.message", ""),
			error: host.shared("login.error", false),
			unsaved: host.shared("login.unsaved", false),
			storage_warning: host.shared("session.storage_warning", ""),
		}
	}
}

impl Link {
	pub(super) fn install_sign_in(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("login.acknowledge", move |_| {
			if let Some(link) = actions.upgrade() {
				link.acknowledge();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.submit", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.submit(text(&argument).unwrap_or_default().to_owned());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.continue", move |_| {
			if let Some(link) = actions.upgrade() {
				link.flow.borrow_mut().continue_unsaved();
				link.after_flow_change();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.sign_out", move |_| {
			if let Some(link) = actions.upgrade() {
				link.sign_out();
			}
		});
		link.after_flow_change();
	}

	pub(super) async fn follow_sign_in(self: Rc<Self>) {
		while let Some(events) = self.account.events.next().await {
			for event in events {
				match event {
					Event::Checked(checked) => self.flow.borrow_mut().checked(*checked),
					Event::SignedOut => self.flow.borrow_mut().signed_out(),
				}
				self.after_flow_change();
			}
		}
	}

	pub(super) fn watch_session(&self, changes: &Changes) {
		let Some(shared) = self.state() else {
			return;
		};
		if changes.session && self.account.storage_warnings {
			let warning = storage_warning(&shared.read().session().secrets);
			self.account.storage_warning.set(warning.as_str().into());
		}
		let rejected = self.flow.borrow_mut().observe(&shared.read());
		if let Some(session) = rejected {
			self.release_media();
			self.account.runtime.spawn_blocking(move || session.shutdown());
			self.after_flow_change();
		}
	}

	pub(super) fn drive_capture(&self, screen: capture::Screen) {
		if screen == capture::Screen::LoginToken {
			self.acknowledge();
		}
	}

	pub(super) fn login_settled(&self, screen: capture::Screen) -> bool {
		let flow = self.flow.borrow();
		matches!(flow.screen(), Screen::Login(_))
			&& (screen != capture::Screen::LoginToken || flow.gate().acknowledged())
	}

	fn acknowledge(&self) {
		if let Err(error) = self.flow.borrow().gate().acknowledge() {
			tracing::warn!(%error, "the notice acknowledgement was not saved");
		}
		self.publish_sign_in();
	}

	fn submit(&self, typed: String) {
		let pasted = if typed.trim().is_empty() {
			clipboard_text()
		} else {
			typed
		};
		let check = self.flow.borrow_mut().submit(pasted);
		if let Some(check) = check {
			let events = self.account.events.clone();
			self.account
				.runtime
				.spawn(async move { events.push(Event::Checked(Box::new(check.await))) });
		}
		self.publish_sign_in();
	}

	pub(super) fn sign_out(&self) {
		let sign_out = self.flow.borrow_mut().sign_out();
		let Some(sign_out) = sign_out else {
			return;
		};
		tracing::info!("signing out");
		self.close_settings();
		self.release_media();
		let events = self.account.events.clone();
		self.account.runtime.spawn_blocking(move || {
			sign_out.finish();
			events.push(Event::SignedOut);
		});
		self.after_flow_change();
	}

	fn release_media(&self) {
		let gallery = &self.images.gallery;
		self.luau_visible.borrow_mut().clear();
		gallery.set_visible(std::iter::empty::<&str>());
		gallery.set_budget(0);
		*self.avatar.borrow_mut() = None;
		self.avatar_requested_at.set(None);
		self.avatar_shown.set(false);
	}

	fn after_flow_change(&self) {
		let generation = self.flow.borrow().generation();
		if self.account.generation.replace(generation) != generation {
			self.session_started();
		}
		self.publish_sign_in();
	}

	fn session_started(&self) {
		let gallery = &self.images.gallery;
		gallery.set_budget(self.images.budget);
		if let Some(commander) = self.flow.borrow().session().map(Session::commander) {
			gallery.set_refresher(move |origin| commander.send(bridge::refresh_command(origin)));
		}
		self.timeline.borrow_mut().close();
		self.community.close();
		self.community.attach(self.state());
		*self.avatar.borrow_mut() = None;
		self.avatar_requested_at.set(None);
		self.avatar_shown.set(false);
		self.window.runtime.batch(|| {
			self.rail.reset();
			self.rail_icons.borrow_mut().clear();
			self.sidebar.reset();
			self.sidebar_avatars.borrow_mut().clear();
			self.dms.reset();
			self.dm_avatars.borrow_mut().clear();
			self.members.reset();
			self.member_avatars.borrow_mut().clear();
			for value in [
				&self.open_channel,
				&self.channel_name,
				&self.channel_topic,
				&self.avatar_name,
				&self.account.storage_warning,
			] {
				value.set("".into());
			}
			self.avatar_ready.set(false.into());
			self.status.set("Connecting".into());
			if let Some(shared) = self.state() {
				self.publish_timeline(&shared.read());
			}
		});
	}

	fn publish_sign_in(&self) {
		let flow = self.flow.borrow();
		let (screen, status) = match flow.screen() {
			Screen::Login(status) => ("login", Some(status)),
			Screen::Main => ("main", None),
			Screen::SigningOut => ("signing_out", None),
		};
		if screen != "main" {
			self.window.slot.set(Slot::default());
		}
		let account = &self.account;
		let message = status.and_then(Status::message).unwrap_or_default();
		self.window.runtime.batch(|| {
			account.screen.set(screen.into());
			account.acknowledged.set(flow.gate().acknowledged().into());
			account
				.checking
				.set(matches!(status, Some(Status::Checking)).into());
			account.message.set(message.as_str().into());
			account.error.set(status.is_some_and(Status::is_error).into());
			account
				.unsaved
				.set(matches!(status, Some(Status::Unsaved(_))).into());
		});
	}
}

fn clipboard_text() -> String {
	match platform::clipboard::read_text() {
		Ok(text) => text.unwrap_or_default(),
		Err(error) => {
			tracing::warn!(%error, "the clipboard could not be read");
			String::new()
		}
	}
}

fn storage_warning(secrets: &SecretStorage) -> String {
	let mut warning = if secrets.plaintext {
		"Your token is saved in plain text on this computer, because plain-text token storage is turned on."
			.to_owned()
	} else if let Some(error) = &secrets.keyring_error {
		format!(
			"Windows Credential Manager is unavailable ({error}), so you will need to sign in again next time."
		)
	} else {
		String::new()
	};
	if secrets.cache_in_memory {
		if !warning.is_empty() {
			warning.push(' ');
		}
		warning.push_str("Messages are cached in memory only until Credential Manager is available.");
	}
	warning
}
