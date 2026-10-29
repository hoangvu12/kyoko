use protocol::Opcode;
use serde_json::{Map, Value};

pub const MAX_PAYLOAD_BYTES: usize = 15 * 1024;
const MAX_MEMBER_IDS: usize = 100;
const MAX_MEMBER_LIMIT: u64 = 100;
const MAX_NONCE_BYTES: usize = 32;
const STATUSES: [&str; 5] = ["online", "idle", "dnd", "invisible", "offline"];

pub struct Invalid;

type Checked = Result<(), Invalid>;

fn ensure(condition: bool) -> Checked {
	if condition { Ok(()) } else { Err(Invalid) }
}

pub fn check(opcode: Opcode, bytes: usize, data: &Value, pending: &[u64]) -> Checked {
	ensure(bytes <= MAX_PAYLOAD_BYTES)?;
	match opcode {
		Opcode::Heartbeat => ensure(data.is_null() || data.is_u64()),
		Opcode::PresenceUpdate => presence(data),
		Opcode::VoiceStateUpdate => voice_state(data),
		Opcode::RequestGuildMembers => member_request(data, pending),
		Opcode::GuildSubscriptionsBulk => subscriptions(data, pending),
		_ => Ok(()),
	}
}

pub fn snowflake(value: &Value) -> Option<u64> {
	let text = value.as_str()?;
	if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
		return None;
	}
	let id = text.parse::<u64>().ok()?;
	(id > 0 && i64::try_from(id).is_ok()).then_some(id)
}

fn object(value: &Value) -> Result<&Map<String, Value>, Invalid> {
	value.as_object().ok_or(Invalid)
}

fn optional(fields: &Map<String, Value>, name: &str, valid: impl Fn(&Value) -> bool) -> Checked {
	ensure(fields.get(name).is_none_or(valid))
}

fn nullable_snowflake(value: &Value) -> bool {
	value.is_null() || snowflake(value).is_some()
}

fn presence(data: &Value) -> Checked {
	let fields = object(data)?;
	ensure(
		fields
			.get("status")
			.and_then(Value::as_str)
			.is_some_and(|status| STATUSES.contains(&status)),
	)?;
	optional(fields, "since", |since| since.is_null() || since.is_u64())?;
	optional(fields, "afk", Value::is_boolean)?;
	let activities = fields
		.get("activities")
		.and_then(Value::as_array)
		.ok_or(Invalid)?;
	activities.iter().try_for_each(activity)
}

fn activity(value: &Value) -> Checked {
	let fields = object(value)?;
	ensure(fields.get("name").is_some_and(Value::is_string))?;
	ensure(fields.get("type").is_some_and(Value::is_u64))?;
	optional(fields, "state", Value::is_string)?;
	optional(fields, "emoji", |emoji| {
		emoji.is_null()
			|| emoji.as_object().is_some_and(|emoji| {
				emoji.get("name").is_some_and(Value::is_string)
					&& emoji.get("id").is_none_or(nullable_snowflake)
			})
	})
}

fn voice_state(data: &Value) -> Checked {
	let fields = object(data)?;
	ensure(fields.get("guild_id").is_some_and(nullable_snowflake))?;
	ensure(fields.get("channel_id").is_some_and(nullable_snowflake))?;
	ensure(fields.get("self_mute").is_some_and(Value::is_boolean))?;
	ensure(fields.get("self_deaf").is_some_and(Value::is_boolean))?;
	optional(fields, "self_video", Value::is_boolean)?;
	optional(fields, "preferred_region", |region| {
		region.is_null() || region.is_string()
	})?;
	optional(fields, "preferred_regions", |regions| {
		regions
			.as_array()
			.is_some_and(|regions| regions.iter().all(Value::is_string))
	})?;
	optional(fields, "flags", Value::is_u64)
}

fn guild_ids(value: &Value) -> Result<Vec<u64>, Invalid> {
	match value {
		Value::Array(ids) if !ids.is_empty() => ids.iter().map(|id| snowflake(id).ok_or(Invalid)).collect(),
		single => Ok(vec![snowflake(single).ok_or(Invalid)?]),
	}
}

fn member_request(data: &Value, pending: &[u64]) -> Checked {
	let fields = object(data)?;
	let guilds = guild_ids(fields.get("guild_id").ok_or(Invalid)?)?;
	ensure(guilds.iter().all(|guild_id| !pending.contains(guild_id)))?;
	optional(fields, "presences", Value::is_boolean)?;
	optional(fields, "nonce", |nonce| {
		nonce.as_str().is_some_and(|nonce| nonce.len() <= MAX_NONCE_BYTES)
	})?;
	match (fields.get("user_ids"), fields.get("query")) {
		(Some(ids), None) => member_ids(ids),
		(None, Some(query)) => {
			let query = query.as_str().ok_or(Invalid)?;
			let limit = fields.get("limit").and_then(Value::as_u64).ok_or(Invalid)?;
			ensure(limit <= MAX_MEMBER_LIMIT && (!query.is_empty() || limit == 0))
		}
		_ => Err(Invalid),
	}
}

fn member_ids(value: &Value) -> Checked {
	let ids = match value {
		Value::Array(ids) => ids
			.iter()
			.map(|id| snowflake(id).ok_or(Invalid))
			.collect::<Result<Vec<_>, _>>()?,
		single => vec![snowflake(single).ok_or(Invalid)?],
	};
	let mut unique = ids.clone();
	unique.sort_unstable();
	unique.dedup();
	ensure(!ids.is_empty() && ids.len() <= MAX_MEMBER_IDS && unique.len() == ids.len())
}

fn subscriptions(data: &Value, pending: &[u64]) -> Checked {
	let guilds = object(object(data)?.get("subscriptions").ok_or(Invalid)?)?;
	guilds.iter().try_for_each(|(guild_id, subscription)| {
		let guild_id = snowflake(&Value::String(guild_id.clone())).ok_or(Invalid)?;
		ensure(!pending.contains(&guild_id))?;
		guild_subscription(subscription)
	})
}

fn guild_subscription(value: &Value) -> Checked {
	let fields = object(value)?;
	for flag in ["typing", "threads", "activities", "member_updates"] {
		optional(fields, flag, Value::is_boolean)?;
	}
	for list in ["members", "thread_member_lists"] {
		optional(fields, list, |ids| {
			ids.as_array()
				.is_some_and(|ids| ids.iter().all(|id| snowflake(id).is_some()))
		})?;
	}
	let Some(channels) = fields.get("channels") else {
		return Ok(());
	};
	object(channels)?.iter().try_for_each(|(channel_id, ranges)| {
		snowflake(&Value::String(channel_id.clone())).ok_or(Invalid)?;
		ranges.as_array().ok_or(Invalid)?.iter().try_for_each(range)
	})
}

fn range(value: &Value) -> Checked {
	let bounds = value.as_array().ok_or(Invalid)?;
	let [start, end] = bounds.as_slice() else {
		return Err(Invalid);
	};
	let (start, end) = (start.as_u64().ok_or(Invalid)?, end.as_u64().ok_or(Invalid)?);
	ensure(start <= end && i32::try_from(end).is_ok())
}
