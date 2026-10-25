use chrono::Utc;
use serde_json::{Value, json};

use crate::error::ApiError;
use crate::fixtures::CURRENT_USER_ID;
use crate::json::to_json;
use crate::state::{Shared, Store};

pub const COMMUNITY_INVITE: &str = "mockcommunity";
pub const COMMUNITY_NAME: &str = "Onboarding Commons";
const COMMUNITY_FEATURES: [&str; 4] = [
	"COMMUNITY",
	"GUILD_ONBOARDING",
	"MEMBER_VERIFICATION_GATE_ENABLED",
	"PREVIEW_ENABLED",
];
const STARTED_ONBOARDING: u64 = 1 << 3;
const COMPLETED_ONBOARDING: u64 = 1 << 1;
const VERIFICATION_VERSION: &str = "2026-09-01T12:00:00.000000+00:00";
const RULES: [&str; 3] = [
	"Be kind to each other.",
	"No spam or self-promotion.",
	"Keep conversations in the right channels.",
];

#[derive(Debug, Clone)]
pub struct CommunityGuild {
	pub guild_id: u64,
	pub channel_id: u64,
	pub flags: u64,
	pub pending: bool,
	pub joined_at: String,
	pub responses: Option<Value>,
	pub agreement: Option<Value>,
}

fn ids(store: &mut Store) -> (u64, u64) {
	if let Some(joined) = &store.community {
		return (joined.guild_id, joined.channel_id);
	}
	let guild_id = 1_390_000_000_000_000_001;
	(guild_id, guild_id + 1)
}

fn option(id: u64, title: &str, description: &str, emoji: &str) -> Value {
	json!({
		"id": id.to_string(),
		"title": title,
		"description": description,
		"emoji": { "id": null, "name": emoji, "animated": false },
		"channel_ids": [],
		"role_ids": [],
	})
}

impl Shared {
	fn community_invite(&self, store: &mut Store) -> Value {
		let (guild_id, channel_id) = ids(store);
		json!({
			"type": 0,
			"code": COMMUNITY_INVITE,
			"expires_at": null,
			"guild_id": guild_id.to_string(),
			"guild": {
				"id": guild_id.to_string(),
				"name": COMMUNITY_NAME,
				"icon": null,
				"splash": null,
				"banner": null,
				"description": "A server that asks new members a few questions.",
				"features": COMMUNITY_FEATURES,
				"verification_level": 1,
				"vanity_url_code": null,
				"nsfw_level": 0,
				"premium_subscription_count": 0,
			},
			"channel": { "id": channel_id.to_string(), "type": 0, "name": "welcome" },
			"inviter": to_json(&store.current_user),
			"approximate_member_count": 128,
			"approximate_presence_count": 42,
		})
	}

	pub fn community_invite_json(&self, code: &str) -> Option<Value> {
		(code == COMMUNITY_INVITE).then(|| self.community_invite(&mut self.store.lock()))
	}

	fn member(&self, store: &Store, joined: &CommunityGuild) -> Value {
		json!({
			"guild_id": joined.guild_id.to_string(),
			"user": to_json(&store.current_user),
			"roles": [],
			"joined_at": joined.joined_at,
			"flags": joined.flags,
			"pending": joined.pending,
			"deaf": false,
			"mute": false,
		})
	}

	pub fn accept_community_invite(&self) -> Value {
		let mut store = self.store.lock();
		let mut invite = self.community_invite(&mut store);
		let (guild_id, channel_id) = ids(&mut store);
		if store.community.is_some() {
			invite["new_member"] = json!(false);
			return invite;
		}
		let joined = CommunityGuild {
			guild_id,
			channel_id,
			flags: STARTED_ONBOARDING,
			pending: true,
			joined_at: Utc::now().to_rfc3339(),
			responses: None,
			agreement: None,
		};
		let member = self.member(&store, &joined);
		let guild = json!({
			"id": guild_id.to_string(),
			"name": COMMUNITY_NAME,
			"icon": null,
			"owner_id": "1390000000000000999",
			"member_count": 128,
			"joined_at": joined.joined_at,
			"roles": [{ "id": guild_id.to_string(), "name": "@everyone", "permissions": "2248473465835073", "position": 0, "color": 0 }],
			"emojis": [],
			"stickers": [],
			"channels": [{
				"id": channel_id.to_string(),
				"type": 0,
				"guild_id": guild_id.to_string(),
				"name": "welcome",
				"position": 0,
				"last_message_id": (channel_id + 100).to_string(),
			}],
			"threads": [],
			"members": [member],
			"voice_states": [],
			"presences": [],
			"features": COMMUNITY_FEATURES,
			"premium_tier": 0,
			"verification_level": 1,
			"system_channel_id": channel_id.to_string(),
		});
		store.community = Some(joined);
		drop(store);
		self.hub.broadcast("GUILD_CREATE", &guild);
		invite["new_member"] = json!(true);
		invite["show_verification_form"] = json!(true);
		invite
	}

	fn joined_community(&self, guild_id: u64) -> Result<(), ApiError> {
		let store = self.store.lock();
		match &store.community {
			Some(joined) if joined.guild_id == guild_id => Ok(()),
			_ => Err(ApiError::unknown_guild()),
		}
	}

	pub fn community_onboarding(&self, guild_id: u64) -> Result<Value, ApiError> {
		self.joined_community(guild_id)?;
		let first = guild_id + 10;
		Ok(json!({
			"guild_id": guild_id.to_string(),
			"enabled": true,
			"mode": 0,
			"below_requirements": false,
			"default_channel_ids": [(guild_id + 1).to_string()],
			"responses": [],
			"onboarding_prompts_seen": {},
			"onboarding_responses_seen": {},
			"prompts": [
				{
					"id": first.to_string(),
					"type": 0,
					"title": "What brings you here?",
					"single_select": false,
					"required": true,
					"in_onboarding": true,
					"options": [
						option(first + 1, "Rust", "Talk about the language", "🦀"),
						option(first + 2, "Games", "Find people to play with", "🎮"),
						option(first + 3, "Music", "Share what you listen to", "🎵"),
					],
				},
				{
					"id": (first + 4).to_string(),
					"type": 1,
					"title": "How long have you been coding?",
					"single_select": true,
					"required": false,
					"in_onboarding": true,
					"options": [
						option(first + 5, "Just starting", "", "🌱"),
						option(first + 6, "A few years", "", "🌿"),
						option(first + 7, "Forever", "", "🌳"),
					],
				},
				{
					"id": (first + 8).to_string(),
					"type": 0,
					"title": "Only in customization",
					"single_select": false,
					"required": false,
					"in_onboarding": false,
					"options": [option(first + 9, "Hidden", "", "🙈")],
				},
			],
		}))
	}

	fn member_update(&self, update: impl FnOnce(&mut CommunityGuild)) -> Result<(), ApiError> {
		let mut store = self.store.lock();
		let mut joined = store.community.clone().ok_or_else(ApiError::unknown_guild)?;
		update(&mut joined);
		let member = self.member(&store, &joined);
		store.community = Some(joined);
		drop(store);
		self.hub.broadcast("GUILD_MEMBER_UPDATE", &member);
		Ok(())
	}

	pub fn community_onboarding_responses(&self, guild_id: u64, body: &Value) -> Result<Value, ApiError> {
		self.joined_community(guild_id)?;
		let body = body.clone();
		self.member_update(|joined| {
			joined.flags |= COMPLETED_ONBOARDING;
			joined.responses = Some(body.clone());
		})?;
		Ok(json!({
			"guild_id": guild_id.to_string(),
			"user_id": CURRENT_USER_ID.to_string(),
			"onboarding_responses": body["onboarding_responses"],
			"onboarding_prompts_seen": body["onboarding_prompts_seen"],
			"onboarding_responses_seen": body["onboarding_responses_seen"],
		}))
	}

	pub fn community_verification(&self, guild_id: u64) -> Result<Value, ApiError> {
		self.joined_community(guild_id)?;
		Ok(json!({
			"version": VERIFICATION_VERSION,
			"description": "Please read our rules before you start chatting.",
			"form_fields": [{
				"field_type": "TERMS",
				"label": "Read and agree to the server rules",
				"values": RULES,
				"required": true,
				"description": null,
				"automations": null,
			}],
		}))
	}

	pub fn community_join_request(&self, guild_id: u64, body: &Value) -> Result<Value, ApiError> {
		self.joined_community(guild_id)?;
		let agreed = body["form_fields"].as_array().is_some_and(|fields| {
			!fields.is_empty() && fields.iter().all(|field| field["response"] == json!(true))
		});
		if !agreed || body["version"] != json!(VERIFICATION_VERSION) {
			return Err(ApiError::invalid_body(
				"form_fields must agree to the current rules",
			));
		}
		let body = body.clone();
		self.member_update(|joined| {
			joined.pending = false;
			joined.agreement = Some(body.clone());
		})?;
		Ok(json!({
			"guild_id": guild_id.to_string(),
			"user_id": CURRENT_USER_ID.to_string(),
			"application_status": "APPROVED",
		}))
	}

	pub fn leave_community(&self, guild_id: u64) -> bool {
		let mut store = self.store.lock();
		if store
			.community
			.as_ref()
			.is_none_or(|joined| joined.guild_id != guild_id)
		{
			return false;
		}
		store.community = None;
		drop(store);
		self.hub
			.broadcast("GUILD_DELETE", &json!({ "id": guild_id.to_string() }));
		true
	}

	pub fn community_state(&self) -> Option<CommunityGuild> {
		self.store.lock().community.clone()
	}
}
