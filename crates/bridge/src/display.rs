use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Weak};

use chrono::{DateTime, TimeDelta, Utc};
use markdown::TimestampStyle;
use parking_lot::Mutex;
use protocol::{Snowflake, User};
use session::{DisplayHooks, Formatting, Person, RelativeTime, ShownName};
use state::State;

use crate::message::timestamp_text;

const NAME_CACHE: usize = 4096;
const TIME_CACHE: usize = 1024;
const UNITS: [(&str, u64, u64); 6] = [
	("second", 1, 60),
	("minute", 60, 60),
	("hour", 3600, 24),
	("day", 86_400, 30),
	("month", 30 * 86_400, 12),
	("year", 365 * 86_400, u64::MAX),
];

type Notify = Box<dyn Fn(Formatting) + Send + Sync>;

enum Request {
	Name(Person, u64),
	Time(i64, RelativeTime, u64),
}

struct Memo<K, V> {
	active: Vec<String>,
	generation: u64,
	answered: HashMap<K, Option<V>>,
	stale: HashMap<K, Option<V>>,
	asked: HashSet<K>,
	limit: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Memo<K, V> {
	fn new(limit: usize) -> Self {
		Self {
			active: Vec::new(),
			generation: 0,
			answered: HashMap::new(),
			stale: HashMap::new(),
			asked: HashSet::new(),
			limit,
		}
	}

	fn look(&mut self, key: &K) -> (Option<V>, bool) {
		if let Some(answer) = self.answered.get(key) {
			return (answer.clone(), false);
		}
		let fallback = self.stale.get(key).cloned().flatten();
		(fallback, self.asked.insert(key.clone()))
	}

	fn answer(&mut self, key: K, value: Option<V>, generation: u64) -> bool {
		if generation != self.generation {
			return false;
		}
		self.asked.remove(&key);
		if self.answered.len() >= self.limit {
			self.answered.clear();
			self.stale.clear();
		}
		self.answered.insert(key, value);
		true
	}

	fn renew(&mut self, active: Vec<String>) -> bool {
		if active == self.active {
			return false;
		}
		let answered = std::mem::take(&mut self.answered);
		if active.is_empty() {
			self.stale.clear();
		} else {
			self.stale.extend(answered);
		}
		self.asked.clear();
		self.active = active;
		self.generation += 1;
		true
	}
}

struct Inner {
	hooks: Arc<dyn DisplayHooks>,
	names: Mutex<Memo<Person, ShownName>>,
	times: Mutex<Memo<i64, String>>,
	requests: Mutex<Option<Sender<Request>>>,
	notify: Notify,
	calls: AtomicU64,
}

#[derive(Clone, Default)]
pub struct Display {
	inner: Option<Arc<Inner>>,
}

impl std::fmt::Debug for Display {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Display")
			.field("hooked", &self.inner.is_some())
			.finish()
	}
}

impl Display {
	pub fn new(hooks: Arc<dyn DisplayHooks>, notify: impl Fn(Formatting) + Send + Sync + 'static) -> Self {
		let inner = Arc::new(Inner {
			hooks,
			names: Mutex::new(Memo::new(NAME_CACHE)),
			times: Mutex::new(Memo::new(TIME_CACHE)),
			requests: Mutex::new(None),
			notify: Box::new(notify),
			calls: AtomicU64::new(0),
		});
		let display = Self { inner: Some(inner) };
		display.refresh();
		display
	}

	pub fn refresher(&self) -> impl Fn() + Send + Sync + 'static {
		let weak = self.inner.as_ref().map(Arc::downgrade).unwrap_or_default();
		move || {
			if let Some(inner) = weak.upgrade() {
				Display { inner: Some(inner) }.refresh();
			}
		}
	}

	pub fn refresh(&self) {
		let Some(inner) = &self.inner else {
			return;
		};
		let names = inner.hooks.formatters(Formatting::Names);
		let times = inner.hooks.formatters(Formatting::RelativeTimes);
		let names = inner.names.lock().renew(names);
		let times = inner.times.lock().renew(times);
		if names {
			(inner.notify)(Formatting::Names);
		}
		if times {
			(inner.notify)(Formatting::RelativeTimes);
		}
	}

	pub fn calls(&self) -> u64 {
		self.inner
			.as_ref()
			.map_or(0, |inner| inner.calls.load(Ordering::Relaxed))
	}

	pub fn pending(&self) -> usize {
		self.inner.as_ref().map_or(0, |inner| {
			inner.names.lock().asked.len() + inner.times.lock().asked.len()
		})
	}

	pub fn formats(&self, formatting: Formatting) -> bool {
		self.inner.as_ref().is_some_and(|inner| match formatting {
			Formatting::Names => !inner.names.lock().active.is_empty(),
			Formatting::RelativeTimes => !inner.times.lock().active.is_empty(),
		})
	}

	pub fn name(&self, person: &Person) -> Option<ShownName> {
		let inner = self.inner.as_ref()?;
		let (shown, ask, generation) = {
			let mut names = inner.names.lock();
			if names.active.is_empty() {
				return None;
			}
			let (shown, ask) = names.look(person);
			(shown, ask, names.generation)
		};
		if ask {
			self.ask(Request::Name(person.clone(), generation));
		}
		shown
	}

	pub fn person_name(&self, state: &State, guild_id: Option<Snowflake>, user: &User) -> Option<ShownName> {
		self.name(&person(state, guild_id, user))
	}

	pub fn timestamp(&self, unix: i64, style: TimestampStyle, now: DateTime<Utc>) -> String {
		match style {
			TimestampStyle::Relative => self.relative(unix, now),
			style => timestamp_text(unix, style, now),
		}
	}

	pub fn relative(&self, unix: i64, now: DateTime<Utc>) -> String {
		let Some(time) = relative_time(unix, now) else {
			return unix.to_string();
		};
		let Some(inner) = &self.inner else {
			return time.text;
		};
		let key = if time.past {
			time.seconds as i64
		} else {
			-(time.seconds as i64)
		};
		let (text, ask, generation) = {
			let mut times = inner.times.lock();
			if times.active.is_empty() {
				return time.text;
			}
			let (text, ask) = times.look(&key);
			(text, ask, times.generation)
		};
		let shown = text.unwrap_or_else(|| time.text.clone());
		if ask {
			self.ask(Request::Time(key, time, generation));
		}
		shown
	}

	fn ask(&self, request: Request) {
		let Some(inner) = &self.inner else {
			return;
		};
		let mut requests = inner.requests.lock();
		let request = match requests.as_ref() {
			Some(sender) => match sender.send(request) {
				Ok(()) => return,
				Err(mpsc::SendError(request)) => request,
			},
			None => request,
		};
		let (sender, receiver) = mpsc::channel();
		let weak = Arc::downgrade(inner);
		let spawned = std::thread::Builder::new()
			.name("display-hooks".to_owned())
			.spawn(move || answer(&weak, &receiver));
		match spawned {
			Ok(_) => {
				let _ = sender.send(request);
				*requests = Some(sender);
			}
			Err(error) => tracing::warn!(%error, "the display hook thread did not start"),
		}
	}
}

fn answer(weak: &Weak<Inner>, receiver: &Receiver<Request>) {
	while let Ok(first) = receiver.recv() {
		let mut batch = vec![first];
		batch.extend(receiver.try_iter());
		let Some(inner) = weak.upgrade() else {
			return;
		};
		let (mut names, mut times) = (false, false);
		for request in batch {
			match request {
				Request::Name(person, generation) => {
					if inner.names.lock().generation != generation {
						continue;
					}
					inner.calls.fetch_add(1, Ordering::Relaxed);
					let shown = inner.hooks.format_name(&person);
					names |= inner.names.lock().answer(person, shown, generation);
				}
				Request::Time(key, time, generation) => {
					if inner.times.lock().generation != generation {
						continue;
					}
					inner.calls.fetch_add(1, Ordering::Relaxed);
					let text = inner.hooks.format_relative(&time);
					times |= inner.times.lock().answer(key, text, generation);
				}
			}
		}
		tracing::debug!(
			calls = inner.calls.load(Ordering::Relaxed),
			"display hooks answered"
		);
		if names {
			(inner.notify)(Formatting::Names);
		}
		if times {
			(inner.notify)(Formatting::RelativeTimes);
		}
	}
}

pub fn person(state: &State, guild_id: Option<Snowflake>, user: &User) -> Person {
	let nick = guild_id
		.and_then(|guild_id| state.guild(guild_id))
		.and_then(|guild| guild.members.get(&user.id))
		.and_then(|member| member.nick.clone());
	Person {
		user_id: user.id,
		guild_id,
		username: user.username.clone(),
		global_name: user.global_name.clone(),
		nick,
	}
}

pub fn relative_time(unix: i64, now: DateTime<Utc>) -> Option<RelativeTime> {
	let time = DateTime::from_timestamp(unix, 0)?;
	let delta = now - time;
	let past = delta >= TimeDelta::zero();
	let seconds = delta.num_seconds().unsigned_abs();
	let index = UNITS
		.iter()
		.rposition(|(_, length, _)| seconds >= *length)
		.unwrap_or(0);
	let (unit, length, _) = UNITS[index];
	let value = seconds as f64 / length as f64;
	let (shown_unit, amount) = nearest(seconds, index);
	Some(RelativeTime {
		seconds,
		past,
		unit,
		value,
		text: relative_text(shown_unit, amount, past),
	})
}

fn nearest(seconds: u64, index: usize) -> (&'static str, u64) {
	let (unit, length, ceiling) = UNITS[index];
	let amount = (seconds as f64 / length as f64).round() as u64;
	match UNITS.get(index + 1) {
		Some((next, _, _)) if amount >= ceiling => (next, 1),
		_ => (unit, amount),
	}
}

pub fn relative_text(unit: &str, amount: u64, past: bool) -> String {
	let span = if amount == 1 {
		format!("1 {unit}")
	} else {
		format!("{amount} {unit}s")
	};
	if past {
		format!("{span} ago")
	} else {
		format!("in {span}")
	}
}
