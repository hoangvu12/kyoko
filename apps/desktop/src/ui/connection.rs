use std::rc::Rc;
use std::time::Instant;

use bridge::{Connection, Tone};
use session::Command;
use state::{Changes, State};
use zgx_script::{ScriptHost, Value};

use super::{Link, record};

pub(super) fn empty_banner() -> Value {
	record([
		("shown", false.into()),
		("state", "connecting".into()),
		("text", "".into()),
		("tone", "warning".into()),
		("retry", false.into()),
	])
}

impl Link {
	pub(super) fn install_connection(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("session.reconnect", move |_| {
			if let Some(link) = actions.upgrade() {
				link.send(Command::Reconnect);
			}
		});
	}

	pub(super) fn connection_apply(&self, state: &State, changes: &Changes) {
		if !changes.session || self.connection_pinned.get() {
			return;
		}
		let changed = self
			.connection
			.borrow_mut()
			.observe(&state.session().connection, Instant::now());
		if changed {
			self.publish_connection();
			self.connection_wakes.push(());
		}
	}

	pub(super) fn reset_connection(&self) {
		self.connection_pinned.set(false);
		*self.connection.borrow_mut() = bridge::ConnectionWatch::default();
		self.toasts.forget_rejections();
		self.publish_connection();
	}

	fn publish_connection(&self) {
		let watch = self.connection.borrow();
		let current = watch.current();
		let value = match current.banner(Instant::now()) {
			Some(banner) => record([
				("shown", true.into()),
				("state", current.name().into()),
				("text", banner.text.as_str().into()),
				(
					"tone",
					match banner.tone {
						Tone::Warning => "warning",
						Tone::Danger => "danger",
					}
					.into(),
				),
				("retry", banner.retry.into()),
			]),
			None => record([
				("shown", false.into()),
				("state", current.name().into()),
				("text", "".into()),
				("tone", "warning".into()),
				("retry", false.into()),
			]),
		};
		self.connection_banner.set(value);
	}

	pub(super) async fn follow_connection(self: Rc<Self>) {
		loop {
			let next = self.connection.borrow().current().next_change(Instant::now());
			match next {
				Some(at) => zgx::timer::sleep(at.saturating_duration_since(Instant::now())).await,
				None => {
					if self.connection_wakes.next().await.is_none() {
						return;
					}
				}
			}
			self.publish_connection();
		}
	}

	pub(super) fn drive_disconnected(&self) {
		self.connection_pinned.set(true);
		*self.connection.borrow_mut() =
			bridge::ConnectionWatch::showing(Connection::Reconnecting { retry_at: None });
		self.publish_connection();
	}
}
