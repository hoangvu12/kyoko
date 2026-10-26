use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use bridge::{EmojiPicker, GifSearch, Inbox, ServerPage, StickerPicker};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx::compose::prelude::*;
use zgx_script::{ScriptHost, Value};

use super::{Link, Slot, field, number, text};
use crate::capture;
use crate::pickers::{ANCHOR_INSET, ExpressionPicker, Favorite, Mode, Search, Tab};
use crate::timeline::Pictures;

pub(super) const COMPOSER_SLOT: &str = "composer";
pub(super) const CAPTURE_EMOJI_QUERY: &str = "sm";

pub(super) struct Expressions {
	picker: Rc<ExpressionPicker>,
	gifs: RefCell<GifSearch>,
	due: Inbox<Instant>,
	stickers_asked: Cell<bool>,
	composer: Cell<Option<Slot>>,
	top_asked: RefCell<HashSet<Snowflake>>,
}

impl Expressions {
	pub(super) fn set_plugin_menu(&self, menu: Rc<crate::plugin_menu::PluginMenu>) {
		self.picker.set_plugin_menu(menu);
	}

	pub(super) fn new(runtime: &zgx::reactive::Runtime, pictures: Rc<Pictures>) -> Self {
		Self {
			picker: ExpressionPicker::new(runtime, pictures, Mode::Composer),
			gifs: RefCell::default(),
			due: Inbox::new(),
			stickers_asked: Cell::new(false),
			composer: Cell::new(None),
			top_asked: RefCell::default(),
		}
	}

	pub(super) fn view(&self) -> View {
		div()
			.absolute()
			.left(0.)
			.top(0.)
			.w(0.)
			.h(0.)
			.child(self.picker.view())
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.picker.images().iter().map(ToString::to_string).collect()
	}
}

fn composer_anchor(composer: Slot) -> (f32, f32) {
	(composer.x + composer.width - ANCHOR_INSET, composer.y)
}

fn picker_anchor(composer: Option<Slot>, toolbar: Option<Slot>) -> Option<(f32, f32)> {
	let right = toolbar
		.or(composer)
		.map(|slot| slot.x + slot.width - ANCHOR_INSET)?;
	let top = composer.or(toolbar).map(|slot| slot.y)?;
	Some((right, top))
}

fn toolbar_slot(toolbar: &Value) -> Option<Slot> {
	Some(Slot {
		x: number(toolbar, "x")?,
		y: number(toolbar, "y")?,
		width: number(toolbar, "w")?,
		height: number(toolbar, "h")?,
	})
}

impl Link {
	pub(super) fn install_expressions(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("composer.picker", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let tab = field(&argument, "tab").and_then(text).and_then(Tab::named);
			let toolbar = field(&argument, "toolbar").and_then(toolbar_slot);
			let at = picker_anchor(link.expressions.composer.get(), toolbar);
			if let (Some(tab), Some(at)) = (tab, at) {
				link.open_expressions(tab, at);
			}
		});
		let picker = &link.expressions.picker;
		let actions = Rc::downgrade(link);
		picker.on_images_changed(move || {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_emoji(move |cell| {
			if let Some(link) = actions.upgrade() {
				link.composer_insert(&cell);
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_gif(move |gif| {
			if let Some(link) = actions.upgrade() {
				let share = link.settings.settings().advanced.share_gif_usage;
				let query = link
					.expressions
					.gifs
					.borrow()
					.active()
					.unwrap_or_default()
					.to_owned();
				let usage = share.then_some(query.as_str());
				link.composer_expression(|composer, state| composer.pick_gif(state, gif, usage, Utc::now()));
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_sticker(move |cell| {
			if let Some(link) = actions.upgrade() {
				link.composer_expression(|composer, state| composer.pick_sticker(state, &cell, Utc::now()));
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_search(move |search| {
			if let Some(link) = actions.upgrade() {
				link.expression_search(search);
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_favorite(move |favorite| {
			if let Some(link) = actions.upgrade() {
				link.send(match favorite {
					Favorite::Gif(gif, favorite) => Command::SetGifFavorite { gif, favorite },
					Favorite::Emoji(entry, favorite) => Command::SetEmojiFavorite { entry, favorite },
				});
			}
		});
		let actions = Rc::downgrade(link);
		picker.on_add_emoji(move |()| {
			if let Some(link) = actions.upgrade() {
				link.open_server_settings(Some(ServerPage::Emoji));
			}
		});
	}

	pub(super) fn place_composer(&self, slot: Slot) {
		self.expressions.composer.set(Some(slot));
	}

	pub(super) fn composer_slot(&self) -> Option<Slot> {
		self.expressions.composer.get()
	}

	pub(super) fn open_expressions(&self, tab: Tab, at: (f32, f32)) {
		let Some(shared) = self.state() else {
			return;
		};
		let Some((guild_id, channel_id)) = self.composer_channel() else {
			return;
		};
		let commands = {
			let state = shared.read();
			let expressions = &self.expressions;
			expressions.stickers_asked.set(false);
			expressions.picker.set_emoji(EmojiPicker::of(&state, guild_id));
			expressions
				.picker
				.allow_add_emoji(guild_id.is_some_and(|guild_id| state.may_create_expressions(guild_id)));
			expressions
				.picker
				.set_stickers(StickerPicker::of(&state, guild_id, channel_id));
			let mut commands = expressions.gifs.borrow_mut().open(&state);
			if let Some(guild_id) = guild_id
				&& state.top_emojis(guild_id).is_none()
				&& expressions.top_asked.borrow_mut().insert(guild_id)
			{
				commands.push(Command::LoadTopEmojis { guild_id });
			}
			expressions
				.picker
				.set_gifs(expressions.gifs.borrow().view(&state));
			commands
		};
		for command in commands {
			self.send(command);
		}
		self.expressions.picker.open(tab, at);
	}

	fn expression_search(&self, search: Search) {
		let Some(shared) = self.state() else {
			return;
		};
		let now = Instant::now();
		let commands = {
			let state = shared.read();
			let expressions = &self.expressions;
			let mut gifs = expressions.gifs.borrow_mut();
			let commands = match &search {
				Search::Typed(Tab::Gif, typed) => gifs.typed(&state, typed, now),
				Search::Submitted(Tab::Gif, typed) => gifs.submit(&state, typed),
				Search::Category(name) => gifs.browse_category(&state, name),
				Search::Trending => gifs.show_trending(&state),
				Search::Favorites => {
					gifs.show_favorites();
					Vec::new()
				}
				Search::Back => {
					gifs.back();
					Vec::new()
				}
				Search::Typed(Tab::Sticker, _) if !expressions.stickers_asked.replace(true) => {
					StickerPicker::needs(&state, self.composer_channel().and_then(|(guild_id, _)| guild_id))
				}
				_ => Vec::new(),
			};
			if let Some(due) = gifs.due_at() {
				expressions.due.push(due);
			}
			expressions.picker.set_gifs(gifs.view(&state));
			commands
		};
		for command in commands {
			self.send(command);
		}
	}

	pub(super) async fn follow_gif_search(self: Rc<Self>) {
		while let Some(batch) = self.expressions.due.next().await {
			let Some(due) = batch.into_iter().max() else {
				continue;
			};
			let wait = due.saturating_duration_since(Instant::now());
			zgx::timer::sleep(wait).await;
			let Some(shared) = self.state() else {
				continue;
			};
			let commands = {
				let state = shared.read();
				let mut gifs = self.expressions.gifs.borrow_mut();
				let commands = gifs.poll(&state, Instant::now());
				self.expressions.picker.set_gifs(gifs.view(&state));
				commands
			};
			for command in commands {
				self.send(command);
			}
		}
	}

	pub(super) fn expressions_apply(&self, state: &State, changes: &Changes) {
		let expressions = &self.expressions;
		if !expressions.picker.is_open()
			|| (changes.loaded.is_empty()
				&& !changes.guild_list
				&& !changes.favorite_gifs
				&& !changes.favorite_emojis)
		{
			return;
		}
		let Some((guild_id, channel_id)) = self.composer_channel() else {
			return;
		};
		match expressions.picker.tab() {
			Tab::Gif => expressions.picker.set_gifs(expressions.gifs.borrow().view(state)),
			Tab::Sticker => expressions
				.picker
				.set_stickers(StickerPicker::of(state, guild_id, channel_id)),
			Tab::Emoji => expressions.picker.set_emoji(EmojiPicker::of(state, guild_id)),
		}
	}

	pub(super) fn close_expressions(&self) {
		self.expressions.picker.close();
	}

	pub(super) fn drive_expressions(&self, tab: Tab) {
		let at = self
			.expressions
			.composer
			.get()
			.map_or(capture::COMPOSER_PICKER, composer_anchor);
		self.open_expressions(tab, at);
	}

	pub(super) fn drive_expression_search(&self, query: &str) {
		self.expressions.picker.type_query(query);
	}

	pub(super) fn drive_trending_gifs(&self) {
		self.expression_search(Search::Trending);
	}

	pub(super) fn hover_first_expression(&self) {
		self.expressions.picker.hover_first();
	}

	pub(super) async fn drive_picker_shot(&self, shot: capture::PickerShot) {
		let tab = match shot {
			capture::PickerShot::GifResults => Tab::Gif,
			capture::PickerShot::EmojiSearch | capture::PickerShot::EmojiNoNitro => Tab::Emoji,
		};
		self.drive_expressions(tab);
		self.until_capture(|link| link.expressions_settled(tab)).await;
		match shot {
			capture::PickerShot::GifResults => {
				self.drive_trending_gifs();
				self.until_capture(|link| link.expressions_settled(tab)).await;
			}
			capture::PickerShot::EmojiSearch => {
				self.drive_expression_search(CAPTURE_EMOJI_QUERY);
				self.until_capture(|link| link.expressions_settled(tab)).await;
			}
			capture::PickerShot::EmojiNoNitro => {
				self.expressions.picker.reveal_locked();
				return;
			}
		}
		self.hover_first_expression();
	}

	pub(super) fn expressions_settled(&self, tab: Tab) -> bool {
		let guild_id = self.composer_channel().and_then(|(guild_id, _)| guild_id);
		let loaded = self.state().is_some_and(|shared| {
			let state = shared.read();
			match tab {
				Tab::Gif => {
					state.gif_trending().is_some()
						&& !matches!(
							self.expressions.gifs.borrow().view(&state),
							bridge::GifView::Results { gifs: None, .. }
						)
				}
				Tab::Sticker => state.sticker_packs().is_some(),
				Tab::Emoji => {
					state.favorites_loaded()
						&& guild_id.is_none_or(|guild_id| state.top_emojis(guild_id).is_some())
				}
			}
		});
		loaded && self.expressions.picker.settled()
	}
}
