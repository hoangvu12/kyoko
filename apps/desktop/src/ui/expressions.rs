use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use bridge::{EmojiPicker, GifSearch, Inbox, StickerPicker};
use chrono::Utc;
use state::{Changes, State};
use zgx::compose::prelude::*;
use zgx_script::{ScriptHost, Value};

use super::{Link, field, number, text};
use crate::capture;
use crate::pickers::{ExpressionPicker, Mode, Search, Tab};
use crate::timeline::Pictures;

const TOOLBAR_RIGHT: f32 = 8.;
const BUTTON: f32 = 32.;
const BUTTON_GAP: f32 = 2.;

pub(super) struct Expressions {
	picker: Rc<ExpressionPicker>,
	gifs: RefCell<GifSearch>,
	due: Inbox<Instant>,
	stickers_asked: Cell<bool>,
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

fn button_center(tab: Tab, toolbar: &Value) -> Option<(f32, f32)> {
	let (x, y) = (number(toolbar, "x")?, number(toolbar, "y")?);
	let (width, height) = (number(toolbar, "w")?, number(toolbar, "h")?);
	let from_right = match tab {
		Tab::Emoji => 0.,
		Tab::Sticker => 1.,
		Tab::Gif => 2.,
	};
	let center = x + width - TOOLBAR_RIGHT - BUTTON / 2. - from_right * (BUTTON + BUTTON_GAP);
	Some((center, y + height / 2.))
}

impl Link {
	pub(super) fn install_expressions(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("composer.picker", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let tab = field(&argument, "tab").and_then(text).and_then(Tab::named);
			let at = tab
				.zip(field(&argument, "toolbar"))
				.and_then(|(tab, toolbar)| button_center(tab, toolbar));
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
				.set_stickers(StickerPicker::of(&state, guild_id, channel_id));
			let commands = expressions.gifs.borrow_mut().open(&state);
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
		if !expressions.picker.is_open() || (changes.loaded.is_empty() && !changes.guild_list) {
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
			Tab::Emoji => {}
		}
	}

	pub(super) fn close_expressions(&self) {
		self.expressions.picker.close();
	}

	pub(super) fn drive_expressions(&self, tab: Tab) {
		self.open_expressions(tab, capture::COMPOSER_PICKER);
	}

	pub(super) fn expressions_settled(&self, tab: Tab) -> bool {
		let loaded = self.state().is_some_and(|shared| {
			let state = shared.read();
			match tab {
				Tab::Gif => state.gif_trending().is_some(),
				Tab::Sticker => state.sticker_packs().is_some(),
				Tab::Emoji => true,
			}
		});
		loaded && self.expressions.picker.settled()
	}
}
