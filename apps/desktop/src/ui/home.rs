use bridge::OpenChannel;
use protocol::Snowflake;
use session::Command;
use state::State;

use super::{Link, Slot};
use crate::capture;
use crate::rail::Icons;

impl Link {
	pub(crate) fn select_home(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		self.select_home_only(&shared.read());
		self.leave_home_page();
		match self.dms.home_target() {
			Some(channel_id) => self.select_dm(channel_id),
			None => {
				let state = shared.read();
				self.window.runtime.batch(|| self.publish_timeline(&state));
			}
		}
	}

	pub(crate) fn select_dm(&self, channel_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		if self.dms.list().entry(channel_id).is_none() {
			return;
		}
		if !self.sidebar.sidebar().is_home() {
			self.select_home_only(&shared.read());
		}
		self.leave_home_page();
		self.dms.remember(channel_id);
		let command = {
			let state = shared.read();
			let command = self.timeline.borrow_mut().select(&state, None, channel_id);
			self.window.runtime.batch(|| self.publish_timeline(&state));
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	pub(crate) fn close_dm(&self, channel_id: Snowflake) {
		if self.dms.list().entry(channel_id).is_some() {
			self.send(Command::CloseDm { channel_id });
		}
	}

	pub(crate) fn show_home_page(&self, page: &str) {
		if page.is_empty() {
			self.leave_home_page();
		} else if self.sidebar.sidebar().is_home() && self.dms.set_page(page) {
			self.window.slot.set(Slot::default());
		}
	}

	pub(super) fn place_slot(&self, slot: Slot) {
		self.reported_slot.set(slot);
		self.timeline_view.resize(slot.width, slot.height);
		if self.dms.page().is_empty() {
			self.window.slot.set(slot);
		}
	}

	pub(super) fn publish_dms(&self, state: &State) {
		let icons = Icons {
			gallery: &self.images.gallery,
			cdn: &self.images.cdn,
			scale: self.scale_factor(),
		};
		let names = self.dms.publish(state, &icons);
		if *self.dm_avatars.borrow() != names {
			*self.dm_avatars.borrow_mut() = names;
			self.refresh_visible();
		}
		let open = self.timeline.borrow().open();
		if let Some(open) = open.filter(|open| open.guild_id.is_none()) {
			self.channel_name
				.set(self.channel_title(state, open).as_str().into());
		}
	}

	pub(super) fn channel_title(&self, state: &State, open: OpenChannel) -> String {
		if open.guild_id.is_none()
			&& let Some(title) = self.dms.title(open.channel_id)
		{
			return title;
		}
		state
			.channel(open.channel_id)
			.and_then(|channel| channel.name.clone())
			.unwrap_or_default()
	}

	pub(super) fn drive_home(&self, screen: capture::Screen) {
		self.select_home();
		if screen == capture::Screen::DmGroup {
			let group = self
				.dms
				.list()
				.entries()
				.iter()
				.find(|entry| entry.is_group())
				.map(|entry| entry.id);
			if let Some(group) = group {
				self.select_dm(group);
			}
		}
	}

	pub(super) fn home_settled(&self) -> bool {
		self.timeline
			.borrow()
			.open()
			.is_some_and(|open| open.guild_id.is_none())
	}

	fn select_home_only(&self, state: &State) {
		if self.sidebar.show_home(state) {
			self.timeline.borrow_mut().close();
			self.window.runtime.batch(|| self.publish_selection(state));
		}
	}

	pub(super) fn leave_home_page(&self) {
		if self.dms.set_page("") {
			self.window.slot.set(self.reported_slot.get());
		}
	}
}
