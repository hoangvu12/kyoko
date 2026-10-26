use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use protocol::{
	ApplicationCommandIndex, ApplicationCommandType, AutocompleteResponse, CommandOptionType,
	InteractionOption, Message, Nonce, Snowflake, User,
};
use rest::UploadFile;
use rest::api::CommandScope;
use serde_json::{Value, json};
use state::{Autocomplete, AutocompleteStatus, CommandStage, DEFERRED_REPLY_WINDOW, LocalEvent};

use super::{Core, Internal, actions, outbox};

const INDEX_RETRY: Duration = Duration::from_secs(60);
const KEPT_INTERACTIONS: usize = 32;
pub(crate) const AUTOCOMPLETE_DEBOUNCE: Duration = Duration::from_millis(300);
pub(crate) const AUTOCOMPLETE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const INTERACTION_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const NO_RESPONSE: &str = "The application did not respond";
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

	fn knows(&self, nonce: &str) -> bool {
		self.sent.iter().any(|(sent, _)| sent == nonce)
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

fn invoked_name(name: &str, options: &[InteractionOption]) -> String {
	let mut invoked = name.to_owned();
	let mut level = options;
	while let [only] = level
		&& matches!(
			only.kind,
			CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
		) {
		invoked.push(' ');
		invoked.push_str(&only.name);
		level = &only.options;
	}
	invoked
}

fn application_user(index: &ApplicationCommandIndex, command_id: Snowflake) -> Option<User> {
	let command = index.command(command_id)?;
	let application = index.application(command.application_id);
	let name = application.map_or_else(|| command.name.clone(), |application| application.name.clone());
	Some(User {
		id: application
			.and_then(|application| application.bot_id)
			.unwrap_or(command.application_id),
		username: name.clone(),
		global_name: Some(name),
		bot: Some(true),
		..User::default()
	})
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
		let (limit, application, user) = {
			let state = self.state.read();
			(
				state.upload_limit(channel_id),
				state
					.command_index(channel_id)
					.and_then(|index| application_user(index, command_id)),
				state.current_user().cloned(),
			)
		};
		if let Some(reason) = outbox::too_large(&files, limit) {
			self.reject(reason);
			return;
		}
		if let (Some(application), Some(user)) = (application, user) {
			self.apply_local(LocalEvent::PendingCommand {
				channel_id,
				nonce: nonce.clone(),
				name: invoked_name(&name, &options),
				application: Box::new(application),
				user: Box::new(user),
			});
		}
		self.interactions.remember(nonce.0.clone(), name.clone());
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let sent = async {
				if !files.is_empty() {
					let attachments = outbox::uploaded(&rest, &files, channel_id, None).await?;
					body["data"]["attachments"] = json!(attachments);
				}
				match tokio::time::timeout(INTERACTION_REQUEST_TIMEOUT, rest.create_interaction(&body)).await
				{
					Ok(sent) => sent.map(|()| true),
					Err(_) => Ok(false),
				}
			};
			let reason = match sent.await {
				Ok(true) => return,
				Ok(false) => NO_RESPONSE.to_owned(),
				Err(error) => {
					tracing::warn!(%error, "application command was not sent");
					actions::failure_reason(&format!("/{name}"), &error)
				}
			};
			let _ = internal.send(Internal::CommandFailed { nonce, reason });
		});
	}

	pub(super) fn command_failed(&mut self, nonce: Nonce, reason: String) {
		if self.interactions.settle(&nonce.0).is_some() {
			self.apply_local(LocalEvent::CommandAdvanced {
				nonce,
				stage: CommandStage::Failed(reason),
			});
		}
	}

	pub(super) fn command_became_modal(&mut self, nonce: Nonce) {
		self.interactions.settle(&nonce.0);
		self.apply_local(LocalEvent::PendingDiscarded { nonce });
	}

	pub(super) fn interaction_created(&mut self, value: &Value) {
		let Some(nonce) = value.get("nonce").and_then(Value::as_str) else {
			return;
		};
		if self.interactions.knows(nonce) {
			self.apply_local(LocalEvent::CommandAdvanced {
				nonce: Nonce(nonce.to_owned()),
				stage: CommandStage::Thinking,
			});
		}
	}

	pub(super) fn interaction_settled(&mut self, value: &Value, failed: bool) {
		let Some(nonce) = value.get("nonce").and_then(Value::as_str) else {
			return;
		};
		if self.component_settled(nonce, failed) || self.modal_settled(nonce, failed) {
			return;
		}
		if self.interactions.settle(nonce).is_some() && failed {
			self.apply_local(LocalEvent::CommandAdvanced {
				nonce: Nonce(nonce.to_owned()),
				stage: CommandStage::Failed(NO_RESPONSE.to_owned()),
			});
		}
	}

	pub(super) fn watch_deferred_reply(&mut self, message: &Message) {
		if !message.is_loading() {
			return;
		}
		let (channel_id, id) = (message.channel_id, message.id);
		let deadline = id.timestamp() + DEFERRED_REPLY_WINDOW;
		let left = (deadline - Utc::now()).to_std().unwrap_or_default();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			tokio::time::sleep(left).await;
			let _ = internal.send(Internal::InteractionExpired { channel_id, id });
		});
	}

	pub(super) fn dismiss_message(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		self.apply_local(LocalEvent::EphemeralDismissed {
			channel_id,
			id: message_id,
		});
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
