use serde_json::{Value, json};

use crate::fixtures::CURRENT_USER_ID;
use crate::json::{mentions_user, parse_id};
use crate::state::Shared;

pub const DEFAULT_LIMIT: usize = 25;
const MAX_LIMIT: usize = 100;

pub struct MentionsQuery {
	pub limit: usize,
	pub before: Option<u64>,
	pub guild_id: Option<u64>,
	pub everyone: bool,
	pub roles: bool,
}

impl Shared {
	pub fn recent_mentions(&self, query: &MentionsQuery) -> Vec<Value> {
		let store = self.store.lock();
		let limit = query.limit.clamp(1, MAX_LIMIT);
		let mut found: Vec<(u64, Value)> = Vec::new();
		for (channel_id, messages) in &store.messages {
			let guild_id = store
				.channels
				.get(channel_id)
				.and_then(|channel| channel.guild_id)
				.map(|id| id.0);
			let Some(guild_id) = guild_id else {
				continue;
			};
			if query.guild_id.is_some_and(|wanted| wanted != guild_id) {
				continue;
			}
			let my_roles: Vec<u64> = self.partial_member(guild_id, CURRENT_USER_ID)["roles"]
				.as_array()
				.cloned()
				.unwrap_or_default()
				.iter()
				.filter_map(parse_id)
				.collect();
			for (id, message) in messages {
				if store.dismissed_mentions.contains(id) || query.before.is_some_and(|before| *id >= before) {
					continue;
				}
				if parse_id(&message["author"]["id"]) == Some(CURRENT_USER_ID) {
					continue;
				}
				let everyone = query.everyone && message["mention_everyone"].as_bool() == Some(true);
				let role = query.roles
					&& message["mention_roles"].as_array().is_some_and(|roles| {
						roles
							.iter()
							.filter_map(parse_id)
							.any(|role| my_roles.contains(&role))
					});
				if mentions_user(message, CURRENT_USER_ID) || everyone || role {
					found.push((*id, self.gateway_message(&store, message)));
				}
			}
		}
		found.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
		found
			.into_iter()
			.take(limit)
			.map(|(_, message)| message)
			.collect()
	}

	pub fn dismiss_mention(&self, message_id: u64) {
		self.store.lock().dismissed_mentions.insert(message_id);
		self.hub.broadcast(
			"RECENT_MENTION_DELETE",
			&json!({ "message_id": message_id.to_string() }),
		);
	}
}
