use chrono::DateTime;
use mockcord::{Fixtures, MockConfig};
use serde_json::{Value, json};

const ANCHOR_MS: i64 = 1_767_225_600_000;

fn generated(seed: u64) -> Value {
	let config = MockConfig {
		seed,
		..MockConfig::default()
	};
	let anchor = DateTime::from_timestamp_millis(ANCHOR_MS).expect("valid anchor");
	let fixtures = Fixtures::generate(&config, anchor);
	json!({
		"current_user": fixtures.current_user,
		"users": fixtures.users,
		"channels": fixtures.channels,
		"private_channel_ids": fixtures.private_channel_ids,
		"relationships": fixtures.relationships,
		"read_states": fixtures.read_states,
		"user_guild_settings": fixtures.user_guild_settings,
		"messages": fixtures.messages,
		"guilds": fixtures.guilds.iter().map(|guild| json!({
			"id": guild.id,
			"properties": guild.properties,
			"roles": guild.roles,
			"channel_ids": guild.channel_ids,
			"emojis": guild.emojis,
			"members": guild.members,
			"presences": guild.presences,
			"voice_states": guild.voice_states,
			"joined_at": guild.joined_at,
		})).collect::<Vec<_>>(),
	})
}

#[test]
fn the_same_seed_generates_the_same_fixtures() {
	assert_eq!(generated(7), generated(7));
	assert_ne!(generated(7), generated(8));
}
