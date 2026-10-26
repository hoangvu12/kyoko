use std::collections::VecDeque;
use std::time::Duration;

use protocol::{Component, ComponentType, InteractionModal, Nonce, Snowflake};
use serde_json::{Value, json};
use state::{LocalEvent, ModalStatus};

use super::interactions::INTERACTION_REQUEST_TIMEOUT;
use super::{Core, Internal};
use crate::command::{ModalAnswer, ModalValue};

const KEPT_PRESSES: usize = 32;
const MESSAGE_COMPONENT: u8 = 3;
const MODAL_SUBMIT: u8 = 5;
pub(crate) const MODAL_TIMEOUT: Duration = Duration::from_secs(10);

struct Press {
	nonce: String,
	channel_id: Snowflake,
	message_id: Snowflake,
}

#[derive(Default)]
pub(crate) struct Components {
	pressed: VecDeque<Press>,
	submitted: Option<String>,
}

impl Components {
	fn remember(&mut self, press: Press) {
		if self.pressed.len() >= KEPT_PRESSES {
			self.pressed.pop_front();
		}
		self.pressed.push_back(press);
	}

	fn waiting_on(&self, message_id: Snowflake) -> bool {
		self.pressed.iter().any(|press| press.message_id == message_id)
	}

	fn settle(&mut self, nonce: &str) -> Option<Press> {
		let index = self.pressed.iter().position(|press| press.nonce == nonce)?;
		self.pressed.remove(index)
	}

	fn settle_modal(&mut self, nonce: &str) -> bool {
		if self.submitted.as_deref() == Some(nonce) {
			self.submitted = None;
			true
		} else {
			false
		}
	}
}

pub(crate) fn component_data(custom_id: &str, kind: ComponentType, values: Option<&[String]>) -> Value {
	let mut data = json!({ "component_type": kind, "custom_id": custom_id });
	if let Some(values) = values {
		data["type"] = json!(kind);
		data["values"] = json!(values);
	}
	data
}

fn answer_for<'a>(answers: &'a [ModalAnswer], custom_id: Option<&str>) -> Option<&'a ModalValue> {
	let custom_id = custom_id?;
	answers
		.iter()
		.find(|answer| answer.custom_id == custom_id)
		.map(|answer| &answer.value)
}

fn submitted(component: &Component, answers: &[ModalAnswer]) -> Option<Value> {
	let custom_id = component.custom_id.as_deref();
	let answer = answer_for(answers, custom_id);
	Some(match component.kind {
		ComponentType::ActionRow => json!({
			"type": component.kind,
			"components": submitted_all(&component.components, answers),
		}),
		ComponentType::Label => json!({
			"type": component.kind,
			"component": submitted(component.component.as_deref()?, answers)?,
		}),
		ComponentType::TextDisplay => json!({ "type": component.kind }),
		ComponentType::TextInput => json!({
			"type": component.kind,
			"custom_id": custom_id,
			"value": match answer {
				Some(ModalValue::Text(text)) => json!(text),
				_ => Value::Null,
			},
		}),
		_ if component.is_select() => json!({
			"type": component.kind,
			"custom_id": custom_id,
			"values": match answer {
				Some(ModalValue::Values(values)) => json!(values),
				_ => Value::Null,
			},
		}),
		_ => return None,
	})
}

fn submitted_all(components: &[Component], answers: &[ModalAnswer]) -> Vec<Value> {
	components
		.iter()
		.filter_map(|component| submitted(component, answers))
		.collect()
}

fn fields(components: &[Component]) -> Vec<&Component> {
	let mut out = Vec::new();
	for component in components {
		match component.kind {
			ComponentType::TextInput => out.push(component),
			_ if component.is_select() => out.push(component),
			_ => {
				out.extend(fields(&component.components));
				if let Some(child) = component.component.as_deref() {
					out.extend(fields(std::slice::from_ref(child)));
				}
			}
		}
	}
	out
}

pub(crate) fn missing_fields(modal: &InteractionModal, answers: &[ModalAnswer]) -> Vec<String> {
	fields(&modal.components)
		.into_iter()
		.filter_map(|field| {
			let custom_id = field.custom_id.clone()?;
			let answer = answer_for(answers, Some(&custom_id));
			let empty = answer.is_none_or(ModalValue::is_empty);
			let short = match answer {
				Some(ModalValue::Text(text)) if !text.is_empty() => field
					.min_length
					.is_some_and(|min| (text.chars().count() as i64) < min),
				_ => false,
			};
			((empty && field.is_required()) || short).then_some(custom_id)
		})
		.collect()
}

impl Core {
	pub(super) fn press_component(
		&mut self,
		channel_id: Snowflake,
		message_id: Snowflake,
		custom_id: String,
		kind: ComponentType,
		values: Option<Vec<String>>,
	) {
		if self.components.waiting_on(message_id) {
			return;
		}
		let (session_id, guild_id, message) = {
			let state = self.state.read();
			let message = state.messages(channel_id).and_then(|cache| {
				cache
					.by_id
					.get(&message_id)
					.or_else(|| cache.ephemeral().iter().find(|message| message.id == message_id))
					.cloned()
			});
			(
				state.session().session_id.clone(),
				state.channel_guild(channel_id),
				message,
			)
		};
		let Some(message) = message else {
			self.reject("That message is no longer available");
			return;
		};
		if session_id.is_empty() {
			self.reject("Components need a gateway connection");
			return;
		}
		let nonce = self.next_nonce().0;
		let application_id = message.application_id.unwrap_or(message.author.id);
		let mut body = json!({
			"type": MESSAGE_COMPONENT,
			"nonce": nonce,
			"channel_id": channel_id,
			"message_flags": message.flags.bits(),
			"message_id": message_id,
			"application_id": application_id,
			"session_id": session_id,
			"data": component_data(&custom_id, kind, values.as_deref()),
		});
		if let Some(guild_id) = guild_id {
			body["guild_id"] = json!(guild_id);
		}
		self.apply_local(LocalEvent::ComponentInteraction {
			channel_id,
			id: message_id,
			failed: false,
		});
		self.apply_local(LocalEvent::ComponentWaiting {
			channel_id,
			id: message_id,
			custom_id: Some(custom_id),
		});
		self.components.remember(Press {
			nonce: nonce.clone(),
			channel_id,
			message_id,
		});
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let sent = tokio::time::timeout(INTERACTION_REQUEST_TIMEOUT, rest.post_interaction(&body)).await;
			match sent {
				Ok(Ok(())) => return,
				Ok(Err(error)) => tracing::warn!(%error, "component interaction was not sent"),
				Err(_) => tracing::debug!("component interaction got no answer in time"),
			}
			let _ = internal.send(Internal::ComponentFailed(nonce));
		});
	}

	pub(super) fn component_failed(&mut self, nonce: &str) {
		self.component_settled(nonce, true);
	}

	pub(super) fn component_settled(&mut self, nonce: &str, failed: bool) -> bool {
		let Some(press) = self.components.settle(nonce) else {
			return false;
		};
		self.apply_local(LocalEvent::ComponentWaiting {
			channel_id: press.channel_id,
			id: press.message_id,
			custom_id: None,
		});
		if failed {
			self.apply_local(LocalEvent::ComponentInteraction {
				channel_id: press.channel_id,
				id: press.message_id,
				failed: true,
			});
		}
		true
	}

	pub(super) fn modal_settled(&mut self, nonce: &str, failed: bool) -> bool {
		if !self.components.settle_modal(nonce) {
			return false;
		}
		self.apply_local(if failed {
			LocalEvent::ModalChanged {
				status: ModalStatus::Failed,
				missing: Vec::new(),
			}
		} else {
			LocalEvent::ModalClosed
		});
		true
	}

	pub(super) fn modal_failed(&mut self, nonce: &str) {
		self.modal_settled(nonce, true);
	}

	pub(super) fn modal_created(&mut self, modal: InteractionModal) {
		if let Some(nonce) = modal.nonce.clone() {
			self.component_settled(&nonce, false);
			self.command_became_modal(Nonce(nonce));
		}
		self.components.submitted = None;
		self.apply_local(LocalEvent::ModalOpened(Box::new(modal)));
	}

	pub(super) fn submit_modal(&mut self, answers: Vec<ModalAnswer>) {
		let (modal, session_id, guild_id) = {
			let state = self.state.read();
			let Some(open) = state.app_modal() else {
				return;
			};
			if open.status == ModalStatus::Submitting {
				return;
			}
			(
				open.modal.clone(),
				state.session().session_id.clone(),
				state.channel_guild(open.modal.channel_id),
			)
		};
		let missing = missing_fields(&modal, &answers);
		if !missing.is_empty() {
			self.apply_local(LocalEvent::ModalChanged {
				status: ModalStatus::Open,
				missing,
			});
			return;
		}
		let nonce = self.next_nonce().0;
		let mut body = json!({
			"type": MODAL_SUBMIT,
			"application_id": modal.application.id,
			"channel_id": modal.channel_id,
			"data": {
				"id": modal.id,
				"custom_id": modal.custom_id,
				"components": submitted_all(&modal.components, &answers),
			},
			"session_id": session_id,
			"nonce": nonce,
		});
		if let Some(guild_id) = guild_id {
			body["guild_id"] = json!(guild_id);
		}
		self.components.submitted = Some(nonce.clone());
		self.apply_local(LocalEvent::ModalChanged {
			status: ModalStatus::Submitting,
			missing: Vec::new(),
		});
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			if let Err(error) = rest.post_interaction(&body).await {
				tracing::warn!(%error, "modal was not submitted");
			} else {
				tokio::time::sleep(MODAL_TIMEOUT).await;
			}
			let _ = internal.send(Internal::ModalFailed(nonce));
		});
	}

	pub(super) fn close_modal(&mut self) {
		self.components.submitted = None;
		self.apply_local(LocalEvent::ModalClosed);
	}
}
