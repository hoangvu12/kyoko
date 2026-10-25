use std::rc::Rc;
use std::time::Instant;

use bridge::{EmojiPicker, PanelUser, ProfileCard, StatusLine};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx::input::{InputEvent, PointerButton};
use zgx::widgets::WeakUi;
use zgx_script::ScriptHost;

use super::{Link, number, record};
use crate::capture;
use crate::profile::{Anchor, Opening, Request};

const PANEL_EMOJI: f64 = 14.;

impl Link {
	pub(super) fn install_profiles(host: &ScriptHost, link: &Rc<Link>) {
		let hooks = Rc::downgrade(link);
		link.timeline_view.on_profile(move |(user_id, at)| {
			if let Some(link) = hooks.upgrade() {
				link.open_profile(user_id, at, Anchor::TopLeft);
			}
		});
		let hooks = Rc::downgrade(link);
		link.profile.on_request(move |request| {
			if let Some(link) = hooks.upgrade() {
				link.profile_request(request);
			}
		});
		let hooks = Rc::downgrade(link);
		link.profile.on_images_changed(move |()| {
			if let Some(link) = hooks.upgrade() {
				link.refresh_visible();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("user.status_menu", move |argument| {
			if let (Some(link), Some(x), Some(y)) =
				(actions.upgrade(), number(&argument, "x"), number(&argument, "y"))
			{
				link.profile.open_status_menu((x, y));
			}
		});
		let window = Rc::downgrade(link);
		let bounds = link.window.info.bounds.clone();
		let effect = link.window.runtime.effect_now(move || {
			let (width, height) = bounds.get().size;
			if let Some(link) = window.upgrade() {
				let scale = link.scale_factor() as f32;
				if scale > 0. {
					link.profile
						.set_window(width as f32 / scale, height as f32 / scale);
				}
			}
		});
		*link.profile_effect.borrow_mut() = Some(effect);
	}

	pub(crate) fn open_profile(&self, user_id: Snowflake, at: (f32, f32), anchor: Anchor) {
		let guild_id = self.open_guild();
		self.send(Command::LoadProfile { user_id, guild_id });
		self.profile.open_popout(Opening {
			user_id,
			guild_id,
			at,
			anchor,
		});
		self.refresh_profile();
	}

	pub(crate) fn open_full_profile(&self, user_id: Snowflake) {
		let guild_id = self.open_guild();
		self.send(Command::LoadProfile { user_id, guild_id });
		self.profile.open_full(user_id, guild_id);
		self.refresh_profile();
	}

	fn refresh_profile(&self) {
		let (Some((user_id, guild_id)), Some(shared)) = (self.profile.target(), self.state()) else {
			return;
		};
		let card = ProfileCard::of(&shared.read(), user_id, guild_id, Utc::now());
		self.profile.show_card(card);
	}

	pub(super) fn publish_profiles(&self, state: &State, changes: &Changes) {
		if let Some((user_id, guild_id)) = self.profile.target() {
			self.profile
				.show_card(ProfileCard::of(state, user_id, guild_id, Utc::now()));
		}
		if let Some(user_id) = self.pending_dm.get()
			&& let Some(channel_id) = bridge::dm_channel(state, user_id)
		{
			self.pending_dm.set(None);
			self.open(None, channel_id);
		}
		let panel_changed = changes.current_user
			|| changes.session
			|| changes.user_settings
			|| state
				.current_user_id()
				.is_some_and(|me| changes.presence.contains(me));
		if panel_changed {
			self.publish_panel(PanelUser::of(state, Utc::now()));
		}
	}

	fn publish_panel(&self, panel: Option<PanelUser>) {
		let Some(panel) = panel else {
			self.profile.set_panel(None);
			return;
		};
		let emoji = self.panel_emoji(&panel.custom_status);
		let shown = [
			panel.name.clone(),
			panel.subtitle().to_owned(),
			panel.custom_status.glyph().unwrap_or_default().to_owned(),
			emoji.map(|name| format!("host:{name}")).unwrap_or_default(),
			panel.status.dot().as_str().to_owned(),
			panel.latency_text(),
		];
		if self.panel_shown.borrow().as_ref() != Some(&shown) {
			let [name, subtitle, glyph, image, status, latency] = &shown;
			self.panel.set(record([
				("name", name.as_str().into()),
				("subtitle", subtitle.as_str().into()),
				("emoji", glyph.as_str().into()),
				("emoji_image", image.as_str().into()),
				("status", status.as_str().into()),
				("latency", latency.as_str().into()),
			]));
			*self.panel_shown.borrow_mut() = Some(shown);
		}
		self.profile.set_panel(Some(panel));
	}

	fn panel_emoji(&self, custom: &StatusLine) -> Option<String> {
		let pictures = self.timeline_view.pictures();
		let name = custom
			.emoji_request(pictures.cdn(), PANEL_EMOJI, pictures.scale())
			.map(|request| pictures.request(request).to_string());
		if *self.panel_emoji.borrow() != name {
			*self.panel_emoji.borrow_mut() = name.clone();
			self.refresh_visible();
		}
		name
	}

	pub(super) fn profile_images(&self) -> Vec<String> {
		let mut names: Vec<String> = self
			.profile
			.images()
			.iter()
			.map(|name| name.to_string())
			.collect();
		names.extend(self.panel_emoji.borrow().iter().cloned());
		names
	}

	fn profile_request(&self, request: Request) {
		match request {
			Request::Command(command) => self.send(command),
			Request::Message(user_id) => self.message_user(user_id),
			Request::FullProfile(user_id) => self.open_full_profile(user_id),
			Request::SignOut => self.sign_out(),
			Request::EditProfile => self.open_settings("profiles"),
			Request::SwitchAccounts => self.show_accounts(),
			Request::SwitchTo(user_id) => self.switch_account(user_id),
			Request::AddAccount => self.add_account(),
			Request::RemoveAccount(user_id) => self.remove_account(user_id),
			Request::Picker => {
				let Some(shared) = self.state() else {
					return;
				};
				let model = EmojiPicker::of(&shared.read(), self.open_guild());
				self.profile.pick_emoji(model);
			}
		}
	}

	pub(super) fn message_user(&self, user_id: Snowflake) {
		let existing = self
			.state()
			.and_then(|shared| bridge::dm_channel(&shared.read(), user_id));
		match existing {
			Some(channel_id) => self.open(None, channel_id),
			None => {
				self.pending_dm.set(Some(user_id));
				self.send(Command::OpenDm { user_id });
			}
		}
	}

	pub(super) fn escape_overlay(&self) -> bool {
		self.timeline_view.close_lightbox()
			|| self.community.escape()
			|| self.profile.escape()
			|| self.threads_escape()
	}

	pub(super) async fn drive_profiles(&self, screen: capture::Screen) {
		match screen {
			capture::Screen::StatusMenu => {
				let (x, y) = capture::USER_PANEL_AVATAR;
				self.press_at(x, y).await;
			}
			capture::Screen::AccountSwitcher => {
				let (x, y) = capture::USER_PANEL_AVATAR;
				self.press_at(x, y).await;
				self.show_accounts();
			}
			capture::Screen::CustomStatus => self.profile.open_custom_status(),
			capture::Screen::ProfilePopout => {
				if let Some(author) = self.newest_other_author() {
					self.open_profile(author, capture::PROFILE_ANCHOR, Anchor::TopLeft);
				}
			}
			capture::Screen::FullProfile => {
				if let Some(author) = self.newest_other_author() {
					self.open_full_profile(author);
				}
			}
			_ => return,
		}
		let started = Instant::now();
		while !self.profiles_settled(screen) {
			if started.elapsed() >= capture::LIMIT {
				tracing::warn!(
					?screen,
					unsettled = self.profile.unsettled(),
					"the profile did not settle in time"
				);
				break;
			}
			zgx::timer::sleep(capture::POLL).await;
		}
	}

	fn profiles_settled(&self, screen: capture::Screen) -> bool {
		let loaded = match screen {
			capture::Screen::ProfilePopout | capture::Screen::FullProfile => self
				.profile
				.target()
				.zip(self.state())
				.is_some_and(|((user_id, _), shared)| shared.read().profile(user_id).is_some()),
			_ => true,
		};
		loaded && self.profile.settled()
	}

	pub(super) fn newest_other_author(&self) -> Option<Snowflake> {
		let me = self.state()?.read().current_user_id();
		let timeline = self.timeline.borrow();
		timeline
			.messages()
			.rev()
			.map(|entry| entry.message.author.id)
			.find(|author| Some(*author) != me)
	}

	pub(super) async fn press_at(&self, x: f32, y: f32) {
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			tracing::warn!("the window is gone");
			return;
		};
		ui.dispatch(InputEvent::PointerMove { x, y });
		zgx::timer::sleep(capture::POLL).await;
		ui.dispatch(InputEvent::PointerDown {
			x,
			y,
			button: PointerButton::Primary,
		});
		zgx::timer::sleep(capture::POLL).await;
		ui.dispatch(InputEvent::PointerUp {
			x,
			y,
			button: PointerButton::Primary,
		});
	}
}

pub(super) fn empty_panel() -> zgx_script::Value {
	record([
		("name", "".into()),
		("subtitle", "".into()),
		("emoji", "".into()),
		("emoji_image", "".into()),
		("status", "online".into()),
		("latency", "Connecting".into()),
	])
}
