use std::collections::HashSet;

use protocol::fingerprint::{ContextProperties, JoinLocation};
use protocol::{Permissions, Snowflake};
use rest::RestError;
use serde_json::{Map, Value, json};
use state::Loaded;

use super::actions::failure_reason;
use super::{Command, Core, Internal};
use crate::command::InviteSource;

const ANSWER_FIRST: &str = "Answer the required questions first";
const RULES_ELSEWHERE: &str =
	"This server asks questions kyoko can't show yet. Answer them in the official client.";

fn seen_map(seen: &[(Snowflake, i64)]) -> Value {
	let map: Map<String, Value> = seen.iter().map(|(id, at)| (id.to_string(), json!(at))).collect();
	Value::Object(map)
}

impl Core {
	pub(super) fn on_community_command(&mut self, command: Command) {
		match command {
			Command::CreateInvite {
				channel_id,
				max_age,
				max_uses,
				temporary,
			} => {
				if self.channel_allows(channel_id, Permissions::CREATE_INSTANT_INVITE, "Create Invite") {
					let body = json!({ "max_age": max_age, "max_uses": max_uses, "temporary": temporary });
					self.spawn_load("create invite", move |rest| async move {
						let raw = rest.create_invite(channel_id, body).await?;
						Ok(Loaded::InviteCreated { channel_id, raw })
					});
				}
			}
			Command::ResolveInvite { code } => self.resolve_invite(code),
			Command::AcceptInvite { code, from } => self.accept_invite(code, from),
			Command::LoadOnboarding { guild_id } => self.load_onboarding(guild_id),
			Command::SubmitOnboarding {
				guild_id,
				chosen,
				prompts_seen,
				options_seen,
			} => self.submit_onboarding(guild_id, chosen, &prompts_seen, &options_seen),
			Command::LoadMemberVerification {
				guild_id,
				invite_code,
			} => self.load_member_verification(guild_id, invite_code),
			Command::AcceptMemberVerification { guild_id } => self.accept_member_verification(guild_id),
			Command::ForwardMessage {
				channel_id,
				message_id,
				to_channel_id,
			} => self.forward_message(channel_id, message_id, to_channel_id),
			_ => {}
		}
	}

	fn resolve_invite(&self, code: String) {
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let message = match rest.invite(&code).await {
				Ok(raw) => Internal::Loaded(Loaded::InvitePreview { code, raw }),
				Err(RestError::NotFound) => Internal::Loaded(Loaded::InviteUnknown { code }),
				Err(error) => {
					tracing::warn!(%error, "the invite was not resolved");
					Internal::Rejected(failure_reason("Invite", &error))
				}
			};
			let _ = internal.send(message);
		});
	}

	fn join_context(&self, code: &str, from: InviteSource) -> ContextProperties {
		let state = self.state.read();
		let preview = state.invite_preview(code);
		let guild_id = preview.and_then(|preview| preview.guild_id);
		let channel_id = preview.and_then(|preview| preview.channel_id);
		let channel_type = preview.and_then(|preview| preview.channel_type);
		match from {
			InviteSource::AddServer => {
				ContextProperties::join(JoinLocation::JoinGuild, guild_id, channel_id, channel_type)
			}
			InviteSource::Message {
				channel_id: shown_in,
				message_id,
			} => {
				let kind = state.channel(shown_in).map(|channel| i64::from(channel.kind));
				ContextProperties::join(
					JoinLocation::InviteButtonEmbed,
					state.channel_guild(shown_in),
					Some(shown_in),
					kind,
				)
				.with_message(message_id)
			}
		}
	}

	fn accept_invite(&self, code: String, from: InviteSource) {
		let context = self.join_context(&code, from);
		let session_id = self.state.read().session().session_id.clone();
		let session_id = (!session_id.is_empty()).then_some(session_id);
		self.spawn_load("Join", move |rest| async move {
			let raw = rest.accept_invite(&code, session_id.as_deref(), &context).await?;
			Ok(Loaded::InviteAccepted { code, raw })
		});
	}

	pub(super) fn after_join(&mut self) {
		let Some(join) = self.state.read().last_join().cloned() else {
			return;
		};
		let Some(guild_id) = join.guild_id else {
			return;
		};
		if join.verification {
			self.load_member_verification(guild_id, Some(join.code));
		}
		if join.onboarding {
			self.load_onboarding(guild_id);
		}
	}

	fn load_onboarding(&self, guild_id: Snowflake) {
		self.spawn_load("Onboarding", move |rest| async move {
			let raw = rest.guild_onboarding(guild_id).await?;
			Ok(Loaded::Onboarding { guild_id, raw })
		});
	}

	fn submit_onboarding(
		&mut self,
		guild_id: Snowflake,
		chosen: Vec<Snowflake>,
		prompts_seen: &[(Snowflake, i64)],
		options_seen: &[(Snowflake, i64)],
	) {
		let picked = {
			let state = self.state.read();
			let Some(onboarding) = state.onboarding(guild_id) else {
				drop(state);
				self.reject("Onboarding: open the server again to load its questions");
				return;
			};
			let offered: HashSet<Snowflake> = onboarding
				.prompts
				.iter()
				.flat_map(|prompt| prompt.options.iter().map(|option| option.id))
				.collect();
			let picked: Vec<Snowflake> = chosen.into_iter().filter(|id| offered.contains(id)).collect();
			let answered = onboarding
				.prompts
				.iter()
				.filter(|prompt| prompt.required)
				.all(|prompt| prompt.options.iter().any(|option| picked.contains(&option.id)));
			answered.then_some(picked)
		};
		let Some(picked) = picked else {
			self.reject(ANSWER_FIRST);
			return;
		};
		let body = json!({
			"onboarding_responses": picked,
			"onboarding_prompts_seen": seen_map(prompts_seen),
			"onboarding_responses_seen": seen_map(options_seen),
		});
		self.spawn_load("Onboarding", move |rest| async move {
			rest.onboarding_responses(guild_id, body).await?;
			Ok(Loaded::OnboardingSubmitted { guild_id })
		});
	}

	fn load_member_verification(&self, guild_id: Snowflake, invite_code: Option<String>) {
		self.spawn_load("Server rules", move |rest| async move {
			let raw = rest.member_verification(guild_id, invite_code.as_deref()).await?;
			Ok(Loaded::MemberVerification { guild_id, raw })
		});
	}

	fn accept_member_verification(&mut self, guild_id: Snowflake) {
		let body = self
			.state
			.read()
			.member_verification(guild_id)
			.map(state::MemberVerification::agreement);
		match body {
			None => self.reject("Server rules: open the server again to load them"),
			Some(None) => self.reject(RULES_ELSEWHERE),
			Some(Some(body)) => self.spawn_load("Server rules", move |rest| async move {
				rest.submit_join_request(guild_id, body).await?;
				Ok(Loaded::MemberVerificationSubmitted { guild_id })
			}),
		}
	}

	fn forward_message(&mut self, channel_id: Snowflake, message_id: Snowflake, to_channel_id: Snowflake) {
		if !self.channel_allows(to_channel_id, Permissions::SEND_MESSAGES, "Send Messages") {
			return;
		}
		let guild_id = self.state.read().channel_guild(channel_id);
		let nonce = self.next_nonce();
		self.dispatch_forward(nonce, guild_id, channel_id, message_id, to_channel_id);
	}
}
