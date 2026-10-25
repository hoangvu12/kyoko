use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use protocol::Snowflake;
use serde_json::{Map, Value, json};
use tokio::time::Instant;

const DEBOUNCE: Duration = Duration::from_millis(150);
const MAX_WAIT: Duration = Duration::from_millis(500);
const BUCKET_ROWS: u32 = 100;
const MAX_RANGES_PER_CHANNEL: usize = 3;
const MAX_LISTED_GUILDS: usize = 4;

pub(crate) type Range = [u32; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemberViewport {
	pub(crate) first: u32,
	pub(crate) last: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Want {
	channel_id: Option<Snowflake>,
	viewport: Option<MemberViewport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Sent {
	channel: Option<(Snowflake, Vec<Range>)>,
}

#[derive(Default)]
pub(crate) struct Subscriptions {
	wanted: VecDeque<(Snowflake, Want)>,
	open_guild: Option<Snowflake>,
	sent: HashMap<Snowflake, Sent>,
	due: Option<Instant>,
	first_change: Option<Instant>,
	live: bool,
}

pub(crate) fn member_ranges(viewport: Option<MemberViewport>) -> Vec<Range> {
	let mut buckets = vec![0];
	if let Some(MemberViewport { first, last }) = viewport {
		let (first, last) = (first.min(last), first.max(last));
		let (top, bottom) = (first / BUCKET_ROWS, last / BUCKET_ROWS);
		buckets.extend(top..=bottom);
		let above = first - top * BUCKET_ROWS;
		let below = bottom * BUCKET_ROWS + BUCKET_ROWS - 1 - last;
		buckets.push(if above < below && top > 0 {
			top - 1
		} else {
			bottom.saturating_add(1)
		});
	}
	let mut kept: Vec<u32> = Vec::new();
	for bucket in buckets {
		if !kept.contains(&bucket) && kept.len() < MAX_RANGES_PER_CHANNEL {
			kept.push(bucket);
		}
	}
	kept.sort_unstable();
	kept.into_iter()
		.map(|bucket| {
			let start = bucket.saturating_mul(BUCKET_ROWS);
			[start, start.saturating_add(BUCKET_ROWS - 1)]
		})
		.collect()
}

impl Subscriptions {
	pub(crate) fn open(&mut self, guild_id: Snowflake, channel_id: Option<Snowflake>, now: Instant) {
		self.open_guild = Some(guild_id);
		let mut want = self.take(guild_id).unwrap_or_default();
		if channel_id.is_some() && want.channel_id != channel_id {
			want.channel_id = channel_id;
			want.viewport = None;
		}
		self.push(guild_id, want, now);
	}

	pub(crate) fn leave_guilds(&mut self) {
		self.open_guild = None;
	}

	pub(crate) fn touch(&mut self, guild_id: Snowflake, fallback: Option<Snowflake>, now: Instant) {
		let mut want = self.take(guild_id).unwrap_or_default();
		if want.channel_id.is_none() {
			want.channel_id = fallback;
		}
		self.push(guild_id, want, now);
	}

	pub(crate) fn member_viewport(
		&mut self,
		guild_id: Snowflake,
		channel_id: Snowflake,
		viewport: MemberViewport,
		now: Instant,
	) {
		self.take(guild_id);
		self.push(
			guild_id,
			Want {
				channel_id: Some(channel_id),
				viewport: Some(viewport),
			},
			now,
		);
	}

	pub(crate) fn connected(&mut self, open: Option<(Snowflake, Snowflake)>, now: Instant) {
		self.sent.clear();
		self.wanted.clear();
		self.open_guild = None;
		self.due = None;
		self.first_change = None;
		self.live = true;
		if let Some((guild_id, channel_id)) = open {
			self.open(guild_id, Some(channel_id), now);
		}
	}

	pub(crate) fn offline(&mut self) {
		self.live = false;
	}

	pub(crate) fn deadline(&self) -> Option<Instant> {
		self.due.filter(|_| self.live)
	}

	pub(crate) fn flush(&mut self) -> Option<Map<String, Value>> {
		self.due = None;
		self.first_change = None;
		let mut subscriptions = Map::new();
		let listed: Vec<(Snowflake, Want)> = self.wanted.iter().copied().collect();
		for (guild_id, sent) in &mut self.sent {
			if listed.iter().any(|(wanted, _)| wanted == guild_id) {
				continue;
			}
			if let Some((channel_id, _)) = sent.channel.take() {
				subscriptions.insert(
					guild_id.to_string(),
					json!({ "channels": { channel_id.to_string(): [] } }),
				);
			}
		}
		for (guild_id, want) in listed {
			let target = Sent {
				channel: want
					.channel_id
					.map(|channel_id| (channel_id, member_ranges(want.viewport))),
			};
			let previous = self.sent.get(&guild_id);
			if previous == Some(&target) {
				continue;
			}
			let mut channels = Map::new();
			if let Some((old, _)) = previous.and_then(|sent| sent.channel.as_ref())
				&& target.channel.as_ref().is_none_or(|(new, _)| new != old)
			{
				channels.insert(old.to_string(), json!([]));
			}
			if let Some((channel_id, ranges)) = &target.channel {
				channels.insert(channel_id.to_string(), json!(ranges));
			}
			let entry = if previous.is_some() {
				json!({ "channels": channels })
			} else {
				json!({
					"typing": true,
					"threads": true,
					"activities": true,
					"member_updates": false,
					"members": [],
					"thread_member_lists": [],
					"channels": channels,
				})
			};
			subscriptions.insert(guild_id.to_string(), entry);
			self.sent.insert(guild_id, target);
		}
		(!subscriptions.is_empty()).then_some(subscriptions)
	}

	fn take(&mut self, guild_id: Snowflake) -> Option<Want> {
		let index = self.wanted.iter().position(|(wanted, _)| *wanted == guild_id)?;
		self.wanted.remove(index).map(|(_, want)| want)
	}

	fn push(&mut self, guild_id: Snowflake, want: Want, now: Instant) {
		self.wanted.push_back((guild_id, want));
		while self.wanted.len() > MAX_LISTED_GUILDS {
			let Some(oldest) = self
				.wanted
				.iter()
				.position(|(wanted, _)| Some(*wanted) != self.open_guild)
			else {
				break;
			};
			self.wanted.remove(oldest);
		}
		let first = *self.first_change.get_or_insert(now);
		self.due = Some((now + DEBOUNCE).min(first + MAX_WAIT));
	}
}
