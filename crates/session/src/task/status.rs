use std::time::Duration;

use chrono::{DateTime, Utc};
use protocol::{ProtoStatusSettings, StatusKind, encode_status_settings};
use rest::RestTuning;
use serde_json::{Value, json};
use state::{Changes, CustomStatus, LocalEvent, StatusEmoji};
use tokio::time::Instant;

use super::actions::failure_reason;
use super::{Core, Internal};

pub(super) struct StatusWriter {
	delay: Duration,
	gap: Duration,
	pending: Option<ProtoStatusSettings>,
	due: Option<Instant>,
	next_allowed: Option<Instant>,
	in_flight: Option<ProtoStatusSettings>,
	confirmed: Option<ProtoStatusSettings>,
	expiry_handled: Option<DateTime<Utc>>,
}

impl StatusWriter {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.settings_delay,
			gap: tuning.settings_gap,
			pending: None,
			due: None,
			next_allowed: None,
			in_flight: None,
			confirmed: None,
			expiry_handled: None,
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		if self.in_flight.is_some() || self.pending.is_none() {
			return None;
		}
		self.due
	}

	fn want(&mut self, settings: ProtoStatusSettings, current: ProtoStatusSettings) {
		if self.confirmed.is_none() {
			self.confirmed = Some(current);
		}
		let quiet = Instant::now() + self.delay;
		self.due = Some(self.next_allowed.map_or(quiet, |allowed| quiet.max(allowed)));
		self.pending = Some(settings);
	}

	fn intended(&self) -> Option<ProtoStatusSettings> {
		self.pending.clone().or_else(|| self.in_flight.clone())
	}

	fn take(&mut self) -> Option<ProtoStatusSettings> {
		let settings = self.pending.take()?;
		self.due = None;
		self.next_allowed = Some(Instant::now() + self.gap);
		self.in_flight = Some(settings.clone());
		Some(settings)
	}

	fn finished(&mut self, saved: bool) -> Option<ProtoStatusSettings> {
		let sent = self.in_flight.take();
		match (saved, self.pending.is_some()) {
			(true, true) => {
				self.confirmed = sent;
				None
			}
			(true, false) => {
				self.confirmed = None;
				None
			}
			(false, true) => None,
			(false, false) => self.confirmed.take(),
		}
	}
}

fn custom_activity(custom: &CustomStatus) -> Value {
	let mut activity = json!({ "name": "Custom Status", "type": 4 });
	if let Some(text) = &custom.text {
		activity["state"] = json!(text);
	}
	if let Some(emoji) = &custom.emoji {
		activity["emoji"] = json!({
			"id": emoji.id.map(|id| id.to_string()),
			"name": emoji.name,
			"animated": emoji.animated,
		});
	}
	activity
}

impl Core {
	pub(super) fn set_status(&mut self, status: StatusKind) {
		let current = self.state.read().status_settings().clone();
		let mut next = current.clone();
		next.status = Some(status.as_str().to_owned());
		next.status_expires_at_ms = None;
		self.change_status_settings(current, next);
	}

	pub(super) fn set_custom_status(
		&mut self,
		text: Option<String>,
		emoji: Option<String>,
		expires: Option<DateTime<Utc>>,
	) {
		let now = Utc::now();
		let text = text.filter(|text| !text.trim().is_empty());
		let emoji = emoji.filter(|name| !name.is_empty()).map(|name| StatusEmoji {
			id: None,
			name: Some(name),
			animated: false,
		});
		let custom = (text.is_some() || emoji.is_some()).then(|| CustomStatus {
			text,
			emoji,
			expires_at: expires.filter(|expires| *expires > now),
		});
		let current = self.state.read().status_settings().clone();
		let mut next = current.clone();
		next.custom_status = custom.map(|custom| custom.to_proto(now));
		self.change_status_settings(current, next);
	}

	fn change_status_settings(&mut self, current: ProtoStatusSettings, next: ProtoStatusSettings) {
		if current == next {
			return;
		}
		self.status.want(next.clone(), current);
		self.apply_local(LocalEvent::StatusSettings(next));
		self.publish_presence();
	}

	pub(super) fn flush_status(&mut self) {
		let Some(settings) = self.status.take() else {
			return;
		};
		let encoded = encode_status_settings(&settings);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let failure = match rest.patch_preloaded_settings(encoded).await {
				Ok(_) => None,
				Err(error) => {
					tracing::warn!(%error, "the status was not saved");
					Some(failure_reason("Your status could not be saved", &error))
				}
			};
			let _ = internal.send(Internal::StatusSaved(failure));
		});
	}

	pub(super) fn on_status_saved(&mut self, failure: Option<String>) {
		if let Some(previous) = self.status.finished(failure.is_none()) {
			self.apply_local(LocalEvent::StatusSettings(previous));
			self.publish_presence();
			self.reject(failure.unwrap_or_else(|| "Your status could not be saved".to_owned()));
		}
	}

	pub(super) fn status_deadline(&self) -> Option<Instant> {
		let expires = self.state.read().own_custom_status(Utc::now())?.expires_at?;
		if self.status.expiry_handled == Some(expires) {
			return None;
		}
		let left = (expires - Utc::now()).to_std().unwrap_or_default();
		Some(Instant::now() + left)
	}

	pub(super) fn expire_custom_status(&mut self) {
		let expires = self
			.state
			.read()
			.status_settings()
			.custom_status
			.as_ref()
			.and_then(CustomStatus::from_proto)
			.and_then(|custom| custom.expires_at);
		let Some(expires) = expires.filter(|expires| *expires <= Utc::now()) else {
			return;
		};
		if self.status.expiry_handled == Some(expires) {
			return;
		}
		self.status.expiry_handled = Some(expires);
		self.publish_presence();
		let mut changes = Changes::default();
		if let Some(me) = self.state.read().current_user_id() {
			changes.presence.insert(me);
		}
		self.notifier.push(changes);
	}

	pub(super) fn keep_intended_status(&mut self) {
		let Some(intended) = self.status.intended() else {
			return;
		};
		if *self.state.read().status_settings() != intended {
			self.apply_local(LocalEvent::StatusSettings(intended));
		}
	}

	pub(super) fn is_dnd(&self) -> bool {
		self.state.read().own_status() == StatusKind::Dnd
	}

	pub(super) fn publish_presence(&self) {
		let (status, custom) = {
			let state = self.state.read();
			(state.own_status(), state.own_custom_status(Utc::now()))
		};
		let activities = custom.iter().map(custom_activity).collect();
		let status = if self.afk && status == StatusKind::Online {
			StatusKind::Idle
		} else {
			status
		};
		if let Err(error) = self.gateway.send_presence(status.as_str(), activities, self.afk) {
			tracing::warn!(%error, "presence was not sent");
		}
	}
}
