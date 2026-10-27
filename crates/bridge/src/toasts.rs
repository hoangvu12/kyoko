use std::time::{Duration, Instant};

use state::State;

pub const TOAST_LIFETIME: Duration = Duration::from_secs(5);
pub const TOAST_EXIT: Duration = Duration::from_millis(200);
pub const TOAST_LIMIT: usize = 3;
const TEXT_LIMIT: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToastKind {
	Info,
	Success,
	Warning,
	Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
	pub id: u64,
	pub kind: ToastKind,
	pub text: String,
	pub leaving: bool,
	expires_at: Instant,
	leaving_at: Option<Instant>,
}

#[derive(Debug)]
pub struct Toasts {
	items: Vec<Toast>,
	next_id: u64,
	paused_at: Option<Instant>,
	seen_rejection: u64,
	lifetime: Duration,
}

impl Default for Toasts {
	fn default() -> Self {
		Self::lasting(TOAST_LIFETIME)
	}
}

impl Toasts {
	pub fn lasting(lifetime: Duration) -> Self {
		Self {
			items: Vec::new(),
			next_id: 0,
			paused_at: None,
			seen_rejection: 0,
			lifetime,
		}
	}

	pub fn items(&self) -> &[Toast] {
		&self.items
	}

	pub fn push(&mut self, kind: ToastKind, text: &str, now: Instant) -> u64 {
		let text = clipped(text);
		let expires_at = now + self.lifetime;
		if let Some(toast) = self
			.items
			.iter_mut()
			.find(|toast| !toast.leaving && toast.kind == kind && toast.text == text)
		{
			toast.expires_at = expires_at;
			return toast.id;
		}
		self.next_id += 1;
		let id = self.next_id;
		self.items.insert(
			0,
			Toast {
				id,
				kind,
				text,
				leaving: false,
				expires_at,
				leaving_at: None,
			},
		);
		let staying: Vec<u64> = self
			.items
			.iter()
			.filter(|toast| !toast.leaving)
			.map(|toast| toast.id)
			.skip(TOAST_LIMIT)
			.collect();
		for id in staying {
			self.leave(id, now);
		}
		id
	}

	pub fn dismiss(&mut self, id: u64, now: Instant) -> bool {
		self.leave(id, now)
	}

	pub fn pause(&mut self, now: Instant) {
		self.paused_at.get_or_insert(now);
	}

	pub fn resume(&mut self, now: Instant) {
		let Some(paused_at) = self.paused_at.take() else {
			return;
		};
		let held = now.saturating_duration_since(paused_at);
		for toast in self.items.iter_mut().filter(|toast| !toast.leaving) {
			toast.expires_at += held;
		}
	}

	pub fn tick(&mut self, now: Instant) -> bool {
		let before = self.items.len();
		self.items
			.retain(|toast| toast.leaving_at.is_none_or(|at| now < at + TOAST_EXIT));
		let mut changed = self.items.len() != before;
		if self.paused_at.is_none() {
			let due: Vec<u64> = self
				.items
				.iter()
				.filter(|toast| !toast.leaving && toast.expires_at <= now)
				.map(|toast| toast.id)
				.collect();
			for id in due {
				changed |= self.leave(id, now);
			}
		}
		changed
	}

	pub fn next_deadline(&self) -> Option<Instant> {
		self.items
			.iter()
			.filter_map(|toast| match toast.leaving_at {
				Some(at) => Some(at + TOAST_EXIT),
				None if self.paused_at.is_none() => Some(toast.expires_at),
				None => None,
			})
			.min()
	}

	pub fn new_rejection(&mut self, state: &State) -> Option<String> {
		let rejection = state.last_rejection()?;
		if rejection.id <= self.seen_rejection {
			return None;
		}
		self.seen_rejection = rejection.id;
		Some(rejection.reason.clone())
	}

	pub fn forget_rejections(&mut self) {
		self.seen_rejection = 0;
	}

	fn leave(&mut self, id: u64, now: Instant) -> bool {
		match self
			.items
			.iter_mut()
			.find(|toast| toast.id == id && !toast.leaving)
		{
			Some(toast) => {
				toast.leaving = true;
				toast.leaving_at = Some(now);
				true
			}
			None => false,
		}
	}
}

fn clipped(text: &str) -> String {
	let text = text.trim();
	match text.char_indices().nth(TEXT_LIMIT) {
		Some((end, _)) => format!("{}…", &text[..end]),
		None => text.to_owned(),
	}
}
