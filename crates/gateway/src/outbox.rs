use std::collections::VecDeque;
use std::future::pending;
use std::time::Duration;

use protocol::gateway::Opcode;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};

const URGENT_RESERVE_DIVISOR: usize = 12;
const BULK_RESERVE_DIVISOR: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendBudget {
	pub frames: u32,
	pub period: Duration,
}

impl Default for SendBudget {
	fn default() -> Self {
		Self {
			frames: 120,
			period: Duration::from_secs(60),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lane {
	Control,
	Urgent,
	Bulk,
}

impl Lane {
	pub fn of(opcode: Opcode) -> Self {
		match opcode {
			Opcode::Heartbeat | Opcode::Identify | Opcode::Resume => Self::Control,
			Opcode::GuildSubscriptions | Opcode::GuildSubscriptionsBulk | Opcode::RequestGuildMembers => {
				Self::Bulk
			}
			_ => Self::Urgent,
		}
	}
}

pub(crate) struct SendWindow {
	budget: SendBudget,
	sent: VecDeque<Instant>,
}

impl SendWindow {
	pub fn new(budget: SendBudget) -> Self {
		Self {
			budget,
			sent: VecDeque::new(),
		}
	}

	fn limit(&self, lane: Lane) -> usize {
		let frames = self.budget.frames.max(1) as usize;
		let reserve = match lane {
			Lane::Control => 0,
			Lane::Urgent => frames / URGENT_RESERVE_DIVISOR,
			Lane::Bulk => frames / BULK_RESERVE_DIVISOR,
		};
		(frames - reserve).max(1)
	}

	pub fn opens_at(&mut self, lane: Lane, now: Instant) -> Option<Instant> {
		while self
			.sent
			.front()
			.is_some_and(|sent| *sent + self.budget.period <= now)
		{
			self.sent.pop_front();
		}
		let limit = self.limit(lane);
		(self.sent.len() >= limit).then(|| self.sent[self.sent.len() - limit] + self.budget.period)
	}

	pub fn record(&mut self, now: Instant) {
		self.sent.push_back(now);
	}
}

pub(crate) struct Outbox {
	control: VecDeque<String>,
	urgent: mpsc::Receiver<String>,
	bulk: mpsc::Receiver<String>,
	window: SendWindow,
}

impl Outbox {
	pub fn new(budget: SendBudget, urgent: mpsc::Receiver<String>, bulk: mpsc::Receiver<String>) -> Self {
		Self {
			control: VecDeque::new(),
			urgent,
			bulk,
			window: SendWindow::new(budget),
		}
	}

	pub fn push_control(&mut self, frame: String) {
		self.control.push_back(frame);
	}

	pub fn clear_control(&mut self) {
		self.control.clear();
	}

	pub async fn next(&mut self, open: bool) -> Option<String> {
		loop {
			let now = Instant::now();
			if !self.control.is_empty() {
				match self.window.opens_at(Lane::Control, now) {
					None => {
						self.window.record(now);
						return self.control.pop_front();
					}
					Some(at) => {
						sleep_until(at).await;
						continue;
					}
				}
			}
			if !open {
				return pending().await;
			}
			let urgent_at = self.window.opens_at(Lane::Urgent, now);
			let bulk_at = self.window.opens_at(Lane::Bulk, now);
			let reopens = urgent_at.or(bulk_at);
			let frame = tokio::select! {
				biased;
				frame = self.urgent.recv(), if urgent_at.is_none() => frame,
				frame = self.bulk.recv(), if bulk_at.is_none() => frame,
				() = sleep_until(reopens.unwrap_or(now)), if reopens.is_some() => continue,
			};
			if frame.is_some() {
				self.window.record(Instant::now());
			}
			return frame;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const HEARTBEAT: Duration = Duration::from_millis(41_250);
	const BURST: usize = 400;
	const URGENT: usize = 30;

	fn heartbeat_frame() -> String {
		"heartbeat".to_owned()
	}

	#[tokio::test(start_paused = true)]
	async fn a_bulk_burst_never_exceeds_the_window_or_delays_a_heartbeat() {
		let budget = SendBudget::default();
		let (urgent_tx, urgent_rx) = mpsc::channel(BURST);
		let (bulk_tx, bulk_rx) = mpsc::channel(BURST);
		let mut outbox = Outbox::new(budget, urgent_rx, bulk_rx);
		for index in 0..BURST {
			bulk_tx.try_send(format!("bulk {index}")).expect("bulk queued");
		}
		for index in 0..URGENT {
			urgent_tx
				.try_send(format!("urgent {index}"))
				.expect("urgent queued");
		}
		let start = Instant::now();
		let mut beats = tokio::time::interval_at(start + HEARTBEAT / 2, HEARTBEAT);
		let mut sent: Vec<(Instant, String)> = Vec::new();
		let mut queued_beats: Vec<Instant> = Vec::new();
		let mut beat_sends: Vec<Instant> = Vec::new();
		while sent.iter().filter(|(_, frame)| frame.starts_with("bulk")).count() < BURST {
			tokio::select! {
				biased;
				at = beats.tick() => {
					queued_beats.push(at);
					outbox.push_control(heartbeat_frame());
				}
				frame = outbox.next(true) => {
					let frame = frame.expect("the queues stay open");
					let now = Instant::now();
					if frame == heartbeat_frame() {
						beat_sends.push(now);
					}
					sent.push((now, frame));
				}
			}
		}
		for (index, (at, _)) in sent.iter().enumerate() {
			let in_window = sent[..=index]
				.iter()
				.filter(|(earlier, _)| *at - *earlier < budget.period)
				.count();
			assert!(
				in_window <= budget.frames as usize,
				"frame {index} broke the window"
			);
		}
		assert!(queued_beats.len() >= 3);
		assert_eq!(beat_sends.len(), queued_beats.len());
		assert_eq!(beat_sends, queued_beats);
		let first_bulk = sent
			.iter()
			.position(|(_, frame)| frame.starts_with("bulk"))
			.expect("bulk was sent");
		let urgent_before_bulk = sent[..first_bulk]
			.iter()
			.filter(|(_, frame)| frame.starts_with("urgent"))
			.count();
		assert_eq!(urgent_before_bulk, URGENT);
		let elapsed = sent.last().expect("frames were sent").0 - start;
		let windows_needed = (BURST + URGENT).div_ceil(budget.frames as usize) - 1;
		assert!(elapsed >= budget.period * windows_needed as u32);
	}
}
