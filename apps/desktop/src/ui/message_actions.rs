use std::rc::Rc;

use bridge::{Access, DeleteProgress, EmojiPicker, MessageAction, ReactionsList};
use chrono::Utc;
use state::{Changes, State};
use zgx::input::{InputEvent, PointerButton};
use zgx::widgets::WeakUi;

use super::{Link, Slot};
use crate::capture::{self, OwnShot};
use crate::timeline::Compose;

impl Link {
	pub(super) fn install_message_actions(link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		link.timeline_view.on_compose(move |compose| {
			if let Some(link) = actions.upgrade() {
				match compose {
					Compose::Reply(message_id) => link.composer_reply(message_id),
					Compose::Edit(message_id) => link.composer_edit(message_id),
				}
			}
		});
		let actions = Rc::downgrade(link);
		link.timeline_view.on_emoji_catalog(move || {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.timeline.borrow().open()?.guild_id;
			Some(EmojiPicker::of(&shared.read(), guild_id))
		});
		let actions = Rc::downgrade(link);
		link.timeline_view.on_expression_details(move |subject| {
			let shared = actions.upgrade()?.state()?;
			Some(bridge::expression_details(&shared.read(), subject))
		});
		let actions = Rc::downgrade(link);
		link.timeline_view.on_reaction_names(move |message_id, chip| {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.timeline.borrow().open()?.guild_id;
			bridge::tip_names(&shared.read(), guild_id, message_id, chip)
		});
		let actions = Rc::downgrade(link);
		link.timeline_view.on_reaction_lists(move |request| {
			let link = actions.upgrade()?;
			let shared = link.state()?;
			let guild_id = link.timeline.borrow().open()?.guild_id;
			ReactionsList::of(&shared.read(), guild_id, request)
		});
	}

	pub(super) fn place_actions(&self, slot: Slot) {
		self.timeline_view.set_origin(slot.x, slot.y);
		let (width, height) = self.window.info.bounds.with_untracked(|bounds| bounds.size);
		let scale = self.scale_factor() as f32;
		if width > 0 && height > 0 && scale > 0. {
			self.timeline_view
				.set_window(width as f32 / scale, height as f32 / scale);
		}
	}

	pub(super) fn attach_ui(&self, ui: WeakUi) {
		*self.ui.borrow_mut() = Some(ui);
	}

	pub(super) fn publish_actions(&self, state: &State, changes: &Changes) {
		let open = self.timeline.borrow().open();
		let access = open
			.map(|open| Access::of(state, open.guild_id, open.channel_id, Utc::now()))
			.unwrap_or_default();
		self.timeline_view.set_access(access);
		self.timeline_view.set_nitro(state.has_nitro());
		if !changes.loaded.is_empty() || !changes.messages.is_empty() {
			self.timeline_view.reactions_changed();
		}
		if changes.favorite_gifs {
			self.timeline_view
				.set_favorite_gifs(state.favorite_gifs().iter().map(|gif| gif.url.clone()).collect());
		}
		if changes.deletions
			&& let Some(open) = open
		{
			self.timeline_view
				.set_progress(DeleteProgress::of(state, open.channel_id));
		}
		if changes.rejected
			&& let Some(rejection) = state.last_rejection()
		{
			self.timeline_view.rejected(&rejection.reason);
		}
	}

	pub(super) async fn drive_own_message(&self, shot: OwnShot) {
		let Some(channel_id) = self.timeline.borrow().open().map(|open| open.channel_id) else {
			tracing::warn!("no open channel to send the message in");
			return;
		};
		self.say(channel_id, capture::OWN_MESSAGE.to_owned()).await;
		self.until_capture(|link| link.timeline_view.settled()).await;
		zgx::timer::sleep(capture::SETTLE).await;
		match shot {
			OwnShot::Menu => self.point_at_last_message(true).await,
			OwnShot::DeleteDialog => {
				let newest = self
					.timeline
					.borrow()
					.messages()
					.rev()
					.find(|entry| entry.mine && !entry.pending)
					.cloned();
				match newest {
					Some(entry) => self.timeline_view.run_action(MessageAction::Delete, &entry),
					None => tracing::warn!("no message of yours to delete"),
				}
			}
		}
	}

	pub(super) async fn point_at_last_message(&self, open_menu: bool) {
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			tracing::warn!("the window is gone");
			return;
		};
		let (x, y) = capture::LAST_MESSAGE;
		ui.dispatch(InputEvent::PointerMove { x, y });
		if open_menu {
			zgx::timer::sleep(capture::POLL).await;
			ui.dispatch(InputEvent::PointerDown {
				x,
				y,
				button: PointerButton::Secondary,
			});
			zgx::timer::sleep(capture::POLL).await;
			ui.dispatch(InputEvent::PointerUp {
				x,
				y,
				button: PointerButton::Secondary,
			});
		}
	}
}
