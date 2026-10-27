use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use bridge::{Inbox, MenuFacts, PluginMenuItem, ToastKind};
use plugins::{MenuItem, PluginRequest, Slot};
use protocol::Snowflake;
use state::{Changes, Expression, State};

use super::Link;
use crate::capture;
use crate::community::Dialog;
use crate::plugin_menu::{Choice, PluginMenu};

pub(super) struct Cloner {
	menu: Rc<PluginMenu>,
	requests: Inbox<PluginRequest>,
	fetched: Inbox<Fetched>,
	fetching: Cell<bool>,
	pending: RefCell<Option<Pending>>,
}

struct Fetched {
	order: Order,
	bytes: Result<Vec<u8>, String>,
}

#[derive(Clone)]
pub(super) struct Order {
	pub expression: Expression,
	pub guild_id: Snowflake,
	pub server: String,
	pub name: String,
}

struct Pending {
	order: Order,
	known: HashSet<Snowflake>,
	rejections: u64,
}

impl Cloner {
	pub(super) fn new() -> Self {
		Self {
			menu: PluginMenu::new(),
			requests: Inbox::new(),
			fetched: Inbox::new(),
			fetching: Cell::new(false),
			pending: RefCell::new(None),
		}
	}

	pub(super) fn menu(&self) -> Rc<PluginMenu> {
		self.menu.clone()
	}
}

fn activity_text(shared: bool) -> &'static str {
	if shared {
		"Game activity is shared again"
	} else {
		"Game activity is hidden"
	}
}

fn menu_item(item: MenuItem) -> PluginMenuItem {
	PluginMenuItem {
		plugin: item.plugin,
		key: item.key,
		label: item.label,
		icon: item.icon.map(Into::into),
		when: item.when,
	}
}

fn known_ids(state: &State, order: &Order) -> HashSet<Snowflake> {
	match &order.expression {
		Expression::Emoji { .. } => state
			.guild(order.guild_id)
			.map(|guild| guild.emojis.iter().filter_map(|emoji| emoji.id).collect())
			.unwrap_or_default(),
		Expression::Sticker { .. } => state
			.guild_stickers(order.guild_id)
			.map(|stickers| stickers.iter().map(|sticker| sticker.id).collect())
			.unwrap_or_default(),
	}
}

fn arrived(state: &State, pending: &Pending) -> bool {
	let order = &pending.order;
	let fresh = |id: Snowflake, name: &str| !pending.known.contains(&id) && name == order.name;
	match &order.expression {
		Expression::Emoji { .. } => state.guild(order.guild_id).is_some_and(|guild| {
			guild.emojis.iter().any(|emoji| {
				emoji
					.id
					.zip(emoji.name.as_deref())
					.is_some_and(|(id, name)| fresh(id, name))
			})
		}),
		Expression::Sticker { .. } => state
			.guild_stickers(order.guild_id)
			.is_some_and(|stickers| stickers.iter().any(|sticker| fresh(sticker.id, &sticker.name))),
	}
}

fn added_text(order: &Order) -> String {
	match order.expression {
		Expression::Emoji { .. } => format!("Added :{}: to {}", order.name, order.server),
		Expression::Sticker { .. } => format!("Added the sticker {} to {}", order.name, order.server),
	}
}

impl Link {
	pub(super) fn install_cloner(link: &Rc<Link>) {
		let menu = link.cloner.menu();
		let facts = Rc::downgrade(link);
		menu.on_facts(move |target| {
			let shared = facts.upgrade().and_then(|link| link.state());
			match shared {
				Some(shared) => MenuFacts::of(&shared.read(), target),
				None => MenuFacts {
					target,
					can_create_expressions: false,
				},
			}
		});
		let pressed = Rc::downgrade(link);
		menu.on_press(move |()| {
			if let Some(link) = pressed.upgrade() {
				link.plugin_input();
			}
		});
		let chosen = Rc::downgrade(link);
		menu.on_choose(move |choice| {
			if let Some(link) = chosen.upgrade() {
				link.choose_plugin_item(choice);
			}
		});
		Link::install_accessories(link, &menu);
		link.timeline_view.set_plugin_menu(menu.clone());
		link.threads.set_plugin_menu(menu.clone());
		link.expressions.set_plugin_menu(menu);
		let requests = link.cloner.requests.clone();
		link.plugins
			.host()
			.on_request(move |request| requests.push(request));
		link.refresh_plugin_menu();
	}

	pub(super) fn refresh_plugin_menu(&self) {
		let items = self
			.plugins
			.host()
			.menu_items(Slot::MessageMenu)
			.into_iter()
			.map(menu_item)
			.collect();
		self.cloner.menu.set_items(items);
	}

	fn choose_plugin_item(&self, choice: Choice) {
		if !self.plugins.take_activation() {
			tracing::warn!(plugin = %choice.item.plugin, "a plugin menu item without a click was refused");
			return;
		}
		let mut context = Vec::new();
		if let Some((channel_id, message_id)) = choice.message {
			context.push(("channel_id".to_owned(), channel_id.to_string()));
			context.push(("message_id".to_owned(), message_id.to_string()));
		}
		let host = self.plugins.host().clone();
		self.plugins.runtime().spawn_blocking(move || {
			let Choice { item, target, .. } = choice;
			if let Err(error) =
				host.menu_gesture(&item.plugin, &item.key, Slot::MessageMenu, target, &context)
			{
				tracing::info!(plugin = %item.plugin, %error, "plugin menu item failed");
			}
		});
	}

	pub(super) async fn follow_plugin_requests(self: Rc<Self>) {
		while let Some(requests) = self.cloner.requests.next().await {
			for request in requests {
				match request {
					PluginRequest::Clone { plugin, expression } => {
						tracing::info!(%plugin, sticker = expression.is_sticker(), "a plugin offers to clone");
						self.community.open(Dialog::Clone(expression));
					}
					PluginRequest::ActivitySharing { shared, .. } => {
						self.toasts.push(ToastKind::Success, activity_text(shared));
					}
				}
			}
		}
	}

	pub(super) fn clone_expression(&self, order: Order) {
		if self.cloner.fetching.get() || self.cloner.pending.borrow().is_some() {
			self.toasts
				.push(ToastKind::Warning, "Wait for the last clone to finish first");
			return;
		}
		let Some(url) = bridge::clone_source(&self.images.cdn, &order.expression) else {
			self.toasts.push(ToastKind::Error, "This sticker can't be cloned");
			return;
		};
		let url = bridge::local_media(&url, self.images.mock_origin.as_deref());
		let loader = self.images.gallery.loader().clone();
		let fetched = self.cloner.fetched.clone();
		self.cloner.fetching.set(true);
		self.plugins.runtime().spawn(async move {
			let bytes = loader
				.bytes(&url)
				.await
				.map(|bytes| bytes.to_vec())
				.map_err(|error| error.to_string());
			fetched.push(Fetched { order, bytes });
		});
	}

	pub(super) async fn follow_clones(self: Rc<Self>) {
		while let Some(batch) = self.cloner.fetched.next().await {
			for fetched in batch {
				self.cloned(fetched);
			}
		}
	}

	fn cloned(&self, Fetched { order, bytes }: Fetched) {
		self.cloner.fetching.set(false);
		let bytes = match bytes {
			Ok(bytes) => bytes,
			Err(error) => {
				tracing::warn!(%error, "the expression to clone was not downloaded");
				let noun = if order.expression.is_sticker() {
					"sticker"
				} else {
					"emoji"
				};
				self.toasts.push(
					ToastKind::Error,
					&format!("Couldn't download the {noun} to clone"),
				);
				return;
			}
		};
		let Some(shared) = self.state() else {
			return;
		};
		let (command, known, rejections) = {
			let state = shared.read();
			(
				bridge::clone_command(&state, &order.expression, order.guild_id, &order.name, bytes),
				known_ids(&state, &order),
				state.last_rejection().map_or(0, |rejection| rejection.id),
			)
		};
		*self.cloner.pending.borrow_mut() = Some(Pending {
			order,
			known,
			rejections,
		});
		self.send(command);
	}

	fn capture_emoji(&self) -> Option<Expression> {
		let shared = self.state()?;
		let state = shared.read();
		let open = self.timeline.borrow().open().and_then(|open| open.guild_id);
		let mut guilds: Vec<&state::GuildState> = state.guilds().collect();
		guilds.sort_by_key(|guild| Some(guild.id) != open);
		guilds.into_iter().find_map(|guild| {
			guild.emojis.iter().find_map(|emoji| {
				Some(bridge::emoji_expression(
					emoji.name.as_deref()?,
					emoji.id?,
					emoji.animated.unwrap_or(false),
				))
			})
		})
	}

	async fn until_plugin_menu(&self) {
		self.until_capture(|link| !link.cloner.menu.shown(None).items.is_empty())
			.await;
	}

	pub(super) async fn drive_clone_menu(&self) {
		self.until_plugin_menu().await;
		let Some(expression) = self.capture_emoji() else {
			tracing::warn!("no custom emoji to right-click");
			return;
		};
		self.timeline_view.aim(expression, capture::LAST_MESSAGE);
		self.point_at_last_message(true).await;
	}

	pub(super) async fn drive_clone_dialog(&self) {
		self.until_plugin_menu().await;
		match self.capture_emoji() {
			Some(expression) => self.community.open(Dialog::Clone(expression)),
			None => tracing::warn!("no custom emoji to clone"),
		}
	}

	pub(super) fn cloner_apply(&self, state: &State, changes: &Changes) {
		let mut pending = self.cloner.pending.borrow_mut();
		let Some(current) = pending.as_ref() else {
			return;
		};
		if arrived(state, current) {
			self.toasts.push(ToastKind::Success, &added_text(&current.order));
			*pending = None;
			return;
		}
		let rejected = changes.rejected
			&& state
				.last_rejection()
				.is_some_and(|rejection| rejection.id > current.rejections);
		if rejected {
			*pending = None;
		}
	}
}
