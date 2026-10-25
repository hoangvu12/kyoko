use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use plugins::{Permission, PluginInfo, Slot, Status};
use serde_json::{Map, Value as Json};
use zgx_script::{Capability, Plugin, ScriptHost, Shared, Value};

use super::{field, text};

pub const PLACED: [Slot; 3] = [Slot::ComposerButtons, Slot::PanelButtons, Slot::RailButtons];

#[derive(Debug, PartialEq)]
pub enum Request {
	Set {
		id: String,
		key: String,
		value: Json,
	},
	Action {
		id: String,
		key: String,
		slot: Option<&'static str>,
	},
}

#[derive(Clone, Debug, PartialEq)]
struct Mount {
	id: String,
	entry: PathBuf,
	slots: Vec<Slot>,
	generation: u64,
}

impl Mount {
	fn of(info: &PluginInfo) -> Option<Mount> {
		if info.status != Status::Running {
			return None;
		}
		let entry = info.ui_entry.clone()?;
		let slots: Vec<Slot> = PLACED
			.into_iter()
			.filter(|slot| info.grants.contains(&Permission::Ui(*slot)))
			.collect();
		(!slots.is_empty()).then(|| Mount {
			id: info.id.clone(),
			entry,
			slots,
			generation: info.generation,
		})
	}

	fn plugin(&self) -> Plugin {
		Plugin {
			name: self.id.clone(),
			entry: self.entry.clone(),
			grants: vec![capability(&self.id)],
		}
	}
}

type Mounted = Rc<RefCell<Vec<Mount>>>;

pub struct PluginSlots {
	slots: Shared,
	handler: Rc<dyn Fn(Request)>,
	mounted: Mounted,
	settings: RefCell<BTreeMap<String, Shared>>,
}

impl PluginSlots {
	pub fn new(script: &ScriptHost, handler: impl Fn(Request) + 'static) -> Self {
		let mounted = Mounted::default();
		let watched = mounted.clone();
		script.on_error(move |message| report_failures(&watched, message));
		Self {
			slots: script.shared("plugins.slots", json_value(&Json::Object(Map::new()))),
			handler: Rc::new(handler),
			mounted,
			settings: RefCell::default(),
		}
	}

	pub fn publish(&self, script: &ScriptHost, plugins: &[PluginInfo]) -> bool {
		let mounts: Vec<Mount> = plugins.iter().filter_map(Mount::of).collect();
		for mount in &mounts {
			self.expose(script, &mount.id);
		}
		for info in plugins {
			if let Some(shared) = self.settings.borrow().get(&info.id) {
				shared.set(json_value(&Json::Object(info.values.clone())));
			}
		}
		if *self.mounted.borrow() == mounts {
			return false;
		}
		script.set_plugins(mounts.iter().map(Mount::plugin).collect());
		self.slots.set(json_value(&slots_json(&mounts)));
		*self.mounted.borrow_mut() = mounts;
		true
	}

	pub fn is_mounted(&self, id: &str) -> bool {
		!slots_of(&self.mounted, id).is_empty()
	}

	fn expose(&self, script: &ScriptHost, id: &str) {
		if self.settings.borrow().contains_key(id) {
			return;
		}
		let settings = format!("plugin.{id}.settings");
		let shared = script.shared(&settings, json_value(&Json::Object(Map::new())));
		script.expose_shared(settings, Some(capability(id)));

		let set = format!("plugin.{id}.set");
		let (handler, mounted, owner) = (self.handler.clone(), self.mounted.clone(), id.to_owned());
		script.action(set.clone(), move |argument| {
			let Some(key) = field(&argument, "key").and_then(text) else {
				return;
			};
			if slots_of(&mounted, &owner).is_empty() {
				return;
			}
			let value = field(&argument, "value")
				.and_then(|value| serde_json::from_str(&value.to_json()).ok())
				.unwrap_or(Json::Null);
			handler(Request::Set {
				id: owner.clone(),
				key: key.to_owned(),
				value,
			});
		});
		script.expose_action(set, Some(capability(id)));

		let action = format!("plugin.{id}.action");
		let (handler, mounted, owner) = (self.handler.clone(), self.mounted.clone(), id.to_owned());
		script.action(action.clone(), move |argument| {
			let Some(key) = field(&argument, "key").and_then(text) else {
				return;
			};
			let granted = slots_of(&mounted, &owner);
			if granted.is_empty() {
				return;
			}
			let slot = field(&argument, "slot")
				.and_then(text)
				.and_then(|name| granted.into_iter().find(|slot| slot.name() == name))
				.map(Slot::name);
			handler(Request::Action {
				id: owner.clone(),
				key: key.to_owned(),
				slot,
			});
		});
		script.expose_action(action, Some(capability(id)));

		self.settings.borrow_mut().insert(id.to_owned(), shared);
	}
}

fn capability(id: &str) -> Capability {
	Capability::Named(format!("plugin.{id}"))
}

fn report_failures(mounted: &Mounted, message: &str) {
	for mount in mounted.borrow().iter() {
		let prefix = format!("{}:", mount.id);
		if message.lines().any(|line| line.trim_start().starts_with(&prefix)) {
			tracing::warn!(plugin = %mount.id, "a plugin's interface failed and was left out");
			tracing::debug!(plugin = %mount.id, error = %message, "plugin interface error");
		}
	}
}

fn slots_of(mounted: &Mounted, id: &str) -> Vec<Slot> {
	mounted
		.borrow()
		.iter()
		.find(|mount| mount.id == id)
		.map(|mount| mount.slots.clone())
		.unwrap_or_default()
}

fn slots_json(mounts: &[Mount]) -> Json {
	let slots: Map<String, Json> = PLACED
		.into_iter()
		.map(|slot| {
			let ids: Vec<Json> = mounts
				.iter()
				.filter(|mount| mount.slots.contains(&slot))
				.map(|mount| Json::String(mount.id.clone()))
				.collect();
			(slot.name().to_owned(), Json::Array(ids))
		})
		.collect();
	Json::Object(slots)
}

fn json_value(json: &Json) -> Value {
	Value::from_json(&json.to_string()).unwrap_or(Value::Nil)
}
