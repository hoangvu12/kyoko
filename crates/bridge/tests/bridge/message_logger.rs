use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{ChannelKind, Display, MessageEntry};
use plugins::{HostConfig, Limits, Permission, PluginHost};
use protocol::Snowflake;
use serde_json::json;
use session::{DeletedLook, Formatting, HistoryLook};

use crate::harness::Running;

const ID: &str = "message-logger";
const WAIT: Duration = Duration::from_secs(15);
const EDITED: &str = "the edited words";

struct Logger {
	host: PluginHost,
	display: Display,
	root: PathBuf,
}

impl Logger {
	fn start(name: &str) -> Logger {
		let root = std::env::temp_dir().join(format!("kyoko-bridge-logger-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&root);
		let host = PluginHost::new(HostConfig {
			data_dir: Some(root.clone()),
			user_dir: None,
			development_dirs: Vec::new(),
			bundled: true,
			limits: Limits {
				load_deadline: Duration::from_secs(15),
				..Limits::default()
			},
		});
		let display = Display::new(Arc::new(host.clone()), |_| {});
		host.on_change(display.refresher());
		Logger { host, display, root }
	}

	fn approve(&self) {
		let declared: Vec<Permission> = self
			.host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == ID)
			.map(|plugin| plugin.permissions)
			.expect("MessageLogger is bundled");
		self.host.approve(ID, &declared).expect("MessageLogger starts");
	}

	fn stop(self) {
		drop(self.display);
		let _ = std::fs::remove_dir_all(&self.root);
	}
}

async fn eventually(what: &str, mut ready: impl FnMut() -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready() {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		tokio::time::sleep(Duration::from_millis(20)).await;
	}
}

fn redraw(running: &mut Running) {
	let state = running.session().state().clone();
	running.view.timeline.redraw(&state.read(), Formatting::Messages);
}

fn entry(running: &Running, id: Snowflake) -> Option<MessageEntry> {
	running
		.view
		.timeline
		.messages()
		.find(|entry| entry.id == id)
		.cloned()
}

async fn open_first_channel(running: &mut Running) -> Snowflake {
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	let command = running.open_channel(channel_id).expect("the channel opens");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().filter(|entry| !entry.mine).count() >= 2
		})
		.await;
	channel_id
}

fn two_others(running: &Running) -> (MessageEntry, MessageEntry) {
	let mut others = running
		.view
		.timeline
		.messages()
		.rev()
		.filter(|entry| entry.is_plain() && !entry.mine && !entry.content.is_empty());
	let edited = others.next().expect("a message to edit").clone();
	let deleted = others.next().expect("a message to delete").clone();
	(edited, deleted)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_logger_shows_deleted_and_edited_messages_in_the_timeline() {
	let logger = Logger::start("shows");
	let mut running = Running::start_hooked("logger-shows", Arc::new(logger.host.clone())).await;
	logger
		.host
		.attach(running.session().state().clone(), running.session().commander());
	running.view.timeline.set_display(logger.display.clone());
	let channel_id = open_first_channel(&mut running).await;
	let (edited, deleted) = two_others(&running);
	let original = bridge::snippet(&edited.body.content, &edited.names, 200);

	assert!(
		running
			.server
			.control()
			.edit_message(channel_id.get(), edited.id.get(), EDITED)
	);
	assert!(
		running
			.server
			.control()
			.delete_message(channel_id.get(), deleted.id.get())
	);
	running
		.until_state("the edit and the delete to arrive", |state| {
			state.deleted(channel_id, deleted.id).is_some()
				&& !state.edit_history(channel_id, edited.id).is_empty()
		})
		.await;
	redraw(&mut running);
	assert!(
		entry(&running, deleted.id).is_none(),
		"kyoko hides deleted messages on its own"
	);
	assert!(entry(&running, edited.id).is_some_and(|entry| entry.look.is_none() && entry.history.is_empty()));

	logger.approve();
	eventually("MessageLogger to show the deleted message and the edit", || {
		redraw(&mut running);
		let shown = entry(&running, deleted.id).is_some_and(|entry| {
			entry.deleted
				&& entry.look.as_ref().is_some_and(|look| {
					look.deleted == Some(DeletedLook::Text)
						&& look
							.accessories
							.first()
							.is_some_and(|accessory| accessory.text == "Remove from log")
				})
		});
		let history = entry(&running, edited.id).is_some_and(|entry| {
			entry.content == EDITED
				&& entry
					.look
					.as_ref()
					.is_some_and(|look| look.history == Some(HistoryLook::Inline))
				&& entry.history.len() == 1
				&& bridge::snippet(&entry.history[0], &entry.names, 200) == original
		});
		shown && history && logger.display.pending() == 0
	})
	.await;
	let calls = logger.display.look_calls();
	assert_eq!(calls, 2, "one question per kept message: {calls}");
	for _ in 0..50 {
		redraw(&mut running);
	}
	assert_eq!(
		logger.display.look_calls(),
		calls,
		"redrawing asks the plugin again"
	);

	logger
		.host
		.set_setting(ID, "deleted_style", &json!("tint"))
		.expect("a setting");
	eventually("the deleted style to change", || {
		redraw(&mut running);
		entry(&running, deleted.id).is_some_and(|entry| {
			entry
				.look
				.is_some_and(|look| look.deleted == Some(DeletedLook::Tint))
		})
	})
	.await;

	let context = [("message_id".to_owned(), deleted.id.to_string())];
	logger
		.host
		.gesture(ID, "remove", &context)
		.expect("Remove from log runs");
	eventually("Remove from log to drop the deleted message", || {
		redraw(&mut running);
		entry(&running, deleted.id).is_none()
	})
	.await;

	logger.host.disable(ID);
	eventually("kyoko's own look once MessageLogger is off", || {
		redraw(&mut running);
		entry(&running, edited.id).is_some_and(|entry| entry.look.is_none() && entry.history.is_empty())
			&& !running.session().state().read().shows_deleted_messages()
	})
	.await;
	running.stop().await;
	logger.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_history_only_keeps_deleted_messages_hidden() {
	let logger = Logger::start("edits-only");
	logger
		.host
		.set_setting(ID, "show", &json!("edits"))
		.expect("a setting");
	logger
		.host
		.set_setting(ID, "inline_edits", &json!(false))
		.expect("a setting");
	logger.approve();
	let mut running = Running::start_hooked("logger-edits-only", Arc::new(logger.host.clone())).await;
	logger
		.host
		.attach(running.session().state().clone(), running.session().commander());
	running.view.timeline.set_display(logger.display.clone());
	let channel_id = open_first_channel(&mut running).await;
	let (edited, deleted) = two_others(&running);
	assert!(
		running
			.server
			.control()
			.edit_message(channel_id.get(), edited.id.get(), EDITED)
	);
	assert!(
		running
			.server
			.control()
			.delete_message(channel_id.get(), deleted.id.get())
	);
	running
		.until_state("the edit and the delete to arrive", |state| {
			state.deleted(channel_id, deleted.id).is_some()
				&& !state.edit_history(channel_id, edited.id).is_empty()
		})
		.await;
	eventually("the collapsed edit history", || {
		redraw(&mut running);
		entry(&running, edited.id).is_some_and(|entry| {
			entry
				.look
				.as_ref()
				.is_some_and(|look| look.history == Some(HistoryLook::Collapsed))
				&& entry.history.len() == 1
		})
	})
	.await;
	assert!(entry(&running, deleted.id).is_none());
	assert!(!running.session().state().read().shows_deleted_messages());
	running.stop().await;
	logger.stop();
}
