use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use plugins::{HostConfig, Limits, Permission, PluginHost};
use protocol::Snowflake;
use session::Command;

use crate::harness::{Options, Running};

pub struct Lab {
	pub root: PathBuf,
	pub host: PluginHost,
	pub running: Running,
}

pub fn scratch(name: &str) -> PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-plugins-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	std::fs::create_dir_all(root.join("dev")).expect("scratch dir");
	root
}

pub fn write_plugin(root: &Path, id: &str, permissions: &[&str], extra: &str, logic: &str) -> PathBuf {
	let dir = root.join("dev").join(id);
	std::fs::create_dir_all(&dir).expect("plugin dir");
	let permissions: Vec<String> = permissions
		.iter()
		.map(|permission| format!("{permission:?}"))
		.collect();
	let manifest = format!(
		"id = {id:?}\nname = {id:?}\nversion = \"1.0.0\"\napi = 1\npermissions = [{}]\n{extra}\n",
		permissions.join(", ")
	);
	std::fs::write(dir.join("logic.luau"), logic).expect("logic");
	std::fs::write(dir.join("plugin.toml"), manifest).expect("manifest");
	dir
}

const LOAD_DEADLINE: Duration = Duration::from_secs(15);

pub fn config(root: &Path, limits: Limits, bundled: bool) -> HostConfig {
	HostConfig {
		data_dir: Some(root.join("data")),
		user_dir: None,
		development_dirs: vec![root.join("dev")],
		bundled,
		limits: Limits {
			load_deadline: LOAD_DEADLINE,
			..limits
		},
	}
}

pub async fn start(name: &'static str, root: PathBuf, limits: Limits, bundled: bool) -> Lab {
	let host = PluginHost::new(config(&root, limits, bundled));
	let running = Running::start_with(Options::new(name).hooks(Arc::new(host.clone()))).await;
	host.attach(running.session().state().clone(), running.session().commander());
	Lab { root, host, running }
}

pub fn approve(host: &PluginHost, id: &str) {
	let declared: Vec<Permission> = host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == id)
		.map(|plugin| plugin.permissions)
		.expect("the plugin is loaded");
	host.approve(id, &declared).expect("the plugin is approved");
}

impl Lab {
	pub async fn stop(self) {
		self.running.stop().await;
		let _ = std::fs::remove_dir_all(&self.root);
	}

	pub async fn edit_and_wait(&self, channel_id: Snowflake, message_id: Snowflake, sent: &str, shown: &str) {
		self.running.session().send(Command::EditMessage {
			channel_id,
			message_id,
			content: sent.to_owned(),
		});
		self.running
			.until("the edit to arrive", |state| {
				state
					.messages(channel_id)
					.and_then(|cache| cache.by_id.get(&message_id))
					.is_some_and(|message| message.content == shown)
			})
			.await;
	}
}
