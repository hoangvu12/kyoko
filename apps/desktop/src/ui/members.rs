use std::rc::Rc;
use std::time::Instant;

use bridge::{MemberListEntry, OpenChannel, OpenMembers};

use super::Link;
use crate::capture;
use crate::rail::Icons;

impl Link {
	pub(super) async fn flush_members(self: Rc<Self>) {
		while self.members.wakes().next().await.is_some() {
			loop {
				let due = self.members.due();
				let (Some(due), true) = (due, self.state().is_some()) else {
					break;
				};
				let now = Instant::now();
				if due > now {
					zgx::timer::sleep(due - now).await;
				} else {
					self.publish_members();
				}
			}
		}
	}

	fn publish_members(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		let state = shared.read();
		let Some(change) = self.members.flush(&state) else {
			return;
		};
		let icons = Icons {
			gallery: &self.images.gallery,
			cdn: &self.images.cdn,
			scale: self.scale_factor(),
		};
		let names = self
			.window
			.runtime
			.batch(|| self.members.publish(change, &state, &icons));
		if *self.member_avatars.borrow() != names {
			*self.member_avatars.borrow_mut() = names;
			self.refresh_visible();
		}
	}

	pub(super) fn show_members(&self, open: Option<OpenChannel>) {
		let open = open.and_then(|open| {
			open.guild_id.map(|guild_id| OpenMembers {
				guild_id,
				channel_id: open.channel_id,
			})
		});
		self.members.show(open);
	}

	pub(super) async fn until_members(&self, capture: &capture::Capture) {
		let started = Instant::now();
		let mut scrolled = capture.screen != capture::Screen::MemberListScrolled;
		loop {
			let settled = self.members_settled() && self.settled(capture.screen);
			if settled && scrolled {
				break;
			}
			if settled {
				self.members.scroll_to_end();
				scrolled = true;
				zgx::timer::sleep(capture::SETTLE).await;
				continue;
			}
			if started.elapsed() >= capture::LIMIT {
				tracing::warn!(screen = %capture.name, "the member list did not settle in time");
				break;
			}
			zgx::timer::sleep(capture::POLL).await;
		}
	}

	fn members_settled(&self) -> bool {
		let panel = self.members.panel();
		panel.open().is_some()
			&& panel.members().next().is_some()
			&& !panel
				.entries()
				.iter()
				.any(|entry| matches!(entry, MemberListEntry::Placeholder))
	}
}
