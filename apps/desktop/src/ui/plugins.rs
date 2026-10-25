use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bridge::Inbox;
use plugins::{Origin, Permission, PluginHost, PluginInfo, SettingKind, Status};
use serde_json::{Value as Json, json};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, text};

const ACTIVATION: Duration = Duration::from_secs(1);
const GESTURE_SOURCE: &str = "settings";

pub struct PluginsView {
	host: PluginHost,
	runtime: tokio::runtime::Handle,
	list: Shared,
	notice: Shared,
	wakes: Inbox<()>,
	activation: Cell<Option<Instant>>,
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
		let view = Self {
			list: script.shared("plugins.list", Value::List(Rc::from([]))),
			notice: script.shared("plugins.notice", Value::Nil),
			host,
			runtime,
			wakes,
			activation: Cell::new(None),
		};
		script.shared("plugins.folder", folder.as_str());
		view.publish();
		view
	}

	pub fn host(&self) -> &PluginHost {
		&self.host
	}

	pub fn activated(&self) {
		self.activation.set(Some(Instant::now()));
	}

	fn take_activation(&self) -> bool {
		self.activation
			.take()
			.is_some_and(|at| at.elapsed() <= ACTIVATION)
	}

	fn publish(&self) {
		let list: Vec<Json> = self.host.snapshot().iter().map(plugin_json).collect();
		if let Some(value) = Value::from_json(&Json::Array(list).to_string()) {
			self.list.set(value);
		}
		if let Some(notice) = self.host.take_notices().pop() {
			let json = json!({ "plugin": notice.plugin, "text": notice.text });
			if let Some(value) = Value::from_json(&json.to_string()) {
				self.notice.set(value);
			}
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
		while self.plugins.wakes.next().await.is_some() {
			self.plugins.publish();
		}
	}

	pub(super) fn plugin_input(&self) {
		self.plugins.activated();
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

fn plugin_json(info: &PluginInfo) -> Json {
	let settings: Vec<Json> = info
		.settings
		.iter()
		.map(|spec| {
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
			json!({
				"key": spec.key,
				"type": spec.type_name(),
				"label": spec.label,
				"description": spec.description,
				"value": info.values.get(&spec.key).cloned().unwrap_or(Json::Null),
				"min": min,
				"max": max,
				"step": step,
				"multiline": matches!(spec.kind, SettingKind::Text { multiline: true, .. }),
				"options": options,
			})
		})
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
