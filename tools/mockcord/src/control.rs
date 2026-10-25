use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::{Instant, timeout_at};

use crate::json::parse_id;
use crate::ops::Draft;
use crate::record::Recording;
use crate::scenario::{HeartbeatAcks, Reply};
use crate::state::Shared;

#[derive(Clone)]
pub struct Control {
	shared: Arc<Shared>,
}

impl Control {
	pub(crate) fn new(shared: Arc<Shared>) -> Self {
		Self { shared }
	}

	pub fn set_recording(&self, enabled: bool) {
		self.shared.recorder.set_enabled(enabled);
	}

	pub fn clear_recording(&self) {
		self.shared.recorder.clear();
	}

	pub fn recording(&self) -> Recording {
		self.shared.recorder.snapshot()
	}

	pub fn inspect<T>(&self, read: impl FnOnce(&Recording) -> T) -> T {
		self.shared.recorder.inspect(read)
	}

	pub async fn wait_for<T>(
		&self,
		limit: Duration,
		mut found: impl FnMut(&Recording) -> Option<T>,
	) -> Option<T> {
		let deadline = Instant::now() + limit;
		let mut revisions = self.shared.recorder.subscribe();
		loop {
			revisions.borrow_and_update();
			if let Some(value) = self.inspect(&mut found) {
				return Some(value);
			}
			match timeout_at(deadline, revisions.changed()).await {
				Ok(Ok(())) => {}
				_ => return self.inspect(&mut found),
			}
		}
	}

	pub fn respond_once(&self, method: &str, pattern: &str, reply: Reply) {
		self.shared.scenario.respond(method, pattern, reply, Some(1));
	}

	pub fn respond_times(&self, method: &str, pattern: &str, reply: Reply, times: usize) {
		self.shared.scenario.respond(method, pattern, reply, Some(times));
	}

	pub fn respond_always(&self, method: &str, pattern: &str, reply: Reply) {
		self.shared.scenario.respond(method, pattern, reply, None);
	}

	pub fn clear_replies(&self) {
		self.shared.scenario.clear_replies();
	}

	pub fn close_gateway(&self, code: u16) {
		self.shared.hub.close_all(code);
	}

	pub fn close_next_identify(&self, code: u16) {
		self.shared.scenario.close_next_identify(code);
	}

	pub fn close_next_connections(&self, code: u16, count: usize) {
		self.shared.scenario.close_next_connections(code, count);
	}

	pub fn fail_next_resume(&self) {
		self.shared.scenario.fail_resumes(1);
	}

	pub fn request_reconnect(&self) {
		self.shared.hub.send_all(&json!({ "op": 7, "d": null }));
	}

	pub fn invalidate_session(&self, resumable: bool) {
		self.shared.hub.invalidate_all(resumable);
	}

	pub fn request_heartbeat(&self) {
		self.shared.hub.send_all(&json!({ "op": 1, "d": null }));
	}

	pub fn set_heartbeat_acks(&self, acks: HeartbeatAcks) {
		self.shared.scenario.set_acks(acks);
	}

	pub fn dispatch(&self, event: &str, data: &Value) {
		self.shared.hub.broadcast(event, data);
	}

	pub fn post_message(&self, channel_id: u64, author_id: u64, content: &str) -> Option<u64> {
		let author = self.shared.fixtures.users.get(&author_id)?;
		let message = self
			.shared
			.post_message(channel_id, author, Draft::text(content))
			.ok()?;
		parse_id(&message["id"])
	}

	pub fn transform_ready(&self, transform: impl Fn(&Value, &mut Value) + Send + Sync + 'static) {
		self.shared.scenario.set_ready(Some(Arc::new(transform)));
	}

	pub fn reset_ready(&self) {
		self.shared.scenario.set_ready(None);
	}
}
