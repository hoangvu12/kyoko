use std::collections::HashMap;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::time::Instant;
use wreq::Method;

use crate::route::RouteKey;
use crate::tuning::RestTuning;

const PRUNE_THRESHOLD: usize = 256;

pub(crate) enum Pace {
	Free,
	Mutation,
	Delete,
	BulkAck,
	Send(String),
}

impl Pace {
	pub fn of(method: &Method, route: &RouteKey) -> Self {
		if route.is_message_send() {
			Self::Send(route.major().to_owned())
		} else if route.is_interaction() {
			Self::Send(route.as_str().to_owned())
		} else if route.is_message_delete() {
			Self::Delete
		} else if route.is_bulk_ack() {
			Self::BulkAck
		} else if method == Method::GET || method == Method::HEAD {
			Self::Free
		} else {
			Self::Mutation
		}
	}
}

#[derive(Default)]
struct Slots {
	mutation: Option<Instant>,
	send: Option<Instant>,
	delete: Option<Instant>,
	bulk_ack: Option<Instant>,
	channels: HashMap<String, Instant>,
}

pub(crate) struct Pacer {
	mutation_gap: Duration,
	send_gap: Duration,
	channel_send_gap: Duration,
	delete_gap: Duration,
	bulk_ack_gap: Duration,
	slots: Mutex<Slots>,
}

fn later(at: Instant, other: Option<Instant>) -> Instant {
	other.map_or(at, |other| at.max(other))
}

impl Pacer {
	pub fn new(tuning: &RestTuning) -> Self {
		Self {
			mutation_gap: tuning.mutation_gap,
			send_gap: tuning.send_gap,
			channel_send_gap: tuning.channel_send_gap,
			delete_gap: tuning.delete_gap,
			bulk_ack_gap: tuning.bulk_ack_gap,
			slots: Mutex::default(),
		}
	}

	pub fn reserve(&self, pace: &Pace) -> Option<Instant> {
		let now = Instant::now();
		let mut slots = self.slots.lock();
		let mut at = later(now, slots.mutation);
		match pace {
			Pace::Free => return None,
			Pace::Mutation => {}
			Pace::Delete => {
				at = later(at, slots.delete);
				slots.delete = Some(at + self.delete_gap);
			}
			Pace::BulkAck => {
				at = later(at, slots.bulk_ack);
				slots.bulk_ack = Some(at + self.bulk_ack_gap);
			}
			Pace::Send(channel) => {
				at = later(at, slots.send);
				at = later(at, slots.channels.get(channel).copied());
				slots.send = Some(at + self.send_gap);
				slots.channels.insert(channel.clone(), at + self.channel_send_gap);
				if slots.channels.len() > PRUNE_THRESHOLD {
					slots.channels.retain(|_, next| *next > now);
				}
			}
		}
		slots.mutation = Some(at + self.mutation_gap);
		Some(at)
	}
}
