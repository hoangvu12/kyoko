use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use protocol::Snowflake;
use serde_json::{Map, Value as Json, json};
use session::Commander;
use state::SharedState;

use crate::bundled;
use crate::calls::{Call, Fault, Reply};
use crate::context::{ActionLog, Attachment, Link, Settings};
use crate::limits::Limits;
use crate::manifest::{SettingKind, SettingSpec};
use crate::package::{Origin, Package, Stamp, package_dirs};
use crate::permission::{Hook, Permission};
use crate::worker::{Setup, Worker};

const STATE_FILE: &str = "plugins.json";
const DATA_DIR: &str = "plugin-data";
const STORAGE_FILE: &str = "storage.json";
const USER_DIR: &str = "plugins";
const ACTION_KEY_LIMIT: usize = 48;
const CONTEXT_LIMIT: usize = 16;

#[derive(Clone, Debug)]
pub struct HostConfig {
	pub data_dir: Option<PathBuf>,
	pub user_dir: Option<PathBuf>,
	pub development_dirs: Vec<PathBuf>,
	pub bundled: bool,
	pub limits: Limits,
}

impl HostConfig {
	pub fn for_data_dir(data_dir: &Path) -> HostConfig {
		HostConfig {
			data_dir: Some(data_dir.to_path_buf()),
			user_dir: Some(data_dir.join(USER_DIR)),
			development_dirs: Vec::new(),
			bundled: true,
			limits: Limits::default(),
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
	Disabled,
	NeedsApproval,
	Running,
	Suspended { until: Instant },
	Stopped,
	Failed(String),
	Incompatible,
}

impl Status {
	pub fn name(&self) -> &'static str {
		match self {
			Status::Disabled => "disabled",
			Status::NeedsApproval => "approval",
			Status::Running => "running",
			Status::Suspended { .. } => "suspended",
			Status::Stopped => "stopped",
			Status::Failed(_) => "failed",
			Status::Incompatible => "incompatible",
		}
	}
}

#[derive(Clone, Debug)]
pub struct PluginInfo {
	pub id: String,
	pub name: String,
	pub version: String,
	pub description: String,
	pub authors: Vec<String>,
	pub origin: Origin,
	pub status: Status,
	pub enabled: bool,
	pub permissions: Vec<Permission>,
	pub needs_approval: bool,
	pub settings: Vec<SettingSpec>,
	pub values: Map<String, Json>,
	pub notice: Option<String>,
	pub ui_entry: Option<PathBuf>,
	pub grants: Vec<Permission>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
	pub plugin: String,
	pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnableError {
	Unknown,
	Incompatible,
	NeedsApproval(Vec<Permission>),
	Changed,
}

impl std::fmt::Display for EnableError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			EnableError::Unknown => formatter.write_str("no such plugin"),
			EnableError::Incompatible => formatter.write_str("the plugin needs another version of Kyoko"),
			EnableError::NeedsApproval(_) => formatter.write_str("the plugin's permissions need approval"),
			EnableError::Changed => {
				formatter.write_str("the plugin's permissions changed; review them again")
			}
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
enum Run {
	Idle,
	Running,
	Suspended(Instant),
	Stopped,
	Failed(String),
}

struct Entry {
	package: Package,
	stamp: Stamp,
	enabled: bool,
	approved: BTreeSet<Permission>,
	settings: Settings,
	actions: ActionLog,
	run: Run,
	strikes: VecDeque<Instant>,
	suspensions: u32,
	notice: Option<String>,
	worker: Option<Arc<Worker>>,
}

impl Entry {
	fn name(&self) -> &str {
		&self.package.manifest.name
	}

	fn declared(&self) -> BTreeSet<Permission> {
		self.package
			.manifest
			.permissions
			.iter()
			.filter(|permission| permission.is_known())
			.cloned()
			.collect()
	}

	fn needs_approval(&self) -> bool {
		!self.declared().is_subset(&self.approved)
	}

	fn grants(&self) -> BTreeSet<Permission> {
		self.declared().intersection(&self.approved).cloned().collect()
	}

	fn status(&self) -> Status {
		if !self.package.manifest.compatible() {
			return Status::Incompatible;
		}
		match &self.run {
			Run::Failed(error) => Status::Failed(error.clone()),
			Run::Stopped => Status::Stopped,
			_ if !self.enabled => Status::Disabled,
			_ if self.needs_approval() => Status::NeedsApproval,
			Run::Suspended(until) => Status::Suspended { until: *until },
			Run::Running => Status::Running,
			Run::Idle => Status::Disabled,
		}
	}

	fn stop(&mut self) {
		if let Some(worker) = self.worker.take() {
			worker.stop();
		}
	}

	fn saved(&self) -> Json {
		json!({
			"enabled": self.enabled,
			"approved": self.approved.iter().map(Permission::name).collect::<Vec<_>>(),
			"settings": Json::Object(self.settings.read().clone()),
		})
	}
}

#[derive(Default)]
struct Registry {
	entries: BTreeMap<String, Entry>,
	saved: Map<String, Json>,
}

type Listener = Arc<dyn Fn() + Send + Sync>;

struct Inner {
	config: HostConfig,
	registry: Mutex<Registry>,
	link: Link,
	listeners: Mutex<Vec<Listener>>,
	notices: Mutex<Vec<Notice>>,
}

#[derive(Clone)]
pub struct PluginHost {
	inner: Arc<Inner>,
}

impl PluginHost {
	pub fn new(config: HostConfig) -> PluginHost {
		let saved = config
			.data_dir
			.as_ref()
			.and_then(|dir| std::fs::read(dir.join(STATE_FILE)).ok())
			.and_then(|bytes| serde_json::from_slice::<Json>(&bytes).ok())
			.and_then(|json| json.get("plugins").and_then(Json::as_object).cloned())
			.unwrap_or_default();
		let host = PluginHost {
			inner: Arc::new(Inner {
				config,
				registry: Mutex::new(Registry {
					entries: BTreeMap::new(),
					saved,
				}),
				link: Arc::new(RwLock::new(None)),
				listeners: Mutex::new(Vec::new()),
				notices: Mutex::new(Vec::new()),
			}),
		};
		host.scan();
		host
	}

	pub fn limits(&self) -> &Limits {
		&self.inner.config.limits
	}

	pub fn user_dir(&self) -> Option<&Path> {
		self.inner.config.user_dir.as_deref()
	}

	pub fn attach(&self, state: SharedState, commander: Commander) {
		*self.inner.link.write() = Some(Attachment { state, commander });
	}

	pub fn detach(&self) {
		*self.inner.link.write() = None;
	}

	pub fn on_change(&self, listener: impl Fn() + Send + Sync + 'static) {
		self.inner.listeners.lock().push(Arc::new(listener));
	}

	pub fn take_notices(&self) -> Vec<Notice> {
		std::mem::take(&mut *self.inner.notices.lock())
	}

	pub fn snapshot(&self) -> Vec<PluginInfo> {
		let registry = self.inner.registry.lock();
		registry
			.entries
			.values()
			.map(|entry| {
				let manifest = &entry.package.manifest;
				PluginInfo {
					id: manifest.id.clone(),
					name: manifest.name.clone(),
					version: manifest.version.clone(),
					description: manifest.description.clone(),
					authors: manifest.authors.clone(),
					origin: entry.package.origin,
					status: entry.status(),
					enabled: entry.enabled,
					permissions: manifest.permissions.clone(),
					needs_approval: entry.needs_approval(),
					settings: manifest.settings.clone(),
					values: entry.settings.read().clone(),
					notice: entry.notice.clone(),
					ui_entry: entry.package.ui_entry(),
					grants: entry.grants().into_iter().collect(),
				}
			})
			.collect()
	}

	pub fn status(&self, id: &str) -> Option<Status> {
		self.inner.registry.lock().entries.get(id).map(Entry::status)
	}

	pub fn enable(&self, id: &str) -> Result<(), EnableError> {
		let result = {
			let mut registry = self.inner.registry.lock();
			let entry = registry.entries.get_mut(id).ok_or(EnableError::Unknown)?;
			if !entry.package.manifest.compatible() {
				return Err(EnableError::Incompatible);
			}
			if entry.needs_approval() {
				let missing = entry.declared().difference(&entry.approved).cloned().collect();
				return Err(EnableError::NeedsApproval(missing));
			}
			entry.enabled = true;
			entry.suspensions = 0;
			entry.strikes.clear();
			entry.notice = None;
			self.inner.start(entry);
			self.inner.save(&mut registry);
			Ok(())
		};
		self.inner.changed();
		result
	}

	pub fn approve(&self, id: &str, shown: &[Permission]) -> Result<(), EnableError> {
		{
			let mut registry = self.inner.registry.lock();
			let entry = registry.entries.get_mut(id).ok_or(EnableError::Unknown)?;
			let shown: BTreeSet<Permission> = shown
				.iter()
				.filter(|permission| permission.is_known())
				.cloned()
				.collect();
			if shown != entry.declared() {
				return Err(EnableError::Changed);
			}
			entry.approved = shown;
			tracing::info!(plugin = %id, "plugin permissions approved");
		}
		self.enable(id)
	}

	pub fn disable(&self, id: &str) {
		{
			let mut registry = self.inner.registry.lock();
			let Some(entry) = registry.entries.get_mut(id) else {
				return;
			};
			entry.enabled = false;
			entry.stop();
			entry.run = Run::Idle;
			entry.suspensions = 0;
			entry.strikes.clear();
			entry.notice = None;
			self.inner.save(&mut registry);
		}
		self.inner.changed();
	}

	pub fn revoke(&self, id: &str) {
		{
			let mut registry = self.inner.registry.lock();
			let Some(entry) = registry.entries.get_mut(id) else {
				return;
			};
			entry.approved.clear();
			entry.enabled = false;
			entry.stop();
			entry.run = Run::Idle;
			self.inner.save(&mut registry);
		}
		self.inner.changed();
	}

	pub fn reload(&self, id: &str) {
		{
			let mut registry = self.inner.registry.lock();
			let Some(entry) = registry.entries.get_mut(id) else {
				return;
			};
			let reopened = entry
				.package
				.dir()
				.map(|dir| Package::open(dir, entry.package.origin));
			match reopened {
				Some(Err(error)) => {
					entry.stop();
					entry.run = Run::Failed(error);
				}
				reopened => {
					if let Some(Ok(package)) = reopened {
						entry.stamp = package.stamp();
						entry.package = package;
					}
					entry.suspensions = 0;
					entry.strikes.clear();
					entry.notice = None;
					self.inner.start(entry);
				}
			}
		}
		self.inner.changed();
	}

	pub fn set_setting(&self, id: &str, key: &str, value: &Json) -> Result<Json, String> {
		let (accepted, worker) = {
			let mut registry = self.inner.registry.lock();
			let entry = registry
				.entries
				.get_mut(id)
				.ok_or_else(|| "no such plugin".to_owned())?;
			let spec = entry
				.package
				.manifest
				.setting(key)
				.ok_or_else(|| format!("the plugin has no setting {key}"))?;
			let accepted = spec
				.accept(value)
				.ok_or_else(|| format!("{value} is not a valid value for {key}"))?;
			entry.settings.write().insert(key.to_owned(), accepted.clone());
			let worker = entry.worker.clone().filter(|_| entry.run == Run::Running);
			self.inner.save(&mut registry);
			(accepted, worker)
		};
		if let Some(worker) = worker.filter(|worker| worker.defines("on_settings_changed")) {
			let limit = self.inner.config.limits.event_deadline;
			if let Err(fault) = worker.call(Call::SettingsChanged, limit) {
				self.inner.strike(id, fault);
			}
		}
		self.inner.changed();
		Ok(accepted)
	}

	pub fn gesture(&self, id: &str, key: &str, context: &[(String, String)]) -> Result<(), String> {
		if key.is_empty() || key.len() > ACTION_KEY_LIMIT {
			return Err("the action key is not valid".to_owned());
		}
		let worker = {
			let mut registry = self.inner.registry.lock();
			let entry = registry
				.entries
				.get_mut(id)
				.ok_or_else(|| "no such plugin".to_owned())?;
			self.inner.revive(entry, Instant::now());
			if entry.status() != Status::Running {
				return Err(format!("{} is not running", entry.name()));
			}
			entry
				.worker
				.clone()
				.ok_or_else(|| "the plugin has no logic".to_owned())?
		};
		let context = context.iter().take(CONTEXT_LIMIT).cloned().collect();
		let call = Call::Action {
			key: key.to_owned(),
			context,
		};
		match worker.call(call, self.inner.config.limits.action_deadline) {
			Ok(_) => Ok(()),
			Err(fault) => {
				let reason = fault.describe();
				self.inner.strike(id, fault);
				Err(reason)
			}
		}
	}

	pub fn scan(&self) -> bool {
		let changed = self.inner.scan();
		if changed {
			self.inner.changed();
		}
		changed
	}

	pub fn watch(&self, every: Duration) {
		let weak = Arc::downgrade(&self.inner);
		let spawned = std::thread::Builder::new()
			.name("plugin-watch".to_owned())
			.spawn(move || {
				loop {
					std::thread::sleep(every);
					let Some(inner) = weak.upgrade() else {
						return;
					};
					let mut changed = inner.scan();
					changed |= inner.revive_due();
					if changed {
						inner.changed();
					}
				}
			});
		if let Err(error) = spawned {
			tracing::warn!(%error, "plugin hot reload did not start");
		}
	}

	pub(crate) fn rewrite(&self, call: impl Fn(&str) -> Call, original: &str) -> Option<(String, String)> {
		let deadline = self.inner.config.limits.send_deadline;
		let mut text = original.to_owned();
		let mut by = None;
		for target in self.inner.targets(Hook::Send, call(&text).hook_name(), true) {
			match target.worker.call(call(&text), deadline) {
				Ok(Reply::Text(Some(rewritten))) => {
					text = rewritten;
					by = Some(target.name);
				}
				Ok(_) => {}
				Err(fault) => self.inner.strike(&target.id, fault),
			}
		}
		by.filter(|_| text != original).map(|by| (text, by))
	}

	pub(crate) fn allow(&self, hook: Hook, call: Call) -> bool {
		let deadline = self.inner.config.limits.veto_deadline;
		for target in self.inner.targets(hook, call.hook_name(), true) {
			match target.worker.call(call.clone(), deadline) {
				Ok(Reply::Allow(false)) => {
					tracing::debug!(plugin = %target.id, hook = call.hook_name(), "a plugin vetoed");
					return false;
				}
				Ok(_) => {}
				Err(fault) => self.inner.strike(&target.id, fault),
			}
		}
		true
	}

	pub(crate) fn deliver(&self, event: crate::calls::PluginEvent) {
		let registry = self.inner.registry.lock();
		for entry in registry.entries.values() {
			if entry.run != Run::Running {
				continue;
			}
			let Some(worker) = entry
				.worker
				.as_ref()
				.filter(|worker| worker.defines(crate::calls::ON_EVENT))
			else {
				continue;
			};
			let grants = entry.grants();
			let readable = event
				.scope()
				.is_none_or(|scope| grants.contains(&Permission::Read(scope)));
			let direct = !event.is_direct()
				|| grants.contains(&Permission::Read(crate::permission::Scope::DirectMessages));
			if readable && direct && !worker.deliver(event.clone()) {
				tracing::debug!(plugin = %entry.package.id(), event = event.name(), "a plugin event was dropped");
			}
		}
	}

	pub(crate) fn guild_of(&self, channel_id: Snowflake) -> Option<Snowflake> {
		let attachment = self.inner.link.read().clone()?;
		let state = attachment.state.read();
		state.channel_guild(channel_id)
	}
}

struct Target {
	id: String,
	name: String,
	worker: Arc<Worker>,
}

impl Inner {
	fn changed(&self) {
		let listeners: Vec<Listener> = self.listeners.lock().clone();
		for listener in listeners {
			listener();
		}
	}

	fn targets(self: &Arc<Self>, hook: Hook, name: &str, revive: bool) -> Vec<Target> {
		let mut registry = self.registry.lock();
		let now = Instant::now();
		let mut revived = false;
		let targets = registry
			.entries
			.values_mut()
			.filter_map(|entry| {
				if revive {
					revived |= self.revive(entry, now);
				}
				if entry.status() != Status::Running || !entry.grants().contains(&Permission::Hook(hook)) {
					return None;
				}
				let worker = entry.worker.clone().filter(|worker| worker.defines(name))?;
				Some(Target {
					id: entry.package.id().to_owned(),
					name: entry.name().to_owned(),
					worker,
				})
			})
			.collect();
		drop(registry);
		if revived {
			self.changed();
		}
		targets
	}

	fn revive(self: &Arc<Self>, entry: &mut Entry, now: Instant) -> bool {
		let Run::Suspended(until) = entry.run else {
			return false;
		};
		if now < until || !entry.enabled {
			return false;
		}
		tracing::info!(plugin = %entry.package.id(), "a suspended plugin starts again");
		self.start(entry);
		if entry.run == Run::Running {
			entry.notice = None;
		}
		true
	}

	fn revive_due(self: &Arc<Self>) -> bool {
		let mut registry = self.registry.lock();
		let now = Instant::now();
		let mut revived = false;
		for entry in registry.entries.values_mut() {
			revived |= self.revive(entry, now);
		}
		revived
	}

	fn start(self: &Arc<Self>, entry: &mut Entry) {
		entry.stop();
		if !entry.enabled || entry.needs_approval() || !entry.package.manifest.compatible() {
			entry.run = Run::Idle;
			return;
		}
		if entry.package.logic_source().is_none() {
			entry.run = Run::Running;
			return;
		}
		let id = entry.package.id().to_owned();
		let weak: Weak<Inner> = Arc::downgrade(self);
		let faulted = id.clone();
		let setup = Setup {
			package: entry.package.clone(),
			grants: entry.grants(),
			settings: entry.settings.clone(),
			link: self.link.clone(),
			limits: self.config.limits,
			actions: entry.actions.clone(),
			storage: self
				.config
				.data_dir
				.as_ref()
				.map(|dir| dir.join(DATA_DIR).join(&id).join(STORAGE_FILE)),
			faults: Arc::new(move |fault| {
				if let Some(inner) = weak.upgrade() {
					inner.strike(&faulted, fault);
				}
			}),
		};
		match Worker::start(setup) {
			Ok(worker) => {
				tracing::info!(plugin = %id, "plugin started");
				entry.worker = Some(Arc::new(worker));
				entry.run = Run::Running;
			}
			Err(error) => {
				tracing::warn!(plugin = %id, "plugin did not start");
				let text = format!("{} could not start: {error}", entry.name());
				entry.notice = Some(text.clone());
				self.notices.lock().push(Notice { plugin: id, text });
				entry.run = Run::Failed(error);
			}
		}
	}

	fn strike(self: &Arc<Self>, id: &str, fault: Fault) {
		let changed = {
			let mut registry = self.registry.lock();
			let Some(entry) = registry.entries.get_mut(id) else {
				return;
			};
			if entry.run != Run::Running {
				return;
			}
			tracing::warn!(plugin = %id, fault = fault_kind(&fault), "plugin call failed");
			if let Fault::Error(error) = &fault {
				tracing::debug!(plugin = %id, %error, "plugin error");
			}
			let now = Instant::now();
			let limits = self.config.limits;
			entry.strikes.push_back(now);
			while entry
				.strikes
				.front()
				.is_some_and(|at| now.duration_since(*at) > limits.strike_window)
			{
				entry.strikes.pop_front();
			}
			let suspend = fault == Fault::Gone || entry.strikes.len() >= limits.strikes;
			if suspend {
				self.suspend(entry, &fault.describe(), now);
				self.save(&mut registry);
			}
			suspend
		};
		if changed {
			self.changed();
		}
	}

	fn suspend(&self, entry: &mut Entry, reason: &str, now: Instant) {
		entry.stop();
		entry.strikes.clear();
		entry.suspensions += 1;
		let limits = self.config.limits;
		let text = if entry.suspensions >= limits.suspensions {
			entry.run = Run::Stopped;
			entry.enabled = false;
			tracing::warn!(plugin = %entry.package.id(), "plugin turned off after repeated failures");
			format!(
				"{} was turned off because {reason}, again and again. You can turn it back on in Settings, Plugins.",
				entry.name()
			)
		} else {
			let wait = limits.backoff_after(entry.suspensions);
			entry.run = Run::Suspended(now + wait);
			tracing::warn!(plugin = %entry.package.id(), wait_s = wait.as_secs(), "plugin suspended");
			format!(
				"{} was stopped because {reason}. It starts again in {}.",
				entry.name(),
				spoken(wait)
			)
		};
		entry.notice = Some(text.clone());
		self.notices.lock().push(Notice {
			plugin: entry.package.id().to_owned(),
			text,
		});
	}

	fn save(&self, registry: &mut Registry) {
		let Some(dir) = &self.config.data_dir else {
			return;
		};
		let ids: Vec<String> = registry.entries.keys().cloned().collect();
		for id in ids {
			if let Some(entry) = registry.entries.get(&id) {
				registry.saved.insert(id, entry.saved());
			}
		}
		let document = json!({ "version": 1, "plugins": Json::Object(registry.saved.clone()) });
		let bytes = serde_json::to_vec_pretty(&document).unwrap_or_default();
		let _ = std::fs::create_dir_all(dir);
		if let Err(error) = store::write_atomic(&dir.join(STATE_FILE), &bytes) {
			tracing::warn!(%error, "plugin settings were not saved");
		}
	}

	fn discover(&self) -> Vec<Package> {
		let mut found: Vec<Package> = Vec::new();
		if self.config.bundled {
			found.extend(bundled::packages());
		}
		let bundled_ids: BTreeSet<String> = found.iter().map(|package| package.id().to_owned()).collect();
		if let Some(user_dir) = &self.config.user_dir {
			for dir in package_dirs(user_dir) {
				match Package::open(&dir, Origin::User) {
					Ok(package) if bundled_ids.contains(package.id()) => {
						tracing::warn!(plugin = %package.id(), "a user plugin may not replace a bundled one");
					}
					Ok(package) if found.iter().any(|known| known.id() == package.id()) => {
						tracing::warn!(plugin = %package.id(), "two user plugins share an id; the first is kept");
					}
					Ok(package) => found.push(package),
					Err(error) => tracing::warn!(%error, dir = %dir.display(), "a user plugin was left out"),
				}
			}
		}
		for root in &self.config.development_dirs {
			let dirs = if root.join(crate::manifest::MANIFEST_FILE).is_file() {
				vec![root.clone()]
			} else {
				package_dirs(root)
			};
			for dir in dirs {
				match Package::open(&dir, Origin::Development) {
					Ok(package) => {
						found.retain(|known| known.id() != package.id());
						found.push(package);
					}
					Err(error) => {
						tracing::warn!(%error, dir = %dir.display(), "a development plugin was left out")
					}
				}
			}
		}
		found
	}

	fn scan(self: &Arc<Self>) -> bool {
		let packages = self.discover();
		let mut registry = self.registry.lock();
		let mut changed = false;
		let ids: BTreeSet<String> = packages.iter().map(|package| package.id().to_owned()).collect();
		let gone: Vec<String> = registry
			.entries
			.keys()
			.filter(|id| !ids.contains(*id))
			.cloned()
			.collect();
		for id in gone {
			if let Some(mut entry) = registry.entries.remove(&id) {
				entry.stop();
				tracing::info!(plugin = %id, "plugin removed");
				changed = true;
			}
		}
		for package in packages {
			let id = package.id().to_owned();
			let stamp = package.stamp();
			let same = registry
				.entries
				.get(&id)
				.is_some_and(|entry| entry.package.origin == package.origin && entry.stamp == stamp);
			if same {
				continue;
			}
			changed = true;
			let saved = registry.saved.get(&id).cloned().unwrap_or(Json::Null);
			match registry.entries.get_mut(&id) {
				Some(entry) => {
					tracing::info!(plugin = %id, "plugin files changed, reloading");
					let values = entry.settings.read().clone();
					*entry.settings.write() = settings_for(&package.manifest.settings, &Json::Object(values));
					entry.package = package;
					entry.stamp = stamp;
					entry.run = Run::Idle;
					entry.notice = None;
					entry.strikes.clear();
					self.start(entry);
				}
				None => {
					let mut entry = Entry {
						settings: Arc::new(RwLock::new(settings_for(
							&package.manifest.settings,
							saved.get("settings").unwrap_or(&Json::Null),
						))),
						enabled: saved.get("enabled").and_then(Json::as_bool).unwrap_or(false),
						approved: saved
							.get("approved")
							.and_then(Json::as_array)
							.into_iter()
							.flatten()
							.filter_map(Json::as_str)
							.map(Permission::parse)
							.filter(Permission::is_known)
							.collect(),
						package,
						stamp,
						actions: ActionLog::default(),
						run: Run::Idle,
						strikes: VecDeque::new(),
						suspensions: 0,
						notice: None,
						worker: None,
					};
					self.start(&mut entry);
					registry.entries.insert(id, entry);
				}
			}
		}
		changed
	}
}

fn settings_for(specs: &[SettingSpec], saved: &Json) -> Map<String, Json> {
	specs
		.iter()
		.filter(|spec| !matches!(spec.kind, SettingKind::Action))
		.map(|spec| {
			let value = saved
				.get(&spec.key)
				.and_then(|value| spec.accept(value))
				.unwrap_or_else(|| spec.default_value());
			(spec.key.clone(), value)
		})
		.collect()
}

fn fault_kind(fault: &Fault) -> &'static str {
	match fault {
		Fault::Error(_) => "error",
		Fault::Overrun => "overrun",
		Fault::Busy => "busy",
		Fault::Gone => "gone",
	}
}

fn spoken(wait: Duration) -> String {
	let seconds = wait.as_secs().max(1);
	match seconds {
		1 => "1 second".to_owned(),
		2..60 => format!("{seconds} seconds"),
		60..120 => "1 minute".to_owned(),
		_ => format!("{} minutes", seconds / 60),
	}
}
