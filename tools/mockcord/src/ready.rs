use std::collections::BTreeMap;

use protocol::capability::{
	DEDUPE_USER_OBJECTS, LAZY_USER_NOTES, NON_CHANNEL_READ_STATES, PRIORITIZED_READY_PAYLOAD,
	VERSIONED_READ_STATES, VERSIONED_USER_GUILD_SETTINGS, has,
};
use protocol::{Channel, ChannelType, Presence, RelationshipKind, Snowflake, User};
use serde::Serialize;
use serde_json::{Value, json};

use crate::fixtures::{Fixtures, GuildFixture};
use crate::json::to_json;
use crate::state::{Shared, Store, Versions};
use crate::web::installation_id;

pub const DEFAULT_API_VERSION: i64 = 9;
const MOCK_FOLDER_ID: i64 = 1_001;
const MOCK_FOLDER_COLOR: i64 = 0x6666ff;
const GUILD_EVENT_READ_STATE: i64 = 1;

pub fn is_thread(kind: ChannelType) -> bool {
	matches!(
		kind,
		ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::AnnouncementThread
	)
}

pub fn guild_threads(store: &Store, guild_id: u64) -> Vec<&Channel> {
	let mut threads: Vec<&Channel> = store
		.channels
		.values()
		.filter(|channel| channel.guild_id == Some(Snowflake(guild_id)) && is_thread(channel.kind))
		.collect();
	threads.sort_by_key(|channel| channel.id);
	threads
}

fn ready_guild(store: &Store, guild: &GuildFixture) -> Value {
	let threads: Vec<Value> = guild_threads(store, guild.id)
		.into_iter()
		.map(|thread| store.thread_json(thread))
		.collect();
	json!({
		"id": guild.id.to_string(),
		"properties": store.properties[&guild.id],
		"channels": store.guild_channels(guild),
		"roles": store.roles[&guild.id],
		"emojis": store.emojis[&guild.id],
		"stickers": [],
		"threads": threads,
		"members": [],
		"voice_states": guild.voice_states,
		"presences": [],
		"member_count": store.live_member_count(guild),
		"joined_at": guild.joined_at,
		"large": false,
		"premium_subscription_count": guild.properties.premium_subscription_count,
		"stage_instances": [],
		"guild_scheduled_events": [],
	})
}

fn live_guilds<'a>(fixtures: &'a Fixtures, store: &Store) -> impl Iterator<Item = &'a GuildFixture> {
	fixtures
		.guilds
		.iter()
		.filter(|guild| !store.left_guilds.contains(&guild.id))
}

fn merged_members(fixtures: &Fixtures, store: &Store) -> Value {
	let per_guild: Vec<Vec<Value>> = live_guilds(fixtures, store)
		.map(|guild| vec![to_json(guild.current_member())])
		.collect();
	json!(per_guild)
}

fn referenced_users<'a>(fixtures: &'a Fixtures, store: &Store) -> Vec<&'a User> {
	let mut users: BTreeMap<u64, &User> = BTreeMap::new();
	for relationship in store.relationships.values() {
		if let Some(user) = fixtures.users.get(&relationship.id.0) {
			users.insert(relationship.id.0, user);
		}
	}
	for channel_id in &store.private_channel_ids {
		let recipient_ids = store
			.channels
			.get(channel_id)
			.map(|channel| channel.recipient_ids.as_slice());
		for id in recipient_ids.unwrap_or_default() {
			if let Some(user) = fixtures.users.get(&id.0) {
				users.insert(id.0, user);
			}
		}
	}
	for guild in live_guilds(fixtures, store) {
		for state in &guild.voice_states {
			if let Some(user) = fixtures.users.get(&state.user_id.0) {
				users.insert(state.user_id.0, user);
			}
		}
	}
	users.into_values().collect()
}

fn guild_presences(store: &Store, guild_id: u64) -> Vec<&Presence> {
	let mut presences: Vec<&Presence> = store
		.presences
		.get(&guild_id)
		.map(|by_user| by_user.values().collect())
		.unwrap_or_default();
	presences.sort_by_key(|presence| presence.user_id);
	presences
}

fn friend_presences<'a>(fixtures: &Fixtures, store: &'a Store) -> Vec<&'a Presence> {
	let mut seen: BTreeMap<u64, &Presence> = BTreeMap::new();
	for guild in live_guilds(fixtures, store) {
		for presence in guild_presences(store, guild.id) {
			if let Some(user_id) = presence.user_id {
				seen.entry(user_id.0).or_insert(presence);
			}
		}
	}
	store
		.relationships
		.values()
		.filter(|relationship| relationship.kind == RelationshipKind::Friend)
		.filter_map(|relationship| seen.get(&relationship.id.0).copied())
		.collect()
}

fn known_version(client_state: &Value, field: &str, versions: &Versions) -> Option<u64> {
	client_state[field]
		.as_u64()
		.filter(|known| *known > 0 && *known <= versions.current)
}

fn container<T: Serialize>(
	entries: impl Iterator<Item = (u64, T)>,
	versions: Option<(&Versions, Option<u64>)>,
) -> Value {
	let Some((versions, known)) = versions else {
		return json!(entries.map(|(_, entry)| entry).collect::<Vec<_>>());
	};
	let entries: Vec<T> = entries
		.filter(|(id, _)| known.is_none_or(|known| versions.changed_since(*id, known)))
		.map(|(_, entry)| entry)
		.collect();
	json!({ "entries": entries, "version": versions.current, "partial": known.is_some() })
}

fn read_states(fixtures: &Fixtures, store: &Store, capabilities: u64, client_state: &Value) -> Value {
	let versions = &store.read_state_version;
	let known = known_version(client_state, "read_state_version", versions);
	let mut entries: Vec<(u64, Value)> = store
		.read_states
		.iter()
		.map(|(id, state)| (*id, to_json(state)))
		.collect();
	if known.is_none() && has(capabilities, NON_CHANNEL_READ_STATES) {
		entries.extend(live_guilds(fixtures, store).map(|guild| {
			(
				guild.id,
				json!({
					"id": guild.id.to_string(),
					"read_state_type": GUILD_EVENT_READ_STATE,
					"last_acked_id": "0",
					"badge_count": 0,
				}),
			)
		}));
	}
	let versioned = has(capabilities, VERSIONED_READ_STATES).then_some((versions, known));
	container(entries.into_iter(), versioned)
}

fn guild_settings(store: &Store, capabilities: u64, client_state: &Value) -> Value {
	let versions = &store.settings_version;
	let known = known_version(client_state, "user_guild_settings_version", versions);
	let entries = store
		.settings
		.iter()
		.map(|(scope, settings)| (*scope, to_json(settings)));
	let versioned = has(capabilities, VERSIONED_USER_GUILD_SETTINGS).then_some((versions, known));
	container(entries, versioned)
}

fn private_channels(fixtures: &Fixtures, store: &Store, dedupe: bool) -> Vec<Value> {
	store
		.private_channel_ids
		.iter()
		.filter_map(|id| store.channels.get(id))
		.map(|channel| {
			let mut value = to_json(channel);
			if !dedupe {
				let recipients: Vec<&User> = channel
					.recipient_ids
					.iter()
					.filter_map(|id| fixtures.users.get(&id.0))
					.collect();
				value["recipients"] = json!(recipients);
				if let Some(fields) = value.as_object_mut() {
					fields.remove("recipient_ids");
				}
			}
			value
		})
		.collect()
}

fn relationships(fixtures: &Fixtures, store: &Store, dedupe: bool) -> Vec<Value> {
	store
		.relationships
		.values()
		.map(|relationship| {
			let mut value = to_json(relationship);
			if !dedupe && let Some(user) = fixtures.users.get(&relationship.id.0) {
				value["user"] = to_json(user);
			}
			value
		})
		.collect()
}

pub fn build(
	shared: &Shared,
	session_id: &str,
	identify: &Value,
	capabilities: u64,
	version: i64,
) -> (Value, Value) {
	let client_state = &identify["client_state"];
	let dedupe = has(capabilities, DEDUPE_USER_OBJECTS);
	let prioritized = has(capabilities, PRIORITIZED_READY_PAYLOAD);

	let fixtures = &shared.fixtures;
	let store = shared.store.lock();
	let guilds: Vec<Value> = live_guilds(fixtures, &store)
		.map(|guild| {
			let mut value = ready_guild(&store, guild);
			if !prioritized {
				value["presences"] = json!(guild_presences(&store, guild.id));
			}
			value
		})
		.collect();
	let folder_guild_ids: Vec<String> = live_guilds(fixtures, &store)
		.take(2)
		.map(|guild| guild.id.to_string())
		.collect();
	let mut ready = json!({
		"v": version,
		"user": store.current_user,
		"guilds": guilds,
		"private_channels": private_channels(fixtures, &store, dedupe),
		"relationships": relationships(fixtures, &store, dedupe),
		"read_state": read_states(fixtures, &store, capabilities, client_state),
		"user_guild_settings": guild_settings(&store, capabilities, client_state),
		"session_id": session_id,
		"resume_gateway_url": shared.urls.gateway,
		"sessions": [{
			"session_id": session_id,
			"status": "online",
			"activities": [],
			"client_info": { "client": "desktop", "os": "windows", "version": 0 },
			"active": true,
		}],
		"merged_members": merged_members(fixtures, &store),
		"analytics_token": "x",
		"apex_experiments": { "installation": installation_id(shared.config.seed) },
		"user_settings_proto": null,
		"user_settings": {
			"guild_folders": [{
				"id": MOCK_FOLDER_ID,
				"name": "Mock folder",
				"color": MOCK_FOLDER_COLOR,
				"guild_ids": folder_guild_ids,
			}],
		},
	});
	if dedupe {
		ready["users"] = json!(referenced_users(fixtures, &store));
	}
	if !has(capabilities, LAZY_USER_NOTES) {
		ready["notes"] = json!({});
	}
	let merged_presences = if prioritized {
		json!({
			"guilds": live_guilds(fixtures, &store)
				.map(|guild| guild_presences(&store, guild.id))
				.collect::<Vec<_>>(),
			"friends": friend_presences(fixtures, &store),
		})
	} else {
		json!({ "guilds": [], "friends": [] })
	};
	let supplemental = json!({
		"merged_presences": merged_presences,
		"merged_members": merged_members(fixtures, &store),
		"guilds": live_guilds(fixtures, &store)
			.map(|guild| json!({ "id": guild.id.to_string(), "voice_states": guild.voice_states }))
			.collect::<Vec<_>>(),
	});
	(ready, supplemental)
}
