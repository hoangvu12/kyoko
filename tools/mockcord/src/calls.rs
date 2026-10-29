use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use protocol::{CallInfo, LenientVec, Message, MessageType, Snowflake};
use serde_json::{Value, json};

use crate::fixtures::CURRENT_USER_ID;
use crate::json::to_json;
use crate::state::Shared;

pub const CALL_REGION: &str = "mock-east";
const ANSWER_DELAY: Duration = Duration::from_millis(1_500);
const ELSEWHERE_SESSION: &str = "mock-elsewhere";
const RING_AFTER_READY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockCall {
	pub channel_id: u64,
	pub message_id: u64,
	pub ringing: Vec<u64>,
	pub members: Vec<u64>,
	pub participants: Vec<u64>,
	pub started: DateTime<Utc>,
}

#[derive(Default)]
pub struct Calls {
	calls: Mutex<HashMap<u64, Live>>,
	rang: AtomicBool,
}

struct Live {
	call: MockCall,
	states: BTreeMap<u64, Value>,
}

impl Live {
	fn create_event(&self) -> Value {
		json!({
			"channel_id": self.call.channel_id.to_string(),
			"message_id": self.call.message_id.to_string(),
			"region": CALL_REGION,
			"ringing": ids(&self.call.ringing),
			"voice_states": self.states.values().cloned().collect::<Vec<_>>(),
		})
	}

	fn update_event(&self) -> Value {
		json!({
			"channel_id": self.call.channel_id.to_string(),
			"message_id": self.call.message_id.to_string(),
			"region": CALL_REGION,
			"ringing": ids(&self.call.ringing),
		})
	}
}

fn ids(users: &[u64]) -> Vec<String> {
	users.iter().map(u64::to_string).collect()
}

pub fn call_voice_state(channel_id: Option<u64>, user_id: u64, session_id: &str) -> Value {
	json!({
		"guild_id": null,
		"channel_id": channel_id.map(|id| id.to_string()),
		"user_id": user_id.to_string(),
		"session_id": session_id,
		"deaf": false,
		"mute": false,
		"self_deaf": false,
		"self_mute": false,
		"self_stream": false,
		"self_video": false,
		"suppress": false,
	})
}

fn scripted_session(user_id: u64) -> String {
	format!("mock-call-{user_id}")
}

impl Calls {
	pub fn get(&self, channel_id: u64) -> Option<MockCall> {
		self.calls.lock().get(&channel_id).map(|live| live.call.clone())
	}

	pub fn creates(&self) -> Vec<Value> {
		self.calls.lock().values().map(Live::create_event).collect()
	}

	fn channel_of(&self, user_id: u64) -> Option<u64> {
		self.calls
			.lock()
			.values()
			.find(|live| live.call.members.contains(&user_id))
			.map(|live| live.call.channel_id)
	}
}

impl Shared {
	pub fn is_private_channel(&self, channel_id: u64) -> bool {
		self.store.lock().private_channel_ids.contains(&channel_id)
	}

	fn recipients(&self, channel_id: u64) -> Vec<u64> {
		self.store
			.lock()
			.channels
			.get(&channel_id)
			.map(|channel| channel.recipient_ids.iter().map(|id| id.0).collect())
			.unwrap_or_default()
	}

	pub fn call_voice_moved(&self, user_id: u64, channel_id: Option<u64>, state: &Value) {
		let previous = self.calls.channel_of(user_id);
		if let Some(previous) = previous.filter(|previous| Some(*previous) != channel_id) {
			self.leave_call(previous, user_id);
		}
		if let Some(channel_id) = channel_id.filter(|id| self.is_private_channel(*id)) {
			self.join_call(channel_id, user_id, state.clone());
		}
	}

	fn join_call(&self, channel_id: u64, user_id: u64, state: Value) {
		let mut calls = self.calls.calls.lock();
		if let Some(live) = calls.get_mut(&channel_id) {
			let rung = live.call.ringing.contains(&user_id);
			live.call.ringing.retain(|id| *id != user_id);
			if !live.call.members.contains(&user_id) {
				live.call.members.push(user_id);
			}
			if !live.call.participants.contains(&user_id) {
				live.call.participants.push(user_id);
			}
			live.states.insert(user_id, state);
			let update = live.update_event();
			drop(calls);
			if rung {
				self.hub.broadcast("CALL_UPDATE", &update);
			}
			return;
		}
		drop(calls);
		let message_id = self.post_call_message(channel_id, user_id);
		let live = Live {
			call: MockCall {
				channel_id,
				message_id,
				ringing: Vec::new(),
				members: vec![user_id],
				participants: vec![user_id],
				started: Utc::now(),
			},
			states: BTreeMap::from([(user_id, state)]),
		};
		let create = live.create_event();
		self.calls.calls.lock().insert(channel_id, live);
		self.hub.broadcast("CALL_CREATE", &create);
	}

	fn leave_call(&self, channel_id: u64, user_id: u64) {
		let mut calls = self.calls.calls.lock();
		let Some(live) = calls.get_mut(&channel_id) else {
			return;
		};
		live.call.members.retain(|id| *id != user_id);
		live.states.remove(&user_id);
		if !live.call.members.is_empty() {
			return;
		}
		let Some(live) = calls.remove(&channel_id) else {
			return;
		};
		drop(calls);
		self.hub
			.broadcast("CALL_DELETE", &json!({ "channel_id": channel_id.to_string() }));
		self.end_call_message(&live.call);
	}

	pub fn ring(self: &Arc<Self>, channel_id: u64, recipients: Option<Vec<u64>>) {
		let wanted = recipients.unwrap_or_else(|| self.recipients(channel_id));
		let mut calls = self.calls.calls.lock();
		let Some(live) = calls.get_mut(&channel_id) else {
			return;
		};
		let mut added = Vec::new();
		for user_id in wanted {
			if !live.call.members.contains(&user_id) && !live.call.ringing.contains(&user_id) {
				live.call.ringing.push(user_id);
				added.push(user_id);
			}
		}
		let update = live.update_event();
		drop(calls);
		self.hub.broadcast("CALL_UPDATE", &update);
		if self.config.answer_calls
			&& let Some(user_id) = added.into_iter().find(|id| *id != CURRENT_USER_ID)
		{
			let shared = self.clone();
			tokio::spawn(async move {
				tokio::time::sleep(ANSWER_DELAY).await;
				shared.call_join_as(channel_id, user_id);
			});
		}
	}

	pub fn stop_ringing(&self, channel_id: u64, recipients: Option<Vec<u64>>) {
		let mut calls = self.calls.calls.lock();
		let Some(live) = calls.get_mut(&channel_id) else {
			return;
		};
		let stopped = recipients.unwrap_or_else(|| vec![CURRENT_USER_ID]);
		live.call.ringing.retain(|id| !stopped.contains(id));
		let update = live.update_event();
		drop(calls);
		self.hub.broadcast("CALL_UPDATE", &update);
	}

	pub fn ring_me(&self, channel_id: u64, caller: u64) -> bool {
		if !self.is_private_channel(channel_id) || !self.recipients(channel_id).contains(&caller) {
			return false;
		}
		let state = call_voice_state(Some(channel_id), caller, &scripted_session(caller));
		self.hub.broadcast("VOICE_STATE_UPDATE", &state);
		self.join_call(channel_id, caller, state);
		let others: Vec<u64> = self
			.recipients(channel_id)
			.into_iter()
			.filter(|id| *id != caller)
			.chain([CURRENT_USER_ID])
			.collect();
		let mut calls = self.calls.calls.lock();
		let Some(live) = calls.get_mut(&channel_id) else {
			return false;
		};
		live.call.ringing = others;
		let update = live.update_event();
		drop(calls);
		self.hub.broadcast("CALL_UPDATE", &update);
		true
	}

	fn first_dm_caller(&self) -> Option<(u64, u64)> {
		let channel_id = self.store.lock().private_channel_ids.first().copied()?;
		let caller = self.recipients(channel_id).first().copied()?;
		Some((channel_id, caller))
	}

	pub fn ring_once_after_ready(self: &Arc<Self>) {
		if self.calls.rang.swap(true, Ordering::AcqRel) {
			return;
		}
		let shared = self.clone();
		tokio::spawn(async move {
			tokio::time::sleep(RING_AFTER_READY).await;
			if let Some((channel_id, caller)) = shared.first_dm_caller() {
				shared.ring_me(channel_id, caller);
			}
		});
	}

	pub fn seed_first_ring(&self) {
		if let Some((channel_id, caller)) = self.first_dm_caller() {
			self.seed_ring(channel_id, caller);
		}
	}

	fn seed_ring(&self, channel_id: u64, caller: u64) {
		let message_id = self.post_call_message(channel_id, caller);
		let state = call_voice_state(Some(channel_id), caller, &scripted_session(caller));
		let live = Live {
			call: MockCall {
				channel_id,
				message_id,
				ringing: vec![CURRENT_USER_ID],
				members: vec![caller],
				participants: vec![caller],
				started: Utc::now(),
			},
			states: BTreeMap::from([(caller, state)]),
		};
		self.calls.calls.lock().insert(channel_id, live);
	}

	pub fn call_join_as(&self, channel_id: u64, user_id: u64) {
		let state = call_voice_state(Some(channel_id), user_id, &scripted_session(user_id));
		self.hub.broadcast("VOICE_STATE_UPDATE", &state);
		self.join_call(channel_id, user_id, state);
		self.voice.connect_as(user_id);
	}

	pub fn call_leave_as(&self, channel_id: u64, user_id: u64) {
		let state = call_voice_state(None, user_id, &scripted_session(user_id));
		self.hub.broadcast("VOICE_STATE_UPDATE", &state);
		self.voice.disconnect_as(user_id);
		self.leave_call(channel_id, user_id);
	}

	pub fn answer_elsewhere(&self, channel_id: u64) {
		let state = call_voice_state(Some(channel_id), CURRENT_USER_ID, ELSEWHERE_SESSION);
		self.hub.broadcast("VOICE_STATE_UPDATE", &state);
		self.join_call(channel_id, CURRENT_USER_ID, state);
	}

	fn post_call_message(&self, channel_id: u64, author_id: u64) -> u64 {
		let author = self
			.fixtures
			.users
			.get(&author_id)
			.cloned()
			.unwrap_or_else(|| self.store.lock().current_user.clone());
		let mut store = self.store.lock();
		let id = store.next_snowflake();
		let message = Message {
			id: Snowflake(id),
			channel_id: Snowflake(channel_id),
			author,
			timestamp: Some(Utc::now()),
			kind: MessageType::Call,
			call: Some(CallInfo {
				participants: LenientVec::from(vec![Snowflake(author_id)]),
				ended_timestamp: None,
			}),
			..Message::default()
		};
		let value = to_json(&message);
		store
			.messages
			.entry(channel_id)
			.or_default()
			.insert(id, value.clone());
		store.refresh_last_message(channel_id);
		let gateway = self.gateway_message(&store, &value);
		drop(store);
		self.hub.broadcast("MESSAGE_CREATE", &gateway);
		id
	}

	fn end_call_message(&self, call: &MockCall) {
		let mut store = self.store.lock();
		let Some(value) = store
			.messages
			.get_mut(&call.channel_id)
			.and_then(|messages| messages.get_mut(&call.message_id))
		else {
			return;
		};
		value["call"] = json!({
			"participants": ids(&call.participants),
			"ended_timestamp": Utc::now().to_rfc3339(),
		});
		let value = value.clone();
		let gateway = self.gateway_message(&store, &value);
		drop(store);
		self.hub.broadcast("MESSAGE_UPDATE", &gateway);
	}
}
