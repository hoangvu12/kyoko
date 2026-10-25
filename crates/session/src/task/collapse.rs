use std::collections::BTreeMap;
use std::time::Duration;

use protocol::Snowflake;
use rest::RestTuning;
use serde_json::{Map, Value, json};
use state::LocalEvent;
use tokio::time::Instant;

use super::{Core, Internal};

type Categories = BTreeMap<Snowflake, bool>;

pub(super) struct CollapseWriter {
	delay: Duration,
	gap: Duration,
	pending: BTreeMap<Snowflake, Categories>,
	in_flight: Option<(Snowflake, Categories)>,
	due: Option<Instant>,
	next_allowed: Option<Instant>,
}

impl CollapseWriter {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.settings_delay,
			gap: tuning.collapse_gap,
			pending: BTreeMap::new(),
			in_flight: None,
			due: None,
			next_allowed: None,
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		if self.in_flight.is_some() || self.pending.is_empty() {
			return None;
		}
		let due = self.due?;
		Some(self.next_allowed.map_or(due, |allowed| due.max(allowed)))
	}

	fn want(&mut self, guild_id: Snowflake, category_id: Snowflake, collapsed: bool) {
		self.pending
			.entry(guild_id)
			.or_default()
			.insert(category_id, collapsed);
		self.due = Some(Instant::now() + self.delay);
	}

	fn take(&mut self) -> Option<(Snowflake, Categories)> {
		let (guild_id, categories) = self.pending.pop_first()?;
		self.next_allowed = Some(Instant::now() + self.gap);
		self.in_flight = Some((guild_id, categories.clone()));
		Some((guild_id, categories))
	}

	fn finished(&mut self, saved: bool) -> Vec<(Snowflake, bool)> {
		let Some((guild_id, sent)) = self.in_flight.take() else {
			return Vec::new();
		};
		if saved {
			return Vec::new();
		}
		let newer = self.pending.get(&guild_id);
		sent.into_iter()
			.filter(|(category_id, _)| newer.is_none_or(|newer| !newer.contains_key(category_id)))
			.map(|(category_id, collapsed)| (category_id, !collapsed))
			.collect()
	}

	fn intended(&self) -> Categories {
		let mut wanted = Categories::new();
		if let Some((_, sent)) = &self.in_flight {
			wanted.extend(sent);
		}
		for categories in self.pending.values() {
			wanted.extend(categories);
		}
		wanted
	}
}

fn overrides_body(categories: &Categories) -> Value {
	let overrides: Map<String, Value> = categories
		.iter()
		.map(|(category_id, collapsed)| (category_id.to_string(), json!({ "collapsed": collapsed })))
		.collect();
	json!({ "channel_overrides": overrides })
}

impl Core {
	pub(super) fn toggle_category(&mut self, guild_id: Snowflake, category_id: Snowflake) {
		let collapsed = !self.state.read().is_collapsed(category_id);
		self.apply_local(LocalEvent::CategoryCollapsed {
			channel_id: category_id,
			collapsed,
		});
		self.collapse.want(guild_id, category_id, collapsed);
	}

	pub(super) fn flush_collapse(&mut self) {
		let Some((guild_id, categories)) = self.collapse.take() else {
			return;
		};
		let body = overrides_body(&categories);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let saved = match rest.patch_guild_settings(Some(guild_id), body).await {
				Ok(_) => true,
				Err(error) => {
					tracing::warn!(%error, "the collapsed categories were not saved");
					false
				}
			};
			let _ = internal.send(Internal::CollapseSaved(saved));
		});
	}

	pub(super) fn on_collapse_saved(&mut self, saved: bool) {
		let restored = self.collapse.finished(saved);
		if restored.is_empty() {
			return;
		}
		for (category_id, collapsed) in restored {
			self.apply_local(LocalEvent::CategoryCollapsed {
				channel_id: category_id,
				collapsed,
			});
		}
		self.reject("The collapsed categories could not be saved".to_owned());
	}

	pub(super) fn keep_intended_collapse(&mut self) {
		let wanted = self.collapse.intended();
		let differing: Vec<(Snowflake, bool)> = {
			let state = self.state.read();
			wanted
				.into_iter()
				.filter(|(category_id, collapsed)| state.is_collapsed(*category_id) != *collapsed)
				.collect()
		};
		for (category_id, collapsed) in differing {
			self.apply_local(LocalEvent::CategoryCollapsed {
				channel_id: category_id,
				collapsed,
			});
		}
	}
}
