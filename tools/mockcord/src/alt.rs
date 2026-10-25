use serde_json::{Value, json};

use crate::ready::API_CODE_VERSION;
use crate::state::Shared;
use crate::web::installation_id;

pub const ALT_USER_ID: u64 = 9_100_000_000_000_000_101;
pub const ALT_USERNAME: &str = "mockalt";

pub fn user() -> Value {
	json!({
		"id": ALT_USER_ID.to_string(),
		"username": ALT_USERNAME,
		"global_name": "Mock Alt",
		"discriminator": "0",
		"avatar": null,
		"verified": true,
		"mfa_enabled": false,
		"phone": null,
		"flags": 0,
		"premium_type": 0,
	})
}

pub fn ready(shared: &Shared, session_id: &str, version: i64) -> (Value, Value) {
	let empty = json!({ "entries": [], "version": 1, "partial": false });
	let ready = json!({
		"v": version,
		"user": user(),
		"guilds": [],
		"private_channels": [],
		"relationships": [],
		"users": [],
		"notes": {},
		"read_state": empty,
		"user_guild_settings": empty,
		"session_id": session_id,
		"resume_gateway_url": shared.urls.gateway,
		"sessions": [{
			"session_id": session_id,
			"status": "online",
			"activities": [],
			"client_info": { "client": "desktop", "os": "windows", "version": 0 },
			"active": true,
		}],
		"merged_members": [],
		"analytics_token": "x",
		"api_code_version": API_CODE_VERSION,
		"apex_experiments": { "installation": installation_id(shared.config.seed) },
		"user_settings": { "guild_folders": [] },
	});
	let supplemental = json!({
		"merged_presences": { "guilds": [], "friends": [] },
		"merged_members": [],
		"guilds": [],
	});
	(ready, supplemental)
}
