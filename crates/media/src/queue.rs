use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::oneshot;
use wreq::Version;

use crate::error::{MediaError, Result};

pub const HTTP1_PER_HOST: usize = 6;
pub const HTTP2_PER_HOST: usize = 100;
const UNKNOWN_PER_HOST: usize = 1;

static NEXT_ORDER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Rank {
	Urgent = 0,
	Visible = 1,
	Asked = 2,
}

impl Rank {
	fn of(value: u8) -> Self {
		match value {
			0 => Self::Urgent,
			1 => Self::Visible,
			_ => Self::Asked,
		}
	}
}

#[derive(Debug)]
pub struct Ticket {
	order: u64,
	rank: AtomicU8,
	dropped: AtomicBool,
	started: AtomicBool,
}

impl Ticket {
	pub fn new(rank: Rank) -> Arc<Self> {
		Arc::new(Self {
			order: NEXT_ORDER.fetch_add(1, Ordering::Relaxed),
			rank: AtomicU8::new(rank as u8),
			dropped: AtomicBool::new(false),
			started: AtomicBool::new(false),
		})
	}

	pub fn urgent() -> Arc<Self> {
		Self::new(Rank::Urgent)
	}

	pub fn rank(&self) -> Rank {
		Rank::of(self.rank.load(Ordering::Relaxed))
	}

	pub fn show(&self) {
		self.rank.fetch_min(Rank::Visible as u8, Ordering::Relaxed);
	}

	pub fn drop_unless_started(&self) {
		if !self.started() {
			self.dropped.store(true, Ordering::Relaxed);
		}
	}

	pub fn started(&self) -> bool {
		self.started.load(Ordering::Relaxed)
	}

	fn dropped(&self) -> bool {
		self.dropped.load(Ordering::Relaxed) && !self.started()
	}

	fn place(&self) -> (Rank, u64) {
		(self.rank(), self.order)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
	Unknown,
	Http1,
	Http2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerHost {
	pub http1: usize,
	pub http2: usize,
}

impl Default for PerHost {
	fn default() -> Self {
		Self {
			http1: HTTP1_PER_HOST,
			http2: HTTP2_PER_HOST,
		}
	}
}

struct Waiter {
	ticket: Arc<Ticket>,
	grant: oneshot::Sender<Result<Slot>>,
}

struct Host {
	protocol: Protocol,
	active: usize,
	idle_since: Instant,
	waiting: Vec<Waiter>,
}

impl Host {
	fn new() -> Self {
		Self {
			protocol: Protocol::Unknown,
			active: 0,
			idle_since: Instant::now(),
			waiting: Vec::new(),
		}
	}
}

pub(crate) struct Gate {
	hosts: Mutex<HashMap<String, Host>>,
	per_host: PerHost,
	cold_after: Duration,
}

impl Gate {
	pub(crate) fn new(per_host: PerHost, cold_after: Duration) -> Arc<Self> {
		Arc::new(Self {
			hosts: Mutex::new(HashMap::new()),
			per_host,
			cold_after,
		})
	}

	pub(crate) async fn acquire(self: &Arc<Self>, host: &str, ticket: Arc<Ticket>) -> Result<Slot> {
		let (grant, granted) = oneshot::channel();
		self.hosts
			.lock()
			.entry(host.to_owned())
			.or_insert_with(Host::new)
			.waiting
			.push(Waiter { ticket, grant });
		self.dispatch(host);
		granted
			.await
			.unwrap_or_else(|_| Err(MediaError::Fetch("the fetcher closed".to_owned())))
	}

	pub(crate) fn learn(self: &Arc<Self>, host: &str, version: Version) {
		let protocol = if version == Version::HTTP_2 {
			Protocol::Http2
		} else {
			Protocol::Http1
		};
		if let Some(state) = self.hosts.lock().get_mut(host) {
			state.protocol = protocol;
		}
		self.dispatch(host);
	}

	pub(crate) fn protocol(&self, host: &str) -> Protocol {
		self.hosts
			.lock()
			.get(host)
			.map_or(Protocol::Unknown, |state| state.protocol)
	}

	pub(crate) fn waiting(&self) -> usize {
		self.hosts.lock().values().map(|state| state.waiting.len()).sum()
	}

	pub(crate) fn reschedule(self: &Arc<Self>) {
		let hosts: Vec<String> = self.hosts.lock().keys().cloned().collect();
		for host in hosts {
			self.dispatch(&host);
		}
	}

	fn limit(&self, protocol: Protocol) -> usize {
		match protocol {
			Protocol::Unknown => UNKNOWN_PER_HOST,
			Protocol::Http1 => self.per_host.http1,
			Protocol::Http2 => self.per_host.http2,
		}
	}

	fn release(self: &Arc<Self>, host: &str) {
		if let Some(state) = self.hosts.lock().get_mut(host) {
			state.active = state.active.saturating_sub(1);
			if state.active == 0 {
				state.idle_since = Instant::now();
			}
		}
		self.dispatch(host);
	}

	fn dispatch(self: &Arc<Self>, host: &str) {
		let mut hosts = self.hosts.lock();
		let Some(state) = hosts.get_mut(host) else {
			return;
		};
		if state.active == 0 && state.idle_since.elapsed() >= self.cold_after {
			state.protocol = Protocol::Unknown;
		}
		let (dropped, waiting): (Vec<Waiter>, Vec<Waiter>) = std::mem::take(&mut state.waiting)
			.into_iter()
			.filter(|waiter| !waiter.grant.is_closed())
			.partition(|waiter| waiter.ticket.dropped());
		state.waiting = waiting;
		for waiter in dropped {
			let _ = waiter.grant.send(Err(MediaError::Dropped));
		}
		while state.active < self.limit(state.protocol) {
			let Some(best) = state
				.waiting
				.iter()
				.enumerate()
				.min_by_key(|(_, waiter)| waiter.ticket.place())
				.map(|(index, _)| index)
			else {
				break;
			};
			let waiter = state.waiting.swap_remove(best);
			waiter.ticket.started.store(true, Ordering::Relaxed);
			state.active += 1;
			let slot = Slot {
				gate: Some(self.clone()),
				host: host.to_owned(),
			};
			if let Err(Ok(mut refused)) = waiter.grant.send(Ok(slot)) {
				refused.gate = None;
				state.active -= 1;
			}
		}
	}
}

pub(crate) struct Slot {
	gate: Option<Arc<Gate>>,
	host: String,
}

impl Drop for Slot {
	fn drop(&mut self) {
		if let Some(gate) = self.gate.take() {
			gate.release(&self.host);
		}
	}
}
