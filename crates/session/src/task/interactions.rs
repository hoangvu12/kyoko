use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use protocol::{
	ApplicationCommandIndex, ApplicationCommandType, AutocompleteResponse, InteractionOption, Snowflake,
};
use rest::UploadFile;
use rest::api::CommandScope;
use serde_json::{Value, json};
use state::{Autocomplete, AutocompleteStatus, LocalEvent};

use super::{Core, Internal, actions, outbox};

const INDEX_RETRY: Duration = Duration::from_secs(60);
const KEPT_INTERACTIONS: usize = 32;
pub(crate) const AUTOCOMPLETE_DEBOUNCE: Duration = Duration::from_millis(300);
pub(crate) const AUTOCOMPLETE_TIMEOUT: Duration = Duration::from_secs(5);
const APPLICATION_COMMAND: u8 = 2;
const APPLICATION_COMMAND_AUTOCOMPLETE: u8 = 4;

struct Waiting {
	request: u64,
	channel_id: Snowflake,
	command_id: Snowflake,
	options: Vec<InteractionOption>,
	nonce: Option<String>,
}

#[derive(Default)]
pub(crate) struct Interactions {
	requested: HashMap<Snowflake, Instant>,
	sent: VecDeque<(String, String)>,
	waiting: Option<Waiting>,
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

	fn waiting_for(&mut self, request: u64) -> Option<Waiting> {
		if self
			.waiting
			.as_ref()
			.is_some_and(|waiting| waiting.request == request)
		{
			self.waiting.take()
		} else {
			None
		}
	}
}

pub(crate) struct Interaction<'a> {
	pub(crate) kind: u8,
	pub(crate) command_id: Snowflake,
	pub(crate) guild_id: Option<Snowflake>,
	pub(crate) channel_id: Snowflake,
	pub(crate) session_id: &'a str,
	pub(crate) nonce: &'a str,
	pub(crate) options: &'a [InteractionOption],
}

pub(crate) fn interaction_body(
	index: &ApplicationCommandIndex,
	interaction: &Interaction<'_>,
) -> Option<(String, Value)> {
	let command = index.command(interaction.command_id)?;
	if command.kind != ApplicationCommandType::ChatInput {
		return None;
	}
	let mut body = json!({
		"type": interaction.kind,
		"application_id": command.application_id,
		"channel_id": interaction.channel_id,
		"session_id": interaction.session_id,
		"data": {
			"version": command.version,
			"id": command.id,
			"name": command.name,
			"type": 1,
			"options": interaction.options,
			"application_command": command.raw,
			"attachments": [],
		},
		"nonce": interaction.nonce,
	});
	if interaction.kind == APPLICATION_COMMAND {
		body["analytics_location"] = json!("slash_ui");
	}
	if let Some(guild_id) = interaction.guild_id {
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

	fn prepared_interaction(
		&self,
		kind: u8,
		channel_id: Snowflake,
		command_id: Snowflake,
		nonce: &str,
		options: &[InteractionOption],
	) -> Result<(String, Value), &'static str> {
		let state = self.state.read();
		let session_id = state.session().session_id.clone();
		match state.command_index(channel_id) {
			_ if session_id.is_empty() => Err("Commands need a gateway connection"),
			None => Err("Commands for this channel are still loading"),
			Some(index) => interaction_body(
				index,
				&Interaction {
					kind,
					command_id,
					guild_id: state.channel_guild(channel_id),
					channel_id,
					session_id: &session_id,
					nonce,
					options,
				},
			)
			.ok_or("That command is no longer available"),
		}
	}

	pub(super) fn run_command(
		&mut self,
		channel_id: Snowflake,
		command_id: Snowflake,
		options: Vec<InteractionOption>,
		files: Vec<UploadFile>,
	) {
		let nonce = self.next_nonce();
		let prepared =
			self.prepared_interaction(APPLICATION_COMMAND, channel_id, command_id, &nonce.0, &options);
		let (name, mut body) = match prepared {
			Ok(prepared) => prepared,
			Err(reason) => {
				self.reject(reason);
				return;
			}
		};
		let limit = self.state.read().upload_limit(channel_id);
		if let Some(reason) = outbox::too_large(&files, limit) {
			self.reject(reason);
			return;
		}
		self.interactions.remember(nonce.0, name.clone());
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let sent = async {
				if !files.is_empty() {
					let attachments = outbox::uploaded(&rest, &files, channel_id, None).await?;
					body["data"]["attachments"] = json!(attachments);
				}
				rest.create_interaction(&body).await
			};
			if let Err(error) = sent.await {
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

	fn autocomplete_status(&mut self, request: u64, status: AutocompleteStatus) {
		self.apply_local(LocalEvent::Autocomplete(Autocomplete { request, status }));
	}

	pub(super) fn autocomplete(
		&mut self,
		channel_id: Snowflake,
		command_id: Snowflake,
		options: Vec<InteractionOption>,
		request: u64,
	) {
		self.interactions.waiting = Some(Waiting {
			request,
			channel_id,
			command_id,
			options,
			nonce: None,
		});
		self.autocomplete_status(request, AutocompleteStatus::Loading);
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			tokio::time::sleep(AUTOCOMPLETE_DEBOUNCE).await;
			let _ = internal.send(Internal::AutocompleteDue(request));
		});
	}

	pub(super) fn autocomplete_due(&mut self, request: u64) {
		let Some(mut waiting) = self.interactions.waiting_for(request) else {
			return;
		};
		let nonce = self.next_nonce();
		let prepared = self.prepared_interaction(
			APPLICATION_COMMAND_AUTOCOMPLETE,
			waiting.channel_id,
			waiting.command_id,
			&nonce.0,
			&waiting.options,
		);
		let Ok((_, body)) = prepared else {
			self.autocomplete_status(request, AutocompleteStatus::Failed);
			return;
		};
		waiting.nonce = Some(nonce.0);
		self.interactions.waiting = Some(waiting);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			if let Err(error) = rest.create_interaction(&body).await {
				tracing::debug!(%error, "autocomplete was not sent");
				let _ = internal.send(Internal::AutocompleteFailed(request));
				return;
			}
			tokio::time::sleep(AUTOCOMPLETE_TIMEOUT).await;
			let _ = internal.send(Internal::AutocompleteFailed(request));
		});
	}

	pub(super) fn autocomplete_failed(&mut self, request: u64) {
		if self.interactions.waiting_for(request).is_some() {
			self.autocomplete_status(request, AutocompleteStatus::Failed);
		}
	}

	pub(super) fn autocomplete_answered(&mut self, response: AutocompleteResponse) {
		let answered = self
			.interactions
			.waiting
			.as_ref()
			.is_some_and(|waiting| waiting.nonce.is_some() && waiting.nonce == response.nonce);
		if !answered {
			return;
		}
		if let Some(waiting) = self.interactions.waiting.take() {
			self.autocomplete_status(
				waiting.request,
				AutocompleteStatus::Loaded(response.choices.into_vec()),
			);
		}
	}

	pub(super) fn autocomplete_held(&mut self, request: u64) {
		self.interactions.waiting = None;
		self.autocomplete_status(request, AutocompleteStatus::Failed);
	}
}
