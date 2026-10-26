use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::lenient::timestamp;
use crate::permissions::Permissions;
use crate::snowflake::Snowflake;
use crate::user::{AvatarDecorationData, User};

pub const MEMBER_COMPLETED_ONBOARDING: u64 = 1 << 1;
pub const MEMBER_BYPASSES_VERIFICATION: u64 = 1 << 2;
pub const MEMBER_STARTED_ONBOARDING: u64 = 1 << 3;
pub const MEMBER_QUARANTINED_USERNAME: u64 = 1 << 7;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Member {
	pub user: Option<User>,
	pub nick: Option<String>,
	pub avatar: Option<String>,
	pub banner: Option<String>,
	pub roles: Vec<Snowflake>,
	#[serde(deserialize_with = "timestamp")]
	pub joined_at: Option<DateTime<Utc>>,
	#[serde(deserialize_with = "timestamp")]
	pub premium_since: Option<DateTime<Utc>>,
	pub deaf: bool,
	pub mute: bool,
	pub flags: Option<u64>,
	pub pending: Option<bool>,
	pub permissions: Option<Permissions>,
	#[serde(deserialize_with = "timestamp")]
	pub communication_disabled_until: Option<DateTime<Utc>>,
	pub avatar_decoration_data: Option<AvatarDecorationData>,
	pub guild_id: Option<Snowflake>,
}
