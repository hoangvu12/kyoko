use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bridge::Inbox;
use plugins::{Origin, Permission, PluginHost, PluginInfo, SettingKind, SettingSpec, Status};
use serde_json::{Value as Json, json};
use zgx_script::{ScriptHost, Shared, Value};

use super::plugin_slots::{PluginSlots, Request};
use super::{Link, field, text};
use crate::capture;

const ACTIVATION: Duration = Duration::from_secs(1);
const GESTURE_SOURCE: &str = "settings";
const SLOT_SOURCE: &str = "slot";
const SILENT_TYPING: &str = "silent-typing";
const TEXT_REPLACE: &str = "text-replace";
const GAME_ACTIVITY_TOGGLE: &str = "game-activity-toggle";

type Activation = Rc<Cell<Option<Instant>>>;

pub struct PluginsView {
	host: PluginHost,
	runtime: tokio::runtime::Handle,
	script: ScriptHost,
	list: Shared,
	notice: Shared,
	slots: PluginSlots,
	wakes: Inbox<()>,
	activation: Activation,
}

impl PluginsView {
	pub fn new(script: &ScriptHost, host: PluginHost, runtime: tokio::runtime::Handle) -> Self {
		let wakes = Inbox::new();
		let woken = wakes.clone();
		host.on_change(move || woken.push(()));
		let folder = host
			.user_dir()
			.map(|dir| dir.display().to_string())
			.unwrap_or_default();
		let activation = Activation::default();
		let slots = PluginSlots::new(
			script,
			slot_requests(host.clone(), runtime.clone(), activation.clone()),
		);
		let view = Self {
			list: script.shared("plugins.list", Value::List(Rc::from([]))),
			notice: script.shared("plugins.notice", Value::Nil),
			script: script.clone(),
			slots,
			host,
			runtime,
			wakes,
			activation,
		};
		script.shared("plugins.folder", folder.as_str());
		view.publish();
		view
	}

	pub fn host(&self) -> &PluginHost {
		&self.host
	}

	pub(super) fn runtime(&self) -> &tokio::runtime::Handle {
		&self.runtime
	}

	pub fn activated(&self) {
		self.activation.set(Some(Instant::now()));
	}

	pub(super) fn take_activation(&self) -> bool {
		take_activation(&self.activation)
	}

	pub(super) fn publish(&self) -> bool {
		let snapshot = self.host.snapshot();
		let list: Vec<Json> = snapshot.iter().map(plugin_json).collect();
		if let Some(value) = Value::from_json(&Json::Array(list).to_string()) {
			self.list.set(value);
		}
		if let Some(notice) = self.host.take_notices().pop() {
			let json = json!({ "plugin": notice.plugin, "text": notice.text });
			if let Some(value) = Value::from_json(&json.to_string()) {
				self.notice.set(value);
			}
		}
		self.slots.publish(&self.script, &snapshot)
	}

	pub(super) fn remount(&self) {
		if let Err(error) = self.script.reload() {
			tracing::warn!(%error, "the interface did not load after the plugins changed");
		}
	}

	fn run(&self, work: impl FnOnce(&PluginHost) + Send + 'static) {
		let host = self.host.clone();
		self.runtime.spawn_blocking(move || work(&host));
	}
}

impl Link {
	pub(super) fn install_plugins(script: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		script.action("plugins.enable", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), owned(&argument)) {
				link.plugins.run(move |host| {
					if let Err(error) = host.enable(&id) {
						tracing::info!(plugin = %id, %error, "plugin not enabled");
					}
				});
			}
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.disable", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), owned(&argument)) {
				link.plugins.run(move |host| host.disable(&id));
			}
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.revoke", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), owned(&argument)) {
				link.plugins.run(move |host| host.revoke(&id));
			}
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.reload", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), owned(&argument)) {
				link.plugins.run(move |host| host.reload(&id));
			}
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.approve", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let Some(id) = field(&argument, "id").and_then(owned) else {
				return;
			};
			let shown: Vec<Permission> = match field(&argument, "permissions") {
				Some(Value::List(names)) => names.iter().filter_map(text).map(Permission::parse).collect(),
				_ => Vec::new(),
			};
			link.plugins.run(move |host| {
				if let Err(error) = host.approve(&id, &shown) {
					tracing::info!(plugin = %id, %error, "plugin not approved");
				}
			});
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.set", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let (Some(id), Some(key)) = (
				field(&argument, "id").and_then(owned),
				field(&argument, "key").and_then(owned),
			) else {
				return;
			};
			let value = field(&argument, "value")
				.and_then(|value| serde_json::from_str::<Json>(&value.to_json()).ok())
				.unwrap_or(Json::Null);
			link.plugins.run(move |host| {
				if let Err(error) = host.set_setting(&id, &key, &value) {
					tracing::info!(plugin = %id, %error, "plugin setting refused");
				}
			});
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.action", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let (Some(id), Some(key)) = (
				field(&argument, "id").and_then(owned),
				field(&argument, "key").and_then(owned),
			) else {
				return;
			};
			if !link.plugins.take_activation() {
				tracing::warn!(plugin = %id, "a plugin action without a click or key press was refused");
				return;
			}
			link.plugins.run(move |host| {
				let context = [("source".to_owned(), GESTURE_SOURCE.to_owned())];
				if let Err(error) = host.gesture(&id, &key, &context) {
					tracing::info!(plugin = %id, %error, "plugin action failed");
				}
			});
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.open_folder", move |_| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let Some(dir) = link.plugins.host.user_dir().map(std::path::Path::to_path_buf) else {
				return;
			};
			if let Err(error) = std::fs::create_dir_all(&dir) {
				tracing::warn!(%error, "the plugins folder could not be created");
				return;
			}
			if let Err(error) = platform::open::reveal_path(&dir) {
				tracing::warn!(%error, "could not open the plugins folder");
			}
		});
		let actions = Rc::downgrade(link);
		script.action("plugins.dismiss_notice", move |_| {
			if let Some(link) = actions.upgrade() {
				link.plugins.notice.set(Value::Nil);
			}
		});
	}

	pub(super) async fn follow_plugins(self: Rc<Self>) {
		self.apply_plugin_views();
		while self.plugins.wakes.next().await.is_some() {
			if self.plugins.publish() {
				self.plugins.remount();
			}
			self.refresh_plugin_menu();
			self.apply_plugin_views();
		}
	}

	pub(super) fn plugins_apply(&self, state: &state::State) {
		self.plugins
			.slots
			.share_activity(state.status_settings().shares_activity());
	}

	pub(super) fn plugin_input(&self) {
		self.plugins.activated();
	}

	pub(super) async fn enable_capture_plugins(&self, ids: &[String]) {
		for id in ids {
			let shown = self
				.plugin(id)
				.map(|plugin| plugin.permissions)
				.unwrap_or_default();
			let approved = id.clone();
			self.plugins.run(move |host| {
				if let Err(error) = host.approve(&approved, &shown) {
					tracing::warn!(%error, plugin = %approved, "the capture could not turn a plugin on");
				}
			});
			self.until_capture(|link| {
				link.plugin(id).is_some_and(|plugin| {
					plugin.status == Status::Running
						&& (plugin.ui_entry.is_none() || link.plugins.slots.is_mounted(id))
				})
			})
			.await;
			tracing::info!(plugin = %id, "the capture turned a plugin on");
		}
	}

	pub(super) async fn drive_silent_typing(&self) {
		let shown = self
			.plugin(SILENT_TYPING)
			.map(|plugin| plugin.permissions)
			.unwrap_or_default();
		self.plugins.run(move |host| {
			if let Err(error) = host.approve(SILENT_TYPING, &shown) {
				tracing::warn!(%error, "the capture could not turn SilentTyping on");
			}
		});
		self.until_capture(|link| link.plugins.slots.is_mounted(SILENT_TYPING))
			.await;
		self.drive_composer(&capture::Screen::Composer);
		self.until_capture(|link| link.composer_settled(&capture::Screen::Composer))
			.await;
		for silent in [false, true] {
			let (x, y) = capture::SILENT_TYPING_BUTTON;
			self.press_at(x, y).await;
			self.until_capture(|link| {
				link.plugin(SILENT_TYPING)
					.is_some_and(|plugin| plugin.values.get("silent") == Some(&Json::Bool(silent)))
			})
			.await;
			let now = self
				.plugin(SILENT_TYPING)
				.and_then(|plugin| plugin.values.get("silent").cloned());
			tracing::info!(wanted = silent, ?now, "the SilentTyping button was clicked");
		}
	}

	pub(super) async fn drive_text_replace(&self) {
		let shown = self
			.plugin(TEXT_REPLACE)
			.map(|plugin| plugin.permissions)
			.unwrap_or_default();
		self.plugins.run(move |host| {
			let rules = json!([
				{ "find": "teh", "replace": "the", "only_if": "", "pattern": false },
				{ "find": "(%d+)km", "replace": "%1 kilometres", "only_if": "", "pattern": true },
				{ "find": "brb", "replace": "be right back", "only_if": "!", "pattern": false },
			]);
			if let Err(error) = host.set_setting(TEXT_REPLACE, "rules", &rules) {
				tracing::warn!(%error, "the capture could not set the TextReplace rules");
			}
			if let Err(error) = host.approve(TEXT_REPLACE, &shown) {
				tracing::warn!(%error, "the capture could not turn TextReplace on");
			}
		});
		self.until_capture(|link| {
			link.plugin(TEXT_REPLACE)
				.is_some_and(|plugin| plugin.status == Status::Running)
		})
		.await;
		self.plugins.publish();
		self.open_settings("plugins");
		self.until_capture(|link| link.settings_settled()).await;
		zgx::timer::sleep(capture::SETTLE).await;
		let (x, y) = capture::TEXT_REPLACE_SETTINGS;
		self.press_at(x, y).await;
	}

	pub(super) async fn drive_game_activity(&self) {
		self.until_capture(|link| link.plugins.slots.is_mounted(GAME_ACTIVITY_TOGGLE))
			.await;
		zgx::timer::sleep(capture::SETTLE).await;
		let (x, y) = capture::GAME_ACTIVITY_BUTTON;
		self.press_at(x, y).await;
		self.until_capture(|link| {
			link.state()
				.is_some_and(|state| !state.read().status_settings().shares_activity())
		})
		.await;
		tracing::info!("the GameActivityToggle button was clicked");
	}

	fn plugin(&self, id: &str) -> Option<PluginInfo> {
		self.plugins
			.host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == id)
	}
}

fn take_activation(activation: &Activation) -> bool {
	activation.take().is_some_and(|at| at.elapsed() <= ACTIVATION)
}

fn slot_requests(
	host: PluginHost,
	runtime: tokio::runtime::Handle,
	activation: Activation,
) -> impl Fn(Request) + 'static {
	move |request| {
		if !take_activation(&activation) {
			tracing::warn!("a plugin button without a click or key press was refused");
			return;
		}
		let host = host.clone();
		runtime.spawn_blocking(move || match request {
			Request::Set { id, key, value } => {
				if let Err(error) = host.set_setting(&id, &key, &value) {
					tracing::info!(plugin = %id, %error, "plugin setting refused");
				}
			}
			Request::Action { id, key, slot } => {
				let mut context = vec![("source".to_owned(), SLOT_SOURCE.to_owned())];
				context.extend(slot.map(|slot| ("slot".to_owned(), slot.to_owned())));
				if let Err(error) = host.gesture(&id, &key, &context) {
					tracing::info!(plugin = %id, %error, "plugin action failed");
				}
			}
		});
	}
}

fn owned(value: &Value) -> Option<String> {
	text(value).map(str::to_owned)
}

fn origin_label(origin: Origin) -> &'static str {
	match origin {
		Origin::Bundled => "Built in",
		Origin::User => "Local",
		Origin::Development => "Development",
	}
}

fn status_label(status: &Status) -> &'static str {
	match status {
		Status::Disabled => "Disabled",
		Status::NeedsApproval => "Needs approval",
		Status::Running => "Enabled",
		Status::Suspended { .. } => "Paused",
		Status::Stopped => "Turned off",
		Status::Failed(_) => "Failed",
		Status::Incompatible => "Incompatible",
	}
}

fn setting_json(spec: &SettingSpec, value: Json) -> Json {
	let (min, max, step) = match spec.kind {
		SettingKind::Number { min, max, step, .. } => (min, max, step),
		_ => (0., 0., 0.),
	};
	let options: Vec<Json> = match &spec.kind {
		SettingKind::Select { options, .. } => options
			.iter()
			.map(|option| json!({ "value": option.value, "label": option.label }))
			.collect(),
		_ => Vec::new(),
	};
	let (fields, item, rows): (Vec<Json>, &str, usize) = match &spec.kind {
		SettingKind::List {
			fields, item, max, ..
		} => (
			fields
				.iter()
				.map(|field| setting_json(field, field.default_value()))
				.collect(),
			item,
			*max,
		),
		_ => (Vec::new(), "", 0),
	};
	json!({
		"key": spec.key,
		"type": spec.type_name(),
		"label": spec.label,
		"description": spec.description,
		"value": value,
		"min": min,
		"max": max,
		"step": step,
		"multiline": matches!(spec.kind, SettingKind::Text { multiline: true, .. }),
		"options": options,
		"fields": fields,
		"item": item,
		"rows": rows,
	})
}

fn plugin_json(info: &PluginInfo) -> Json {
	let settings: Vec<Json> = info
		.settings
		.iter()
		.map(|spec| setting_json(spec, info.values.get(&spec.key).cloned().unwrap_or(Json::Null)))
		.collect();
	let permissions: Vec<Json> = info
		.permissions
		.iter()
		.map(|permission| {
			json!({
				"name": permission.name(),
				"text": permission.describe(),
				"sensitive": permission.sensitive(),
				"reserved": !permission.available(),
			})
		})
		.collect();
	let error = match &info.status {
		Status::Failed(error) => error.clone(),
		Status::Incompatible => "This plugin needs another version of Kyoko.".to_owned(),
		_ => String::new(),
	};
	json!({
		"id": info.id,
		"name": info.name,
		"version": info.version,
		"description": info.description,
		"authors": info.authors.join(", "),
		"origin": info.origin.name(),
		"origin_label": origin_label(info.origin),
		"status": info.status.name(),
		"status_label": status_label(&info.status),
		"enabled": info.enabled,
		"needs_approval": info.needs_approval,
		"notice": info.notice.clone().unwrap_or_default(),
		"error": error,
		"settings": settings,
		"permissions": permissions,
	})
}
