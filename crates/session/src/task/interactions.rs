use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use protocol::{ApplicationCommandIndex, ApplicationCommandType, InteractionOption, Snowflake};
use rest::api::CommandScope;
use serde_json::{Value, json};
use state::LocalEvent;

use super::{Core, Internal, actions};

const INDEX_RETRY: Duration = Duration::from_secs(60);
const KEPT_INTERACTIONS: usize = 32;

#[derive(Default)]
pub(crate) struct Interactions {
	requested: HashMap<Snowflake, Instant>,
	sent: VecDeque<(String, String)>,
}

impl Interactions {
	fn remember(&mut self, nonce: String, name: String) {
		if self.sent.len() >= KEPT_INTERACTIONS {
			self.sent.pop_front();
		}
		self.sent.push_back((nonce, name));
	}

	fn settle(&mut self, nonce: &str) -> Option<String> {
		let index = self.sent.iter().position(|(sent, _)| sent == nonce)?;
		self.sent.remove(index).map(|(_, name)| name)
	}
}

pub(crate) fn interaction_body(
	index: &ApplicationCommandIndex,
	command_id: Snowflake,
	guild_id: Option<Snowflake>,
	channel_id: Snowflake,
	session_id: &str,
	nonce: &str,
	options: &[InteractionOption],
) -> Option<(String, Value)> {
	let command = index.command(command_id)?;
	if command.kind != ApplicationCommandType::ChatInput {
		return None;
	}
	let mut body = json!({
		"type": 2,
		"application_id": command.application_id,
		"channel_id": channel_id,
		"session_id": session_id,
		"data": {
			"version": command.version,
			"id": command.id,
			"name": command.name,
			"type": 1,
			"options": options,
			"application_command": command.raw,
			"attachments": [],
		},
		"nonce": nonce,
		"analytics_location": "slash_ui",
	});
	if let Some(guild_id) = guild_id {
		body["guild_id"] = json!(guild_id);
	}
	Some((command.name.clone(), body))
}

impl Core {
	pub(super) fn load_commands(&mut self, channel_id: Snowflake) {
		self.want_frecency();
		let (scope, guild_id, known) = {
			let state = self.state.read();
			let guild_id = state.channel_guild(channel_id);
			(
				state.command_scope(channel_id),
				guild_id,
				state.command_index(channel_id).is_some(),
			)
		};
		let now = Instant::now();
		let recent = self
			.interactions
			.requested
			.get(&scope)
			.is_some_and(|at| now.duration_since(*at) < INDEX_RETRY);
		if known || recent {
			return;
		}
		self.interactions.requested.insert(scope, now);
		let primary = match guild_id {
			Some(guild_id) => CommandScope::Guild(guild_id),
			None => CommandScope::Channel(channel_id),
		};
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let index = match rest.application_command_index(primary).await {
				Ok(mut index) => {
					match rest.application_command_index(CommandScope::User).await {
						Ok(user) => index.merge(user),
						Err(error) => tracing::debug!(%error, "user command index unavailable"),
					}
					Some(Box::new(index))
				}
				Err(error) => {
					tracing::warn!(%error, "application command index unavailable");
					None
				}
			};
			let _ = internal.send(Internal::CommandIndex { scope, index });
		});
	}

	pub(super) fn command_index_arrived(
		&mut self,
		scope: Snowflake,
		index: Option<Box<ApplicationCommandIndex>>,
	) {
		let Some(index) = index else {
			return;
		};
		self.apply_local(LocalEvent::CommandIndexLoaded {
			scope,
			index: Arc::new(*index),
		});
	}

	pub(super) fn run_command(
		&mut self,
		channel_id: Snowflake,
		command_id: Snowflake,
		options: Vec<InteractionOption>,
	) {
		let nonce = self.next_nonce();
		let prepared = {
			let state = self.state.read();
			let session_id = state.session().session_id.clone();
			match state.command_index(channel_id) {
				_ if session_id.is_empty() => Err("Commands need a gateway connection"),
				None => Err("Commands for this channel are still loading"),
				Some(index) => interaction_body(
					index,
					command_id,
					state.channel_guild(channel_id),
					channel_id,
					&session_id,
					&nonce.0,
					&options,
				)
				.ok_or("That command is no longer available"),
			}
		};
		let (name, body) = match prepared {
			Ok(prepared) => prepared,
			Err(reason) => {
				self.reject(reason);
				return;
			}
		};
		self.interactions.remember(nonce.0, name.clone());
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			if let Err(error) = rest.create_interaction(&body).await {
				tracing::warn!(%error, "application command was not sent");
				let what = format!("/{name}");
				let _ = internal.send(Internal::Rejected(actions::failure_reason(&what, &error)));
			}
		});
	}

	pub(super) fn interaction_settled(&mut self, value: &Value, failed: bool) {
		let Some(nonce) = value.get("nonce").and_then(Value::as_str) else {
			return;
		};
		let Some(name) = self.interactions.settle(nonce) else {
			return;
		};
		if failed {
			self.reject(format!("/{name} failed: the application did not respond"));
		}
	}
}
