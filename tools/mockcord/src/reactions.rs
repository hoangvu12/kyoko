use protocol::capability::DEBOUNCE_MESSAGE_REACTIONS;
use serde_json::{Value, json};

use crate::error::ApiError;
use crate::fixtures::CURRENT_USER_ID;
use crate::json::to_json;
use crate::state::{Shared, Store};

pub const BURST_COLORS: [&str; 2] = ["#b47cf2", "#7aa2f7"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct React<'a> {
	pub channel_id: u64,
	pub message_id: u64,
	pub emoji: &'a str,
	pub user_id: u64,
	pub burst: bool,
}

fn split(emoji: &str) -> (&str, Option<&str>) {
	match emoji.rsplit_once(':') {
		Some((name, id)) if id.parse::<u64>().is_ok() => (name, Some(id)),
		_ => (emoji, None),
	}
}

fn emoji_key(emoji: &str) -> String {
	let (name, id) = split(emoji);
	id.unwrap_or(name).to_owned()
}

fn reaction_matches(reaction: &Value, name: &str, emoji_id: Option<&str>) -> bool {
	let emoji = &reaction["emoji"];
	match emoji_id {
		Some(emoji_id) => emoji["id"].as_str() == Some(emoji_id),
		None => emoji["id"].is_null() && emoji["name"].as_str() == Some(name),
	}
}

fn counts(reaction: &Value) -> (i64, i64) {
	let count = reaction["count"].as_i64().unwrap_or(0);
	let details = &reaction["count_details"];
	match (details["normal"].as_i64(), details["burst"].as_i64()) {
		(Some(normal), Some(burst)) if normal + burst > 0 => (normal, burst),
		_ => (count, 0),
	}
}

fn mine(reaction: &Value, burst: bool) -> bool {
	let field = if burst { "me_burst" } else { "me" };
	reaction[field].as_bool() == Some(true)
}

fn seeded(shared: &Shared, reaction: &Value, burst: bool) -> Vec<u64> {
	let (normal, bursts) = counts(reaction);
	let wanted = if burst { bursts } else { normal }.max(0) as usize;
	let mut ids = Vec::with_capacity(wanted);
	if mine(reaction, burst) {
		ids.push(CURRENT_USER_ID);
	}
	ids.extend(
		shared
			.fixtures
			.users
			.keys()
			.copied()
			.filter(|id| *id != CURRENT_USER_ID)
			.take(wanted.saturating_sub(ids.len())),
	);
	ids
}

impl Shared {
	fn reactors_of(&self, store: &Store, message: &Value, emoji: &str, burst: bool) -> Option<Vec<u64>> {
		let (name, emoji_id) = split(emoji);
		let key = (parse(&message["id"])?, emoji_key(emoji), burst);
		if let Some(known) = store.reactors.get(&key) {
			return Some(known.clone());
		}
		let reaction = message["reactions"]
			.as_array()?
			.iter()
			.find(|reaction| reaction_matches(reaction, name, emoji_id))?;
		Some(seeded(self, reaction, burst))
	}

	pub fn react(&self, react: React, add: bool) -> Result<(), ApiError> {
		let (name, emoji_id) = split(react.emoji);
		let mut store = self.store.lock();
		let guild_id = Self::guild_of(&store, react.channel_id)?;
		let message = store
			.message(react.channel_id, react.message_id)
			.ok_or_else(ApiError::unknown_message)?
			.clone();
		let mut reactors = self
			.reactors_of(&store, &message, react.emoji, react.burst)
			.unwrap_or_default();
		let present = reactors.contains(&react.user_id);
		if present == add {
			return Ok(());
		}
		if add {
			reactors.push(react.user_id);
		} else {
			reactors.retain(|id| *id != react.user_id);
		}
		let total = reactors.len() as i64;
		store
			.reactors
			.insert((react.message_id, emoji_key(react.emoji), react.burst), reactors);
		let message = store
			.message_mut(react.channel_id, react.message_id)
			.ok_or_else(ApiError::unknown_message)?;
		let reactions = message
			.as_object_mut()
			.map(|fields| fields.entry("reactions").or_insert_with(|| json!([])))
			.and_then(Value::as_array_mut)
			.ok_or_else(ApiError::unknown_message)?;
		let index = match reactions
			.iter()
			.position(|reaction| reaction_matches(reaction, name, emoji_id))
		{
			Some(index) => index,
			None => {
				reactions.push(json!({
					"count": 0,
					"count_details": { "burst": 0, "normal": 0 },
					"me": false,
					"me_burst": false,
					"emoji": { "id": emoji_id, "name": name, "animated": false },
					"burst_colors": [],
				}));
				reactions.len() - 1
			}
		};
		let reaction = &mut reactions[index];
		let (normal, bursts) = counts(reaction);
		let (normal, bursts) = if react.burst {
			(normal, total)
		} else {
			(total, bursts)
		};
		reaction["count"] = json!(normal + bursts);
		reaction["count_details"] = json!({ "burst": bursts, "normal": normal });
		if react.user_id == CURRENT_USER_ID {
			reaction[if react.burst { "me_burst" } else { "me" }] = json!(add);
		}
		if react.burst && add {
			reaction["burst_colors"] = json!(BURST_COLORS);
		}
		if normal + bursts <= 0 {
			reactions.remove(index);
		}
		let mut data = json!({
			"user_id": react.user_id.to_string(),
			"channel_id": react.channel_id.to_string(),
			"message_id": react.message_id.to_string(),
			"emoji": { "id": emoji_id, "name": name, "animated": false },
			"burst": react.burst,
			"type": u8::from(react.burst),
		});
		if react.burst && add {
			data["burst_colors"] = json!(BURST_COLORS);
		}
		if let Some(guild_id) = guild_id {
			data["guild_id"] = json!(guild_id.to_string());
			if add {
				data["member"] = self.partial_member(guild_id.0, react.user_id);
			}
		}
		drop(store);
		match (add, react.burst) {
			(true, false) => self.hub.broadcast_by_capability(
				DEBOUNCE_MESSAGE_REACTIONS,
				("MESSAGE_REACTION_ADD_MANY", &batched_reaction(&data)),
				("MESSAGE_REACTION_ADD", &data),
			),
			(true, true) => self.hub.broadcast("MESSAGE_REACTION_ADD", &data),
			(false, _) => self.hub.broadcast("MESSAGE_REACTION_REMOVE", &data),
		}
		Ok(())
	}

	pub fn reaction_users(
		&self,
		channel_id: u64,
		message_id: u64,
		emoji: &str,
		page: ReactionPage,
	) -> Result<Value, ApiError> {
		let store = self.store.lock();
		let message = store
			.message(channel_id, message_id)
			.ok_or_else(ApiError::unknown_message)?;
		let mut ids = self
			.reactors_of(&store, message, emoji, page.burst)
			.ok_or_else(ApiError::unknown_message)?;
		ids.sort_unstable();
		let users: Vec<Value> = ids
			.into_iter()
			.filter(|id| page.after.is_none_or(|after| *id > after))
			.take(page.limit)
			.filter_map(|id| {
				if id == CURRENT_USER_ID {
					Some(to_json(&store.current_user))
				} else {
					self.fixtures.users.get(&id).map(to_json)
				}
			})
			.collect();
		Ok(Value::Array(users))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionPage {
	pub limit: usize,
	pub after: Option<u64>,
	pub burst: bool,
}

fn parse(value: &Value) -> Option<u64> {
	value
		.as_str()
		.and_then(|id| id.parse().ok())
		.or_else(|| value.as_u64())
}

fn batched_reaction(single: &Value) -> Value {
	let mut batched = json!({
		"channel_id": single["channel_id"],
		"message_id": single["message_id"],
		"reactions": [{ "emoji": single["emoji"], "users": [single["user_id"]] }],
	});
	if !single["guild_id"].is_null() {
		batched["guild_id"] = single["guild_id"].clone();
	}
	batched
}
