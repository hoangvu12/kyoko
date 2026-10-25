use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::header::HeaderMap;
use tokio::sync::Notify;

use crate::route::RouteKey;

const MAX_HEADER_WAIT: f64 = 600.0;
const PRUNE_THRESHOLD: usize = 1024;
const RESET_MARGIN: Duration = Duration::from_millis(10);
const RETRY_MARGIN: Duration = Duration::from_millis(50);

#[derive(Default)]
pub struct RateLimiter {
	state: Mutex<State>,
	released: Notify,
}

#[derive(Default)]
struct State {
	route_buckets: HashMap<String, String>,
	buckets: HashMap<String, Bucket>,
	global_until: Option<Instant>,
	route_until: HashMap<String, Instant>,
	probing: HashSet<String>,
}

struct Bucket {
	limit: u32,
	remaining: u32,
	window: Duration,
	reset_at: Instant,
}

impl Bucket {
	fn merge(&mut self, observed: Bucket) {
		let new_window = observed.reset_at > self.reset_at + self.window / 2;
		let window = self.window.max(observed.window);
		if self.reset_at <= Instant::now() || new_window {
			*self = observed;
			self.window = window;
			return;
		}
		self.window = window;
		self.limit = observed.limit;
		self.remaining = self.remaining.min(observed.remaining);
		self.reset_at = self.reset_at.max(observed.reset_at);
	}
}

enum Reservation {
	Go { probe: bool },
	Wait(Duration),
	Probing,
}

pub struct Probe<'a> {
	limiter: &'a RateLimiter,
	route: Option<String>,
}

impl Drop for Probe<'_> {
	fn drop(&mut self) {
		if let Some(route) = self.route.take() {
			self.limiter.state.lock().probing.remove(&route);
			self.limiter.released.notify_waiters();
		}
	}
}

fn bucket_id(hash: &str, route: &RouteKey) -> String {
	format!("{hash}:{}", route.major())
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
	headers.get(name)?.to_str().ok()
}

fn header_secs(headers: &HeaderMap, name: &str) -> Option<Duration> {
	let seconds = header_text(headers, name)?.parse::<f64>().ok()?;
	Duration::try_from_secs_f64(seconds.clamp(0.0, MAX_HEADER_WAIT)).ok()
}

pub(crate) fn header_u32(headers: &HeaderMap, name: &str) -> Option<u32> {
	header_text(headers, name)?.parse().ok()
}

pub(crate) fn header_retry_after(headers: &HeaderMap) -> Option<Duration> {
	header_secs(headers, "retry-after")
}

pub(crate) fn header_global(headers: &HeaderMap) -> bool {
	header_text(headers, "x-ratelimit-global").is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn probe(state: &mut State, route: &RouteKey) -> Reservation {
	if state.probing.insert(route.as_str().to_owned()) {
		Reservation::Go { probe: true }
	} else {
		Reservation::Probing
	}
}

impl RateLimiter {
	pub async fn acquire(&self, route: &RouteKey) -> Probe<'_> {
		loop {
			let released = self.released.notified();
			tokio::pin!(released);
			released.as_mut().enable();
			match self.reserve(route) {
				Reservation::Go { probe } => {
					return Probe {
						limiter: self,
						route: probe.then(|| route.as_str().to_owned()),
					};
				}
				Reservation::Wait(wait) => tokio::time::sleep(wait).await,
				Reservation::Probing => released.await,
			}
		}
	}

	pub fn globally_paused(&self) -> bool {
		self.state
			.lock()
			.global_until
			.is_some_and(|until| until > Instant::now())
	}

	fn reserve(&self, route: &RouteKey) -> Reservation {
		let now = Instant::now();
		let mut guard = self.state.lock();
		let state = &mut *guard;
		if let Some(until) = state.global_until {
			if until > now {
				return Reservation::Wait(until - now);
			}
			state.global_until = None;
		}
		if let Some(until) = state.route_until.get(route.as_str()).copied() {
			if until > now {
				return Reservation::Wait(until - now);
			}
			state.route_until.remove(route.as_str());
		}
		let Some(hash) = state.route_buckets.get(route.as_str()) else {
			return probe(state, route);
		};
		let id = bucket_id(hash, route);
		let Some(bucket) = state.buckets.get_mut(&id) else {
			return probe(state, route);
		};
		if bucket.reset_at <= now {
			bucket.remaining = bucket.limit;
			bucket.reset_at = now + bucket.window;
		}
		if bucket.remaining == 0 {
			return Reservation::Wait(bucket.reset_at - now);
		}
		bucket.remaining -= 1;
		Reservation::Go { probe: false }
	}

	pub fn observe(&self, route: &RouteKey, headers: &HeaderMap) {
		let Some(hash) = header_text(headers, "x-ratelimit-bucket") else {
			return;
		};
		let (Some(remaining), Some(reset_after)) = (
			header_u32(headers, "x-ratelimit-remaining"),
			header_secs(headers, "x-ratelimit-reset-after"),
		) else {
			return;
		};
		let now = Instant::now();
		let mut state = self.state.lock();
		let id = bucket_id(hash, route);
		if state.route_buckets.get(route.as_str()).map(String::as_str) != Some(hash) {
			state
				.route_buckets
				.insert(route.as_str().to_owned(), hash.to_owned());
		}
		let limit = header_u32(headers, "x-ratelimit-limit")
			.or_else(|| state.buckets.get(&id).map(|bucket| bucket.limit))
			.unwrap_or(remaining)
			.max(remaining);
		let observed = Bucket {
			limit,
			remaining,
			window: reset_after,
			reset_at: now + reset_after + RESET_MARGIN,
		};
		match state.buckets.get_mut(&id) {
			Some(bucket) => bucket.merge(observed),
			None => {
				state.buckets.insert(id, observed);
			}
		}
		if state.buckets.len() > PRUNE_THRESHOLD {
			state.buckets.retain(|_, bucket| bucket.reset_at > now);
			state.route_until.retain(|_, until| *until > now);
		}
	}

	pub fn limited(&self, route: &RouteKey, retry_after: Duration, global: bool) {
		let reset_at = Instant::now() + retry_after + RETRY_MARGIN;
		let mut guard = self.state.lock();
		let state = &mut *guard;
		if global {
			state.global_until = Some(state.global_until.map_or(reset_at, |until| until.max(reset_at)));
			return;
		}
		state.route_until.insert(route.as_str().to_owned(), reset_at);
		let Some(hash) = state.route_buckets.get(route.as_str()) else {
			return;
		};
		let id = bucket_id(hash, route);
		if let Some(bucket) = state.buckets.get_mut(&id) {
			bucket.remaining = 0;
			bucket.reset_at = bucket.reset_at.max(reset_at);
		}
	}
}
