use std::time::Duration;

use rest::RestTuning;
use serde_json::Value;
use state::{Loaded, LocalEvent, RailEntry, guild_folders_proto, guild_folders_settings};
use tokio::time::Instant;

use super::{Core, Internal};

pub(super) struct LayoutWriter {
	delay: Duration,
	gap: Duration,
	pending: Option<Vec<RailEntry>>,
	due: Option<Instant>,
	next_allowed: Option<Instant>,
	in_flight: Option<Value>,
	confirmed: Option<Value>,
}

impl LayoutWriter {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.settings_delay,
			gap: tuning.settings_gap,
			pending: None,
			due: None,
			next_allowed: None,
			in_flight: None,
			confirmed: None,
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		if self.in_flight.is_some() || self.pending.is_none() {
			return None;
		}
		self.due
	}

	fn want(&mut self, layout: Vec<RailEntry>, current: impl FnOnce() -> Value) {
		if self.confirmed.is_none() {
			self.confirmed = Some(current());
		}
		let now = Instant::now();
		let quiet = now + self.delay;
		self.due = Some(self.next_allowed.map_or(quiet, |allowed| quiet.max(allowed)));
		self.pending = Some(layout);
	}

	fn take(&mut self) -> Option<Vec<RailEntry>> {
		let layout = self.pending.take()?;
		self.due = None;
		self.next_allowed = Some(Instant::now() + self.gap);
		self.in_flight = Some(guild_folders_settings(&layout));
		Some(layout)
	}

	fn finished(&mut self, saved: bool) -> Option<Value> {
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

impl Core {
	pub(super) fn set_guild_layout(&mut self, layout: &[RailEntry]) {
		let state = self.state.clone();
		self.layout.want(layout.to_vec(), || {
			guild_folders_settings(&state.read().guild_layout())
		});
		let body = guild_folders_settings(layout);
		self.apply_local(LocalEvent::Loaded(Box::new(Loaded::UserSettings(body))));
	}

	pub(super) fn flush_layout(&mut self) {
		let Some(layout) = self.layout.take() else {
			return;
		};
		let settings = protocol::encode_guild_folders(&guild_folders_proto(&layout));
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let saved = match rest.patch_preloaded_settings(settings).await {
				Ok(_) => true,
				Err(error) => {
					tracing::warn!(%error, "the server order was not saved");
					false
				}
			};
			let _ = internal.send(Internal::LayoutSaved(saved));
		});
	}

	pub(super) fn on_layout_saved(&mut self, saved: bool) {
		if let Some(previous) = self.layout.finished(saved) {
			self.apply_local(LocalEvent::Loaded(Box::new(Loaded::UserSettings(previous))));
			self.reject("The server order could not be saved".to_owned());
		}
	}
}
