use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use protocol::capability::has;
use rand::Rng;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

const REPLAY_CAPACITY: usize = 500;
const DETACHED_TTL: Duration = Duration::from_secs(300);

pub enum Outgoing {
	Frame(Arc<str>),
	Close(u16),
	Invalidate { resumable: bool },
}

#[derive(Clone)]
pub struct Link {
	pub id: u64,
	pub tx: UnboundedSender<Outgoing>,
}

struct SessionRecord {
	capabilities: u64,
	alt: bool,
	seq: u64,
	buffer: VecDeque<(u64, Arc<str>)>,
	link: Option<Link>,
	detached_at: Option<Instant>,
}

impl SessionRecord {
	fn push_event(&mut self, name: &str, data: &str) {
		self.seq += 1;
		let frame: Arc<str> = format!(r#"{{"op":0,"s":{},"t":"{name}","d":{data}}}"#, self.seq).into();
		self.buffer.push_back((self.seq, frame.clone()));
		if self.buffer.len() > REPLAY_CAPACITY {
			self.buffer.pop_front();
		}
		if let Some(link) = &self.link {
			let _ = link.tx.send(Outgoing::Frame(frame));
		}
	}
}

#[derive(Default)]
pub struct Hub {
	sessions: Mutex<HashMap<String, SessionRecord>>,
	next_link: AtomicU64,
}

impl Hub {
	pub fn next_link_id(&self) -> u64 {
		self.next_link.fetch_add(1, Ordering::Relaxed)
	}

	pub fn create_session(&self, link: Link, capabilities: u64, alt: bool) -> String {
		let session_id = format!("{:032x}", rand::rng().random::<u128>());
		let mut sessions = self.sessions.lock();
		sessions.retain(|_, record| record.detached_at.is_none_or(|at| at.elapsed() < DETACHED_TTL));
		sessions.insert(
			session_id.clone(),
			SessionRecord {
				capabilities,
				alt,
				seq: 0,
				buffer: VecDeque::new(),
				link: Some(link),
				detached_at: None,
			},
		);
		session_id
	}

	pub fn dispatch_to(&self, session_id: &str, name: &str, data: &Value) {
		let text = data.to_string();
		if let Some(record) = self.sessions.lock().get_mut(session_id) {
			record.push_event(name, &text);
		}
	}

	pub fn broadcast(&self, name: &str, data: &Value) {
		let text = data.to_string();
		let mut sessions = self.sessions.lock();
		sessions.retain(|_, record| record.detached_at.is_none_or(|at| at.elapsed() < DETACHED_TTL));
		for record in sessions.values_mut().filter(|record| !record.alt) {
			record.push_event(name, &text);
		}
	}

	pub fn broadcast_by_capability(&self, capability: u64, with: (&str, &Value), without: (&str, &Value)) {
		let with_text = with.1.to_string();
		let without_text = without.1.to_string();
		let mut sessions = self.sessions.lock();
		sessions.retain(|_, record| record.detached_at.is_none_or(|at| at.elapsed() < DETACHED_TTL));
		for record in sessions.values_mut().filter(|record| !record.alt) {
			if has(record.capabilities, capability) {
				record.push_event(with.0, &with_text);
			} else {
				record.push_event(without.0, &without_text);
			}
		}
	}

	pub fn resume(&self, session_id: &str, seq: u64, link: Link) -> bool {
		let mut sessions = self.sessions.lock();
		let Some(record) = sessions.get_mut(session_id) else {
			return false;
		};
		let oldest = record.buffer.front().map_or(record.seq + 1, |(first, _)| *first);
		if seq > record.seq || seq + 1 < oldest {
			return false;
		}
		if let Some(previous) = record.link.replace(link.clone()) {
			let _ = previous.tx.send(Outgoing::Close(4000));
		}
		record.detached_at = None;
		for (_, frame) in record.buffer.iter().filter(|(frame_seq, _)| *frame_seq > seq) {
			let _ = link.tx.send(Outgoing::Frame(frame.clone()));
		}
		record.push_event("RESUMED", "{}");
		true
	}

	pub fn detach(&self, session_id: &str, link_id: u64) {
		if let Some(record) = self.sessions.lock().get_mut(session_id)
			&& record.link.as_ref().is_some_and(|link| link.id == link_id)
		{
			record.link = None;
			record.detached_at = Some(Instant::now());
		}
	}

	pub fn send_all(&self, payload: &Value) {
		let frame: Arc<str> = payload.to_string().into();
		for record in self.sessions.lock().values() {
			if let Some(link) = &record.link {
				let _ = link.tx.send(Outgoing::Frame(frame.clone()));
			}
		}
	}

	pub fn close_all(&self, code: u16) {
		self.signal_all(|| Outgoing::Close(code));
	}

	pub fn invalidate_all(&self, resumable: bool) {
		self.signal_all(|| Outgoing::Invalidate { resumable });
	}

	pub fn end(&self, session_id: &str) {
		self.sessions.lock().remove(session_id);
	}

	fn signal_all(&self, signal: impl Fn() -> Outgoing) {
		for record in self.sessions.lock().values() {
			if let Some(link) = &record.link {
				let _ = link.tx.send(signal());
			}
		}
	}
}
