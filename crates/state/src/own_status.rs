use chrono::{DateTime, TimeZone, Utc};
use protocol::{ProtoCustomStatus, ProtoStatusSettings, Snowflake, StatusKind, decode_status_settings};
use serde_json::Value;

use crate::changes::Changes;
use crate::state::State;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusEmoji {
	pub id: Option<Snowflake>,
	pub name: Option<String>,
	pub animated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomStatus {
	pub text: Option<String>,
	pub emoji: Option<StatusEmoji>,
	pub expires_at: Option<DateTime<Utc>>,
}

impl CustomStatus {
	pub fn from_proto(proto: &ProtoCustomStatus) -> Option<Self> {
		let text = Some(proto.text.clone()).filter(|text| !text.is_empty());
		let emoji = (proto.emoji_id.is_some() || proto.emoji_name.is_some()).then(|| StatusEmoji {
			id: proto.emoji_id,
			name: proto.emoji_name.clone(),
			animated: false,
		});
		if text.is_none() && emoji.is_none() {
			return None;
		}
		Some(Self {
			text,
			emoji,
			expires_at: proto.expires_at_ms.and_then(from_millis),
		})
	}

	pub fn to_proto(&self, created_at: DateTime<Utc>) -> ProtoCustomStatus {
		ProtoCustomStatus {
			text: self.text.clone().unwrap_or_default(),
			emoji_id: self.emoji.as_ref().and_then(|emoji| emoji.id),
			emoji_name: self.emoji.as_ref().and_then(|emoji| emoji.name.clone()),
			expires_at_ms: self.expires_at.map(to_millis),
			created_at_ms: Some(to_millis(created_at)),
		}
	}

	pub fn expired(&self, now: DateTime<Utc>) -> bool {
		self.expires_at.is_some_and(|expires| expires <= now)
	}
}

pub fn status_kind(name: Option<&str>) -> StatusKind {
	match name {
		Some("idle") => StatusKind::Idle,
		Some("dnd") => StatusKind::Dnd,
		Some("invisible") => StatusKind::Invisible,
		_ => StatusKind::Online,
	}
}

fn from_millis(millis: u64) -> Option<DateTime<Utc>> {
	Utc.timestamp_millis_opt(i64::try_from(millis).ok()?).single()
}

fn to_millis(time: DateTime<Utc>) -> u64 {
	u64::try_from(time.timestamp_millis()).unwrap_or(0)
}

fn legacy_custom(value: &Value) -> Option<ProtoCustomStatus> {
	let object = value.as_object()?;
	let text = object.get("text").and_then(Value::as_str).unwrap_or_default();
	let expires = object
		.get("expires_at")
		.and_then(Value::as_str)
		.and_then(|stamp| DateTime::parse_from_rfc3339(stamp).ok())
		.map(|stamp| to_millis(stamp.with_timezone(&Utc)));
	Some(ProtoCustomStatus {
		text: text.to_owned(),
		emoji_id: object
			.get("emoji_id")
			.and_then(Value::as_str)
			.and_then(|id| id.parse().ok())
			.map(Snowflake),
		emoji_name: object
			.get("emoji_name")
			.and_then(Value::as_str)
			.filter(|name| !name.is_empty())
			.map(str::to_owned),
		expires_at_ms: expires,
		created_at_ms: None,
	})
}

impl State {
	pub fn status_settings(&self) -> &ProtoStatusSettings {
		&self.status_settings
	}

	pub fn own_status(&self) -> StatusKind {
		status_kind(self.status_settings.status.as_deref())
	}

	pub fn own_custom_status(&self, now: DateTime<Utc>) -> Option<CustomStatus> {
		self.status_settings
			.custom_status
			.as_ref()
			.and_then(CustomStatus::from_proto)
			.filter(|custom| !custom.expired(now))
	}

	pub(crate) fn apply_status_proto(&mut self, encoded: &str) -> Changes {
		match decode_status_settings(encoded) {
			Some(settings) => self.set_status_settings(settings),
			None => Changes::default(),
		}
	}

	pub(crate) fn apply_legacy_status(&mut self, settings: &Value) -> Changes {
		let status = settings.get("status").and_then(Value::as_str);
		let custom = settings.get("custom_status");
		if status.is_none() && custom.is_none() {
			return Changes::default();
		}
		let mut next = self.status_settings.clone();
		if let Some(status) = status {
			next.status = Some(status.to_owned());
		}
		if let Some(custom) = custom {
			next.custom_status = legacy_custom(custom);
		}
		self.set_status_settings(next)
	}

	pub(crate) fn set_status_settings(&mut self, settings: ProtoStatusSettings) -> Changes {
		let mut changes = Changes::default();
		if self.status_settings == settings {
			return changes;
		}
		self.status_settings = settings;
		if let Some(me) = self.current_user_id() {
			changes.presence.insert(me);
		}
		changes
	}
}
