use std::rc::Rc;
use std::time::Instant;

use bridge::{MemberListEntry, ModerationItem, OpenChannel, OpenMembers, moderation_items};
use chrono::Utc;
use protocol::Snowflake;
use zgx::input::{InputEvent, PointerButton};
use zgx::scene::{NodeKind, Rect};
use zgx::widgets::WeakUi;

use super::Link;
use crate::capture;
use crate::profile::Anchor;
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

	pub(crate) fn hover_member_avatar(&self, name: &str, inside: bool) {
		if self.members.hover_avatar(name, inside) {
			self.images.gallery.set_hovered(inside.then_some(name));
			self.refresh_visible();
		}
	}

	pub(crate) fn open_member_profile(&self, user_id: Snowflake, at: (f32, f32)) {
		self.open_profile(user_id, at, Anchor::TopRight);
	}

	fn member_with_roles(&self) -> Option<String> {
		let shared = self.state()?;
		let state = shared.read();
		let guild_id = self.open_guild()?;
		let panel = self.members.panel();
		let found = panel.members().find(|member| {
			moderation_items(&state, guild_id, member.user_id, Utc::now())
				.iter()
				.any(|item| matches!(item, ModerationItem::Roles(_)))
		});
		found.map(|member| member.name.clone())
	}

	pub(super) fn rightmost_text(&self, wanted: &str) -> Option<Rect> {
		let ui = self.ui.borrow().as_ref().and_then(WeakUi::upgrade)?;
		let scene = ui.scene.borrow();
		let mut stack = vec![scene.root()];
		let mut found: Option<Rect> = None;
		while let Some(node) = stack.pop() {
			if let NodeKind::Text { text, .. } = scene.kind(node)
				&& text.as_ref() == wanted
			{
				let bounds = scene.bounds(node);
				if found.is_none_or(|best| bounds.x > best.x) {
					found = Some(bounds);
				}
			}
			stack.extend(scene.children(node).iter().copied());
		}
		found
	}

	pub(super) async fn drive_roles_submenu(&self, capture: &capture::Capture) {
		self.until_members(capture).await;
		let Some(name) = self.member_with_roles() else {
			tracing::warn!("no member whose roles the current user can change");
			return;
		};
		let Some(row) = self.rightmost_text(&name) else {
			tracing::warn!("the member row is not on screen");
			return;
		};
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			return;
		};
		let (x, y) = (row.x + row.width / 2., row.y + row.height / 2.);
		let button = PointerButton::Secondary;
		ui.dispatch(InputEvent::PointerMove { x, y });
		zgx::timer::sleep(capture::POLL).await;
		ui.dispatch(InputEvent::PointerDown { x, y, button });
		ui.dispatch(InputEvent::PointerUp { x, y, button });
		self.until_capture(|link| link.moderation.roles_trigger().is_some())
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		let Some((x, y)) = self.moderation.roles_trigger() else {
			tracing::warn!("the member menu has no Roles item");
			return;
		};
		ui.dispatch(InputEvent::PointerMove { x, y });
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
