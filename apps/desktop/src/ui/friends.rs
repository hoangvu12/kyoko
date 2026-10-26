use std::rc::Rc;

use protocol::Snowflake;
use session::{Command, FriendSource};
use state::{Changes, State};

use super::Link;
use crate::capture;
use crate::friend_dialog::FriendConfirm;
use crate::profile::Anchor;
use crate::rail::Icons;

impl Link {
	pub(super) fn friends_apply(&self, state: &State, changes: &Changes) {
		if self.friends.apply(state, changes) {
			self.publish_friends(state);
		}
	}

	pub(super) fn publish_friends(&self, state: &State) {
		let icons = Icons {
			gallery: &self.images.gallery,
			cdn: &self.images.cdn,
			scale: self.scale_factor(),
		};
		if self.friends.publish(state, &icons) {
			self.refresh_visible();
		}
	}

	fn republish_friends(&self) {
		if let Some(shared) = self.state() {
			let state = shared.read();
			self.window.runtime.batch(|| self.publish_friends(&state));
		}
	}

	pub(crate) fn select_friends_tab(&self, key: &str) {
		if self.friends.select_tab(key) {
			self.republish_friends();
		}
	}

	pub(crate) fn filter_friends(&self, filter: &str) {
		if self.friends.set_filter(filter) {
			self.republish_friends();
		}
	}

	pub(crate) fn add_friend(&self, username: &str) {
		let baseline = self
			.state()
			.and_then(|shared| shared.read().last_rejection().map(|rejection| rejection.id))
			.unwrap_or(0);
		if self.friends.start_adding(username, baseline) {
			self.send(Command::AddFriend {
				username: username.trim().to_owned(),
				from: FriendSource::AddFriend,
			});
		}
	}

	pub(crate) fn accept_friend(&self, user_id: Snowflake) {
		self.send(Command::AcceptFriend { user_id });
	}

	pub(crate) fn remove_relationship(&self, user_id: Snowflake) {
		self.send(Command::RemoveRelationship { user_id });
	}

	pub(super) fn install_friend_dialog(link: &Rc<Link>) {
		let sender = Rc::downgrade(link);
		link.friend_dialog.on_command(move |command| {
			if let Some(link) = sender.upgrade() {
				link.send(command);
			}
		});
	}

	pub(crate) fn ask_remove_friend(&self, user_id: Snowflake) {
		let name = self.person_name(user_id);
		self.friend_dialog.ask(FriendConfirm::Remove { user_id, name });
	}

	pub(crate) fn ask_block(&self, user_id: Snowflake) {
		let name = self.person_name(user_id);
		let friend = self
			.state()
			.is_some_and(|shared| shared.read().friends().iter().any(|row| row.user_id == user_id));
		self.friend_dialog.ask(FriendConfirm::Block {
			user_id,
			name,
			friend,
		});
	}

	fn person_name(&self, user_id: Snowflake) -> String {
		self.state()
			.map(|shared| shared.read().display_name(None, user_id))
			.unwrap_or_else(|| user_id.to_string())
	}

	pub(crate) fn message_friend(&self, user_id: Snowflake) {
		self.message_user(user_id);
	}

	pub(crate) fn copy_user_id(&self, user_id: Snowflake) {
		if let Err(error) = platform::clipboard::write_text(&user_id.to_string()) {
			tracing::warn!(%error, "the user id was not copied");
		}
	}

	pub(crate) fn open_friend_profile(&self, user_id: Snowflake, at: (f32, f32)) {
		self.open_profile(user_id, at, Anchor::TopLeft);
	}

	pub(super) fn friends_images(&self) -> Vec<String> {
		self.friends.images()
	}

	pub(super) fn drive_friends(&self, screen: capture::Screen) {
		let capture::Screen::Friends(tab) = screen else {
			return;
		};
		self.select_home();
		self.show_home_page("friends");
		self.select_friends_tab(tab.key());
	}

	pub(super) async fn drive_friend_confirm(&self, shot: capture::FriendShot) {
		let friend = self
			.state()
			.and_then(|shared| shared.read().friends().first().map(|row| row.user_id));
		let Some(user_id) = friend else {
			tracing::warn!("no friend to confirm about");
			return;
		};
		match shot {
			capture::FriendShot::Remove => self.ask_remove_friend(user_id),
			capture::FriendShot::Block => self.ask_block(user_id),
		}
		zgx::timer::sleep(capture::SETTLE).await;
	}

	pub(super) fn friends_settled(&self) -> bool {
		let shown = self.shown.borrow();
		self.dms.page() == "friends" && self.friends.images().iter().all(|name| shown.contains(name))
	}
}
