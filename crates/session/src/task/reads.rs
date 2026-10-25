use std::collections::HashMap;
use std::time::Duration;

use protocol::{Channel, ChannelType, DispatchEvent, MessageAck, Permissions, Snowflake};
use rest::RestTuning;
use serde_json::json;
use state::State;
use tokio::time::Instant;

use super::{Core, ack_day};

const ACK_BULK_LIMIT: usize = 100;

struct PendingAck {
	message_id: Snowflake,
	due: Instant,
	latest: Instant,
}

pub(super) struct Acks {
	delay: Duration,
	jitter: Duration,
	max_delay: Duration,
	pending: HashMap<Snowflake, PendingAck>,
	unconfirmed: HashMap<Snowflake, Snowflake>,
}

impl Acks {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.ack_delay,
			jitter: tuning.ack_jitter,
			max_delay: tuning.ack_max_delay,
			pending: HashMap::new(),
			unconfirmed: HashMap::new(),
		}
	}

	pub(super) fn unconfirmed(&self) -> bool {
		!self.unconfirmed.is_empty()
	}

	fn expect_echo(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		self.unconfirmed.insert(channel_id, message_id);
	}

	fn confirm(&mut self, ack: &MessageAck) -> Option<Snowflake> {
		let expected = *self.unconfirmed.get(&ack.channel_id)?;
		if ack.message_id >= expected {
			self.unconfirmed.remove(&ack.channel_id);
			return None;
		}
		Some(expected)
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		self.pending.values().map(|pending| pending.due).min()
	}

	fn quiet_period(&self) -> Duration {
		self.delay + self.jitter.mul_f64(rand::random_range(0.0..=1.0))
	}

	fn note(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		let now = Instant::now();
		let due = now + self.quiet_period();
		let pending = self.pending.entry(channel_id).or_insert(PendingAck {
			message_id,
			due,
			latest: now + self.max_delay,
		});
		pending.message_id = pending.message_id.max(message_id);
		pending.due = due.min(pending.latest);
		let message_id = pending.message_id;
		self.expect_echo(channel_id, message_id);
	}

	fn take_due(&mut self) -> Vec<(Snowflake, Snowflake)> {
		let now = Instant::now();
		let due: Vec<(Snowflake, Snowflake)> = self
			.pending
			.iter()
			.filter(|(_, pending)| pending.due <= now)
			.map(|(&channel_id, pending)| (channel_id, pending.message_id))
			.collect();
		for (channel_id, _) in &due {
			self.pending.remove(channel_id);
		}
		due
	}

	fn forget(&mut self, channel_id: Snowflake) {
		self.pending.remove(&channel_id);
	}
}

fn unread_up_to(state: &State, channel: &Channel) -> Option<(Snowflake, Snowflake)> {
	let last = channel.last_message_id?;
	let read_state = state.read_state(channel.id)?;
	let behind = read_state.last_message_id < Some(last);
	let mentioned = read_state.mention_count.unwrap_or(0) > 0;
	(behind || mentioned).then_some((channel.id, last))
}

fn guild_unread(state: &State, guild_id: Snowflake) -> Vec<(Snowflake, Snowflake)> {
	let Some(guild) = state.guild(guild_id) else {
		return Vec::new();
	};
	guild
		.channels
		.values()
		.filter(|channel| channel.kind != ChannelType::GuildCategory)
		.filter(|channel| {
			state
				.my_permissions(channel.id)
				.contains(Permissions::VIEW_CHANNEL)
		})
		.filter_map(|channel| unread_up_to(state, channel))
		.collect()
}

impl Core {
	pub(super) fn flush_acks(&mut self) {
		for (channel_id, message_id) in self.acks.take_due() {
			let body = json!({ "token": null, "last_viewed": ack_day() });
			self.spawn_rest("ack", move |rest| async move {
				rest.ack_message(channel_id, message_id, body).await.map(drop)
			});
		}
	}

	pub(super) fn keep_own_ack(&mut self, ack: &MessageAck) {
		let Some(expected) = self.acks.confirm(ack) else {
			return;
		};
		self.apply_dispatch(&DispatchEvent::MessageAck(MessageAck {
			channel_id: ack.channel_id,
			message_id: expected,
			version: None,
			mention_count: Some(0),
		}));
	}

	pub(super) fn read_up_to(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		if self.ack_local(channel_id, message_id) {
			self.acks.note(channel_id, message_id);
		}
	}

	fn read_latest(&mut self, channel_id: Snowflake) {
		let latest = self
			.state
			.read()
			.channel(channel_id)
			.and_then(|channel| channel.last_message_id);
		if let Some(message_id) = latest {
			self.read_up_to(channel_id, message_id);
		}
	}

	fn is_viewing(&self, channel_id: Snowflake) -> bool {
		if !self.viewing || self.manual_unread.contains(&channel_id) {
			return false;
		}
		let state = self.state.read();
		state
			.selected()
			.is_some_and(|selected| selected.channel_id == channel_id)
			&& state
				.messages(channel_id)
				.and_then(|cache| cache.viewport())
				.is_none_or(|viewport| viewport.at_bottom)
	}

	pub(super) fn read_if_viewing(&mut self, channel_id: Snowflake) {
		if self.is_viewing(channel_id) {
			self.read_latest(channel_id);
		}
	}

	pub(super) fn on_channel_opened(&mut self, channel_id: Snowflake) {
		self.manual_unread.remove(&channel_id);
		self.read_if_viewing(channel_id);
	}

	pub(super) fn set_viewing(&mut self, viewing: bool) {
		self.viewing = viewing;
		let selected = self.state.read().selected();
		if let Some(selected) = selected {
			self.read_if_viewing(selected.channel_id);
		}
	}

	pub(super) fn mark_channel_read(&mut self, channel_id: Snowflake) {
		self.manual_unread.remove(&channel_id);
		self.read_latest(channel_id);
	}

	pub(super) fn mark_unread(&mut self, channel_id: Snowflake, message_id: Snowflake) {
		self.acks.forget(channel_id);
		self.manual_unread.insert(channel_id);
		let previous = self
			.state
			.read()
			.messages(channel_id)
			.and_then(|cache| {
				let index = cache
					.ordered
					.iter()
					.position(|message| message.id == message_id)?;
				cache.ordered.get(index.checked_sub(1)?).map(|message| message.id)
			})
			.unwrap_or(Snowflake(message_id.0.saturating_sub(1)));
		self.apply_dispatch(&DispatchEvent::MessageAck(MessageAck {
			channel_id,
			message_id: previous,
			version: None,
			mention_count: Some(0),
		}));
		self.acks.expect_echo(channel_id, previous);
		self.persist_read_state(channel_id);
		let body = json!({ "manual": true, "mention_count": 0 });
		self.spawn_rest("mark unread", move |rest| async move {
			rest.ack_message(channel_id, previous, body).await.map(drop)
		});
	}

	pub(super) fn mark_guild_read(&mut self, guild_id: Snowflake) {
		let unread = guild_unread(&self.state.read(), guild_id);
		self.read_in_bulk(unread);
	}

	pub(super) fn mark_all_read(&mut self) {
		let unread: Vec<(Snowflake, Snowflake)> = {
			let state = self.state.read();
			state
				.guilds()
				.flat_map(|guild| guild_unread(&state, guild.id))
				.chain(
					state
						.private_channels()
						.filter_map(|channel| unread_up_to(&state, channel)),
				)
				.collect()
		};
		self.read_in_bulk(unread);
	}

	fn read_in_bulk(&mut self, unread: Vec<(Snowflake, Snowflake)>) {
		if unread.is_empty() {
			return;
		}
		for &(channel_id, message_id) in &unread {
			self.ack_local(channel_id, message_id);
			self.acks.forget(channel_id);
			self.acks.expect_echo(channel_id, message_id);
			self.manual_unread.remove(&channel_id);
		}
		self.spawn_rest("mark read", move |rest| async move {
			for chunk in unread.chunks(ACK_BULK_LIMIT) {
				rest.ack_bulk(chunk.to_vec()).await?;
			}
			Ok(())
		});
	}
}
