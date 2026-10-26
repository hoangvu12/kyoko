use std::rc::Rc;

use bridge::{AppSearch, apps_launcher, search_apps};
use chrono::Utc;
use session::Command;
use state::{Changes, State};
use zgx::compose::prelude::*;
use zgx_script::ScriptHost;

use super::{Link, Slot};
use crate::apps_launcher::{AppsLauncherView, COMPOSER_GAP, FALLBACK_HEIGHT, Pick};
use crate::pickers::ANCHOR_INSET;
use crate::timeline::Pictures;

pub(super) const APPS_TAB: &str = "apps";
const SLOT_TOP: f32 = 6.;
const CHAT_INSET: f32 = 8.;
const SMALLEST: f32 = 160.;

pub(super) struct Apps {
	launcher: Rc<AppsLauncherView>,
}

impl Apps {
	pub(super) fn new(runtime: &zgx::reactive::Runtime, pictures: Rc<Pictures>) -> Self {
		Self {
			launcher: AppsLauncherView::new(runtime, pictures),
		}
	}

	pub(super) fn view(&self) -> View {
		self.launcher.view()
	}

	pub(super) fn launcher(&self) -> &Rc<AppsLauncherView> {
		&self.launcher
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.launcher.images().iter().map(ToString::to_string).collect()
	}
}

pub(super) fn launcher_height(composer: Option<Slot>, timeline: Slot) -> f32 {
	let Some(composer) = composer else {
		return FALLBACK_HEIGHT;
	};
	if timeline.height <= 0. {
		return FALLBACK_HEIGHT;
	}
	let top = timeline.y - SLOT_TOP + CHAT_INSET;
	(composer.y - COMPOSER_GAP - top).max(SMALLEST)
}

fn now_ms() -> u64 {
	u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default()
}

impl Link {
	pub(super) fn install_apps(_host: &ScriptHost, link: &Rc<Link>) {
		let launcher = link.apps.launcher();
		let actions = Rc::downgrade(link);
		launcher.on_pick(move |pick| {
			if let Some(link) = actions.upgrade() {
				match pick {
					Pick::App(app_id) => link.composer_open_app(app_id),
					Pick::Command(entry) => link.composer_pick_command(entry),
				}
			}
		});
		let actions = Rc::downgrade(link);
		launcher.on_search(move |query| {
			if let Some(link) = actions.upgrade() {
				link.apps_search(&query);
			}
		});
		let actions = Rc::downgrade(link);
		launcher.on_images_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
	}

	pub(super) fn open_apps(&self, at: (f32, f32)) {
		let Some(shared) = self.state() else {
			return;
		};
		let Some((guild_id, channel_id)) = self.composer_channel() else {
			return;
		};
		let (model, missing) = {
			let state = shared.read();
			(
				apps_launcher(&state, guild_id, channel_id, now_ms()),
				state.command_index(channel_id).is_none(),
			)
		};
		if missing {
			self.send(Command::LoadApplicationCommands { channel_id });
		}
		self.close_expressions();
		let height = launcher_height(
			self.composer_slot(),
			self.window.slot.with_untracked(|slot| *slot),
		);
		self.apps.launcher().open(at, height, model, guild_id.is_some());
	}

	pub(super) fn use_apps(&self) {
		let at = self
			.composer_slot()
			.map_or(crate::capture::COMPOSER_PICKER, |composer| {
				(composer.x + composer.width - ANCHOR_INSET, composer.y)
			});
		self.open_apps(at);
	}

	pub(super) fn close_apps(&self) {
		self.apps.launcher().close();
	}

	fn apps_search(&self, query: &str) {
		let (Some(shared), Some((guild_id, channel_id))) = (self.state(), self.composer_channel()) else {
			return;
		};
		let found = if query.trim().is_empty() {
			AppSearch::default()
		} else {
			search_apps(&shared.read(), guild_id, channel_id, query)
		};
		self.apps.launcher().set_search(found);
	}

	pub(super) fn apps_apply(&self, state: &State, changes: &Changes) {
		let launcher = self.apps.launcher();
		if !launcher.is_open() || !changes.commands {
			return;
		}
		let Some((guild_id, channel_id)) = self.composer_channel() else {
			return;
		};
		launcher.set_model(apps_launcher(state, guild_id, channel_id, now_ms()));
		let query = launcher.query();
		if !query.trim().is_empty() {
			launcher.set_search(search_apps(state, guild_id, channel_id, &query));
		}
	}

	pub(super) async fn drive_apps(&self) {
		self.until_capture(|link| link.composer_slot().is_some() && link.composer_channel().is_some())
			.await;
		self.use_apps();
		self.until_capture(|link| link.apps_settled()).await;
	}

	pub(super) fn apps_settled(&self) -> bool {
		let loaded = self.composer_channel().is_some_and(|(_, channel_id)| {
			self.state().is_some_and(|shared| {
				let state = shared.read();
				state.command_index(channel_id).is_some() && !state.command_uses().is_empty()
			})
		});
		loaded && self.apps.launcher().settled()
	}
}
