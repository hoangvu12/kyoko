use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use indexmap::{IndexMap, IndexSet};
use protocol::{GatewayRateLimited, GuildMembersChunk, Snowflake};
use tokio::time::Instant;

pub(crate) const MAX_IDS_PER_REQUEST: usize = 100;
pub(crate) const SEARCH_LIMIT: u32 = 100;
const BATCH_DELAY: Duration = Duration::from_millis(50);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
const RECENT_TTL: Duration = Duration::from_secs(60);
const RECENT_CAP: usize = 4096;
const FALLBACK_RETRY: Duration = Duration::from_secs(5);
const MAX_RETRY: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lookup {
	Ids(Vec<Snowflake>),
	Query(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
	pub(crate) guild_id: Snowflake,
	pub(crate) nonce: String,
	pub(crate) lookup: Lookup,
}

#[derive(Default)]
struct Queue {
	ids: IndexSet<Snowflake>,
	query: Option<String>,
	ready_at: Option<Instant>,
}

impl Queue {
	fn is_empty(&self) -> bool {
		self.ids.is_empty() && self.query.is_none()
	}
}

struct InFlight {
	request: Request,
	expires: Instant,
}

#[derive(Default)]
pub(crate) struct MemberRequests {
	queues: IndexMap<Snowflake, Queue>,
	in_flight: HashMap<String, InFlight>,
	recent: HashMap<(Snowflake, Snowflake), Instant>,
	recent_order: VecDeque<((Snowflake, Snowflake), Instant)>,
	blocked: HashMap<Snowflake, Instant>,
	sequence: u64,
	live: bool,
}

impl MemberRequests {
	pub(crate) fn want_ids(
		&mut self,
		guild_id: Snowflake,
		ids: impl IntoIterator<Item = Snowflake>,
		now: Instant,
	) {
		let fresh: Vec<Snowflake> = ids
			.into_iter()
			.filter(|id| !self.is_recent(guild_id, *id, now) && !self.is_in_flight(guild_id, *id))
			.collect();
		if fresh.is_empty() {
			return;
		}
		let queue = self.queues.entry(guild_id).or_default();
		queue.ids.extend(fresh);
		queue.ready_at.get_or_insert(now + BATCH_DELAY);
	}

	pub(crate) fn search(&mut self, guild_id: Snowflake, query: String, now: Instant) {
		let queue = self.queues.entry(guild_id).or_default();
		queue.query = Some(query);
		queue.ready_at.get_or_insert(now + BATCH_DELAY);
	}

	pub(crate) fn connected(&mut self, now: Instant) {
		self.blocked.clear();
		let unanswered: Vec<InFlight> = self.in_flight.drain().map(|(_, flight)| flight).collect();
		for flight in unanswered {
			self.requeue(flight.request, now);
		}
		self.live = true;
	}

	pub(crate) fn offline(&mut self) {
		self.live = false;
	}

	pub(crate) fn deadline(&self) -> Option<Instant> {
		let expiry = self.in_flight.values().map(|flight| flight.expires).min();
		let next_send = self
			.queues
			.iter()
			.filter(|(guild_id, queue)| !queue.is_empty() && !self.guild_in_flight(**guild_id))
			.filter_map(|(guild_id, queue)| {
				let ready = queue.ready_at?;
				Some(
					self.blocked
						.get(guild_id)
						.map_or(ready, |until| ready.max(*until)),
				)
			})
			.min()
			.filter(|_| self.live);
		[expiry, next_send].into_iter().flatten().min()
	}

	pub(crate) fn due(&mut self, now: Instant) -> Vec<Request> {
		self.in_flight.retain(|_, flight| flight.expires > now);
		self.blocked.retain(|_, until| *until > now);
		if !self.live {
			return Vec::new();
		}
		let ready: Vec<Snowflake> = self
			.queues
			.iter()
			.filter(|(guild_id, queue)| {
				!queue.is_empty()
					&& queue.ready_at.is_some_and(|at| at <= now)
					&& !self.blocked.contains_key(guild_id)
					&& !self.guild_in_flight(**guild_id)
			})
			.map(|(guild_id, _)| *guild_id)
			.collect();
		let mut requests = Vec::new();
		for guild_id in ready {
			let Some(lookup) = self.take_lookup(guild_id, now) else {
				continue;
			};
			self.sequence += 1;
			let request = Request {
				guild_id,
				nonce: self.sequence.to_string(),
				lookup,
			};
			self.in_flight.insert(
				request.nonce.clone(),
				InFlight {
					request: request.clone(),
					expires: now + ANSWER_TIMEOUT,
				},
			);
			requests.push(request);
		}
		requests
	}

	pub(crate) fn unsent(&mut self, request: Request, now: Instant) {
		if self.in_flight.remove(&request.nonce).is_some() {
			self.requeue(request, now);
		}
	}

	pub(crate) fn answered(&mut self, chunk: &GuildMembersChunk, now: Instant) {
		let Some(nonce) = chunk.nonce.as_deref() else {
			return;
		};
		let Some(flight) = self.in_flight.get(nonce) else {
			return;
		};
		let guild_id = flight.request.guild_id;
		let users = chunk
			.members
			.iter()
			.filter_map(|member| member.user.as_ref().map(|user| user.id))
			.chain(chunk.not_found.iter().copied())
			.collect::<Vec<_>>();
		for user_id in users {
			self.remember(guild_id, user_id, now);
		}
		let last = chunk.chunk_index.unwrap_or(0) + 1 >= chunk.chunk_count.unwrap_or(1);
		if !last {
			return;
		}
		if let Some(InFlight {
			request: Request {
				lookup: Lookup::Ids(ids),
				..
			},
			..
		}) = self.in_flight.remove(nonce)
		{
			for user_id in ids {
				self.remember(guild_id, user_id, now);
			}
		}
	}

	pub(crate) fn rate_limited(&mut self, limited: &GatewayRateLimited, now: Instant) {
		let flight = limited
			.meta
			.nonce
			.as_deref()
			.and_then(|nonce| self.in_flight.remove(nonce));
		let Some(guild_id) = limited
			.meta
			.guild_id
			.or_else(|| flight.as_ref().map(|flight| flight.request.guild_id))
		else {
			return;
		};
		let retry = limited
			.retry_after
			.filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
			.map_or(FALLBACK_RETRY, |seconds| {
				Duration::from_secs_f64(seconds.min(MAX_RETRY.as_secs_f64()))
			});
		self.blocked.insert(guild_id, now + retry);
		if let Some(flight) = flight {
			self.requeue(flight.request, now);
		}
	}

	fn take_lookup(&mut self, guild_id: Snowflake, now: Instant) -> Option<Lookup> {
		let queue = self.queues.get_mut(&guild_id)?;
		let lookup = if let Some(query) = queue.query.take() {
			Lookup::Query(query)
		} else {
			let count = queue.ids.len().min(MAX_IDS_PER_REQUEST);
			Lookup::Ids(queue.ids.drain(..count).collect())
		};
		if queue.is_empty() {
			self.queues.shift_remove(&guild_id);
		} else {
			queue.ready_at = Some(now);
		}
		Some(lookup)
	}

	fn requeue(&mut self, request: Request, now: Instant) {
		let queue = self.queues.entry(request.guild_id).or_default();
		match request.lookup {
			Lookup::Ids(ids) => {
				let waiting: Vec<Snowflake> = queue.ids.drain(..).collect();
				queue.ids.extend(ids);
				queue.ids.extend(waiting);
			}
			Lookup::Query(query) => {
				queue.query.get_or_insert(query);
			}
		}
		queue.ready_at = Some(now);
	}

	fn guild_in_flight(&self, guild_id: Snowflake) -> bool {
		self.in_flight
			.values()
			.any(|flight| flight.request.guild_id == guild_id)
	}

	fn is_in_flight(&self, guild_id: Snowflake, user_id: Snowflake) -> bool {
		self.in_flight.values().any(|flight| {
			flight.request.guild_id == guild_id
				&& matches!(&flight.request.lookup, Lookup::Ids(ids) if ids.contains(&user_id))
		})
	}

	fn is_recent(&self, guild_id: Snowflake, user_id: Snowflake, now: Instant) -> bool {
		self.recent
			.get(&(guild_id, user_id))
			.is_some_and(|until| *until > now)
	}

	fn remember(&mut self, guild_id: Snowflake, user_id: Snowflake, now: Instant) {
		let until = now + RECENT_TTL;
		self.recent.insert((guild_id, user_id), until);
		self.recent_order.push_back(((guild_id, user_id), until));
		while self.recent_order.len() > RECENT_CAP
			|| self.recent_order.front().is_some_and(|(_, until)| *until <= now)
		{
			let Some((key, until)) = self.recent_order.pop_front() else {
				break;
			};
			if self.recent.get(&key) == Some(&until) {
				self.recent.remove(&key);
			}
		}
	}
}
