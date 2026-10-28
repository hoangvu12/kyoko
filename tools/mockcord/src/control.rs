use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::{Instant, timeout_at};

use crate::json::{parse_id, to_json};
use crate::ops::Draft;
use crate::record::Recording;
use crate::scenario::{HeartbeatAcks, Reply};
use crate::state::Shared;
use crate::voice::MockStream;

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

	pub fn community(&self) -> Option<crate::community::CommunityGuild> {
		self.shared.community_state()
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

	pub fn hold_requests(&self, method: &str, pattern: &str) {
		self.shared.scenario.hold(method, pattern);
	}

	pub fn release_requests(&self) {
		self.shared.scenario.release_holds();
	}

	pub fn clear_replies(&self) {
		self.shared.scenario.clear_replies();
	}

	pub fn mfa_tickets_issued(&self) -> u64 {
		self.shared.password.tickets_issued()
	}

	pub fn mfa_texts_sent(&self) -> u64 {
		self.shared.password.texts_sent()
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

	pub fn split_gateway_messages(&self, parts: usize) {
		self.shared.scenario.split_gateway_messages(parts);
	}

	pub fn send_uncompressed_gateway_text(&self, text: String) {
		self.shared.hub.send_raw_all(text);
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

	pub fn rate_limit_member_requests(&self, retry_after: f64, count: usize) {
		self.shared
			.scenario
			.rate_limit_member_requests(retry_after, count);
	}

	pub fn fill_voice_channel(&self, channel_id: u64) {
		self.shared.scenario.fill_voice_channel(channel_id);
	}

	pub fn start_stream(&self, guild_id: u64, channel_id: u64, owner: u64) -> MockStream {
		let stream = self.shared.voice.start_stream(Some(guild_id), channel_id, owner);
		self.broadcast_streaming(guild_id, channel_id, owner, true);
		stream
	}

	pub fn fill_stream(&self, key: &str) {
		self.shared.voice.fill_stream(key);
	}

	pub fn end_stream(&self, key: &str, reason: &str) {
		let Some(stream) = self.shared.voice.end_stream(key) else {
			return;
		};
		self.shared.hub.broadcast(
			"STREAM_DELETE",
			&json!({ "stream_key": stream.key, "reason": reason }),
		);
		if let Some(guild_id) = stream.guild_id {
			self.broadcast_streaming(guild_id, stream.channel_id, stream.owner, false);
		}
	}

	fn broadcast_streaming(&self, guild_id: u64, channel_id: u64, user_id: u64, streaming: bool) {
		let seated = self
			.shared
			.fixtures
			.guild(guild_id)
			.and_then(|guild| guild.voice_states.iter().find(|state| state.user_id.0 == user_id))
			.map(to_json);
		let mut event = seated.unwrap_or_else(|| {
			json!({
				"guild_id": guild_id.to_string(),
				"user_id": user_id.to_string(),
				"session_id": format!("mock-stream-{user_id}"),
				"deaf": false,
				"mute": false,
				"self_deaf": false,
				"self_mute": false,
				"self_video": false,
				"suppress": false,
			})
		});
		event["channel_id"] = json!(channel_id.to_string());
		event["self_stream"] = json!(streaming);
		self.shared.hub.broadcast("VOICE_STATE_UPDATE", &event);
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

	pub fn add_unjoined_thread(&self, parent_id: u64, name: &str) -> Option<u64> {
		self.shared.add_unjoined_thread(parent_id, name)
	}

	pub fn edit_message(&self, channel_id: u64, id: u64, content: &str) -> bool {
		self.shared.edit_any_message(channel_id, id, content).is_ok()
	}

	pub fn delete_message(&self, channel_id: u64, id: u64) -> bool {
		self.shared.delete_message(channel_id, id).is_ok()
	}

	pub fn react_as(&self, channel_id: u64, message_id: u64, user_id: u64, emoji: &str, burst: bool) -> bool {
		let react = crate::reactions::React {
			channel_id,
			message_id,
			emoji,
			user_id,
			burst,
		};
		self.shared.react(react, true).is_ok()
	}

	pub fn unreact_as(
		&self,
		channel_id: u64,
		message_id: u64,
		user_id: u64,
		emoji: &str,
		burst: bool,
	) -> bool {
		let react = crate::reactions::React {
			channel_id,
			message_id,
			emoji,
			user_id,
			burst,
		};
		self.shared.react(react, false).is_ok()
	}

	pub fn poll_vote_as(
		&self,
		channel_id: u64,
		message_id: u64,
		user_id: u64,
		answer: i64,
		added: bool,
	) -> bool {
		self.shared
			.poll_vote_as(channel_id, message_id, user_id, answer, added)
			.is_ok()
	}

	pub fn bulk_delete(&self, channel_id: u64, ids: &[u64]) -> bool {
		self.shared.bulk_delete(channel_id, ids).is_ok()
	}

	pub fn transform_ready(&self, transform: impl Fn(&Value, &mut Value) + Send + Sync + 'static) {
		self.shared.scenario.set_ready(Some(Arc::new(transform)));
	}

	pub fn reset_ready(&self) {
		self.shared.scenario.set_ready(None);
	}

	pub fn ban_users(&self, guild_id: u64, user_ids: &[u64]) {
		self.shared.ban_users(guild_id, user_ids);
	}

	pub fn favorite_emojis(&self) -> Vec<String> {
		self.shared.store.lock().favorite_emojis.clone()
	}

	pub fn set_favorite_emojis(&self, favorites: &[&str]) {
		self.shared.store.lock().favorite_emojis =
			favorites.iter().map(|favorite| (*favorite).to_owned()).collect();
	}

	pub fn update_favorite_emojis(&self, favorites: &[&str]) {
		self.set_favorite_emojis(favorites);
		let encoded = {
			let store = self.shared.store.lock();
			protocol::encode_frecency(&store.favorite_emojis, &store.favorite_gifs, &store.command_uses)
		};
		self.shared.hub.broadcast(
			"USER_SETTINGS_PROTO_UPDATE",
			&json!({ "settings": { "type": 2, "proto": encoded }, "partial": false }),
		);
	}

	pub fn set_autocomplete(&self, reply: crate::config::AutocompleteReply) {
		self.shared.store.lock().autocomplete = reply;
	}

	pub fn set_failing_modals(&self, failing: bool) {
		self.shared.store.lock().failing_modals = failing;
	}

	pub fn post_components(&self, channel_id: u64, content: &str, components: Vec<Value>) -> Option<u64> {
		let draft = Draft {
			components,
			..Draft::text(content)
		};
		let message = self
			.shared
			.post_message(channel_id, &crate::rest::mockbot(), draft)
			.ok()?;
		parse_id(&message["id"])
	}

	pub fn set_interaction_timing(&self, timing: crate::config::InteractionTiming) {
		self.shared.store.lock().interactions = timing;
	}

	pub fn command_uses(&self) -> Vec<protocol::CommandUse> {
		self.shared.store.lock().command_uses.clone()
	}

	pub fn scan_qr_code(&self) {
		self.shared.remote_auth.scan();
	}

	pub fn confirm_qr_login(&self) {
		self.shared.remote_auth.confirm();
	}

	pub fn cancel_qr_login(&self) {
		self.shared.remote_auth.cancel();
	}

	pub fn expire_qr_code(&self) {
		self.shared.remote_auth.expire();
	}

	pub fn garble_next_qr_nonce(&self) {
		self.shared.remote_auth.garble_next_nonce();
	}

	pub fn qr_connections(&self) -> Vec<crate::record::Headers> {
		self.shared.remote_auth.connections()
	}

	pub fn qr_codes_shown(&self) -> usize {
		self.shared.remote_auth.waiting_phones()
	}

	pub fn favorite_gifs(&self) -> Vec<protocol::FavoriteGif> {
		self.shared.store.lock().favorite_gifs.clone()
	}

	pub fn add_audit_entries(&self, guild_id: u64, count: usize) {
		self.shared.add_audit_entries(guild_id, count);
	}
}
