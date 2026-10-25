use std::collections::VecDeque;
use std::time::Duration;

use indexmap::{IndexMap, IndexSet};
use protocol::{GuildMembersChunk, Message, MessagesQuery, Snowflake, User};
use rest::{RestClient, RestError, RestTuning};
use state::{LocalEvent, State};
use tokio::time::Instant;

use super::{Core, Internal};

const MAX_QUEUED: usize = 200;
const MAX_REMEMBERED: usize = 4096;
const MINUTE: Duration = Duration::from_secs(60);
const CAPTCHA_PAUSE: Duration = Duration::from_secs(600);
const FAILURE_PAUSE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Target {
	Message(Snowflake),
	User(Snowflake),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
	Target(Target),
	Member(Snowflake, Snowflake),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Job {
	channel_id: Snowflake,
	target: Target,
}

#[derive(Debug)]
pub(crate) enum Outcome {
	Message(Box<Message>),
	User(Box<User>),
	Gone,
	Pause { wait: Duration, retry: bool },
	Stop,
	Later,
}

fn classify(error: &RestError) -> Outcome {
	match error {
		RestError::NotFound | RestError::Forbidden | RestError::MissingPermissions => Outcome::Gone,
		RestError::RateLimited { retry_after } => Outcome::Pause {
			wait: *retry_after,
			retry: true,
		},
		RestError::Suspended { retry_in, .. } => Outcome::Pause {
			wait: *retry_in,
			retry: false,
		},
		RestError::Captcha { .. } => Outcome::Pause {
			wait: CAPTCHA_PAUSE,
			retry: false,
		},
		RestError::Unauthorized => Outcome::Stop,
		_ => Outcome::Later,
	}
}

async fn fetch(rest: RestClient, job: Job) -> Outcome {
	match job.target {
		Target::Message(message_id) => {
			let query = MessagesQuery {
				around: Some(message_id),
				limit: Some(1),
				..MessagesQuery::default()
			};
			match rest.messages(job.channel_id, query).await {
				Ok(messages) => messages
					.into_vec()
					.into_iter()
					.find(|message| message.id == message_id)
					.map_or(Outcome::Gone, |message| Outcome::Message(Box::new(message))),
				Err(error) => {
					tracing::debug!(%error, "referenced message was not fetched");
					classify(&error)
				}
			}
		}
		Target::User(user_id) => match rest.user(user_id).await {
			Ok(user) => Outcome::User(Box::new(user)),
			Err(error) => {
				tracing::debug!(%error, "user was not fetched");
				classify(&error)
			}
		},
	}
}

fn still_needed(state: &State, job: &Job) -> bool {
	match job.target {
		Target::Message(message_id) => {
			state
				.selected()
				.is_some_and(|selected| selected.channel_id == job.channel_id)
				&& state.referenced_message(message_id).is_none()
				&& !state
					.messages(job.channel_id)
					.is_some_and(|cache| cache.by_id.contains_key(&message_id))
		}
		Target::User(user_id) => state.user(user_id).is_none(),
	}
}

pub(crate) struct Backfill {
	gap: Duration,
	per_minute: usize,
	cool_down: Duration,
	queue: VecDeque<Job>,
	in_flight: Option<Job>,
	failed: IndexMap<Key, Instant>,
	retried: IndexSet<Target>,
	started: VecDeque<Instant>,
	next_at: Option<Instant>,
	stopped: bool,
	awaiting_members: IndexMap<(Snowflake, Snowflake), Snowflake>,
}

impl Backfill {
	pub(crate) fn new(tuning: &RestTuning) -> Self {
		Self {
			gap: tuning.backfill_gap,
			per_minute: tuning.backfill_per_minute.max(1) as usize,
			cool_down: tuning.backfill_cool_down,
			queue: VecDeque::new(),
			in_flight: None,
			failed: IndexMap::new(),
			retried: IndexSet::new(),
			started: VecDeque::new(),
			next_at: None,
			stopped: false,
			awaiting_members: IndexMap::new(),
		}
	}

	fn has_failed(&self, key: Key, now: Instant) -> bool {
		self.failed.get(&key).is_some_and(|until| *until > now)
	}

	fn remember_failure(&mut self, key: Key, now: Instant) {
		self.failed.retain(|_, until| *until > now);
		self.failed.insert(key, now + self.cool_down);
		while self.failed.len() > MAX_REMEMBERED {
			self.failed.shift_remove_index(0);
		}
	}

	fn want(&mut self, job: Job, now: Instant) {
		let known = self.in_flight.is_some_and(|flight| flight.target == job.target)
			|| self.queue.iter().any(|queued| queued.target == job.target);
		if self.stopped
			|| known || self.queue.len() >= MAX_QUEUED
			|| self.has_failed(Key::Target(job.target), now)
		{
			return;
		}
		self.queue.push_back(job);
	}

	fn keep_channel(&mut self, channel_id: Snowflake) {
		self.queue.retain(|job| job.channel_id == channel_id);
	}

	fn absent_members(&self, guild_id: Snowflake, ids: Vec<Snowflake>, now: Instant) -> Vec<Snowflake> {
		ids.into_iter()
			.filter(|user_id| !self.has_failed(Key::Member(guild_id, *user_id), now))
			.collect()
	}

	fn await_member(&mut self, guild_id: Snowflake, user_id: Snowflake, channel_id: Snowflake) {
		self.awaiting_members.insert((guild_id, user_id), channel_id);
		while self.awaiting_members.len() > MAX_REMEMBERED {
			self.awaiting_members.shift_remove_index(0);
		}
	}

	fn members_answered(&mut self, chunk: &GuildMembersChunk, now: Instant) -> Vec<(Snowflake, Snowflake)> {
		for member in chunk.members.iter() {
			if let Some(user) = &member.user {
				self.awaiting_members.shift_remove(&(chunk.guild_id, user.id));
			}
		}
		let mut missing = Vec::new();
		for user_id in &chunk.not_found {
			self.remember_failure(Key::Member(chunk.guild_id, *user_id), now);
			if let Some(channel_id) = self.awaiting_members.shift_remove(&(chunk.guild_id, *user_id)) {
				missing.push((channel_id, *user_id));
			}
		}
		missing
	}

	fn next_start(&self, now: Instant) -> Option<Instant> {
		if self.stopped || self.in_flight.is_some() || self.queue.is_empty() {
			return None;
		}
		let minute_slot = (self.started.len() >= self.per_minute)
			.then(|| self.started.front().map(|first| *first + MINUTE))
			.flatten();
		Some(
			[self.next_at, minute_slot]
				.into_iter()
				.flatten()
				.max()
				.unwrap_or(now),
		)
	}

	pub(crate) fn deadline(&self) -> Option<Instant> {
		self.next_start(Instant::now())
	}

	fn take(&mut self, now: Instant, needed: impl Fn(&Job) -> bool) -> Option<Job> {
		while self.started.front().is_some_and(|first| *first + MINUTE <= now) {
			self.started.pop_front();
		}
		if self.next_start(now).is_none_or(|start| start > now) {
			return None;
		}
		while let Some(job) = self.queue.pop_front() {
			if !needed(&job) {
				continue;
			}
			self.in_flight = Some(job);
			self.started.push_back(now);
			self.next_at = Some(now + self.gap);
			return Some(job);
		}
		None
	}

	fn pause(&mut self, until: Instant) {
		self.next_at = Some(self.next_at.map_or(until, |next| next.max(until)));
	}

	fn finished(&mut self, job: Job, outcome: &Outcome, now: Instant) {
		self.in_flight = None;
		match outcome {
			Outcome::Message(_) | Outcome::User(_) => {
				self.retried.shift_remove(&job.target);
			}
			Outcome::Gone => {
				self.retried.shift_remove(&job.target);
				self.remember_failure(Key::Target(job.target), now);
			}
			Outcome::Pause { wait, retry } => {
				self.pause(now + (*wait).max(self.gap));
				if *retry && self.retried.insert(job.target) {
					self.queue.push_front(job);
				} else {
					self.retried.shift_remove(&job.target);
					self.remember_failure(Key::Target(job.target), now);
				}
			}
			Outcome::Stop => {
				self.stopped = true;
				self.queue.clear();
			}
			Outcome::Later => {
				self.retried.shift_remove(&job.target);
				self.remember_failure(Key::Target(job.target), now);
				self.pause(now + FAILURE_PAUSE);
			}
		}
		while self.retried.len() > MAX_REMEMBERED {
			self.retried.shift_remove_index(0);
		}
	}
}

impl Core {
	pub(super) fn backfill_channel(&mut self, channel_id: Snowflake, only: Option<Snowflake>) {
		let (needs, guild_id) = {
			let state = self.state.read();
			if state
				.selected()
				.is_none_or(|selected| selected.channel_id != channel_id)
			{
				return;
			}
			(
				state.backfill_needs(channel_id, only),
				state.channel_guild(channel_id),
			)
		};
		if needs.is_empty() {
			return;
		}
		let now = Instant::now();
		for message_id in needs.references {
			self.backfill.want(
				Job {
					channel_id,
					target: Target::Message(message_id),
				},
				now,
			);
		}
		match guild_id {
			Some(guild_id) => {
				for user_id in &needs.users {
					self.backfill.await_member(guild_id, *user_id, channel_id);
				}
				let mut ids = needs.users;
				ids.extend(needs.members);
				let ids = self.backfill.absent_members(guild_id, ids, now);
				if !ids.is_empty() {
					self.request_members(guild_id, ids);
				}
			}
			None => {
				for user_id in needs.users {
					self.backfill.want(
						Job {
							channel_id,
							target: Target::User(user_id),
						},
						now,
					);
				}
			}
		}
		self.run_backfill(now);
	}

	pub(super) fn backfill_opened(&mut self, channel_id: Snowflake) {
		self.backfill.keep_channel(channel_id);
		self.backfill_channel(channel_id, None);
	}

	pub(super) fn backfill_members_answered(&mut self, chunk: &GuildMembersChunk) {
		let now = Instant::now();
		let missing = self.backfill.members_answered(chunk, now);
		if missing.is_empty() {
			return;
		}
		let unknown: Vec<(Snowflake, Snowflake)> = {
			let state = self.state.read();
			missing
				.into_iter()
				.filter(|(_, user_id)| state.user(*user_id).is_none())
				.collect()
		};
		for (channel_id, user_id) in unknown {
			self.backfill.want(
				Job {
					channel_id,
					target: Target::User(user_id),
				},
				now,
			);
		}
		self.run_backfill(now);
	}

	pub(super) fn run_backfill(&mut self, now: Instant) {
		let job = {
			let state = self.state.read();
			self.backfill.take(now, |job| still_needed(&state, job))
		};
		let Some(job) = job else {
			return;
		};
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let outcome = fetch(rest, job).await;
			let _ = internal.send(Internal::Backfilled { job, outcome });
		});
	}

	pub(super) fn on_backfilled(&mut self, job: Job, outcome: Outcome) {
		let now = Instant::now();
		self.backfill.finished(job, &outcome, now);
		let channel_id = job.channel_id;
		match (job.target, outcome) {
			(Target::Message(message_id), Outcome::Message(message)) => {
				self.apply_local(LocalEvent::ReferenceFetched {
					channel_id,
					message_id,
					message: Some(message),
				});
			}
			(Target::Message(message_id), Outcome::Gone) => {
				self.apply_local(LocalEvent::ReferenceFetched {
					channel_id,
					message_id,
					message: None,
				});
			}
			(Target::User(_), Outcome::User(user)) => {
				self.apply_local(LocalEvent::UserFetched { channel_id, user });
			}
			_ => {}
		}
		self.run_backfill(now);
	}
}
