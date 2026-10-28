use std::collections::HashMap;
use std::collections::hash_map::Entry;

use dave::Wire;
use dave::testing::Harness;

pub(super) type Mail = Vec<(u64, Wire)>;

#[derive(Default)]
pub(super) struct Rooms {
	rooms: HashMap<u64, Harness>,
	externals: HashMap<u64, u64>,
}

impl Rooms {
	pub fn join_external(&mut self, channel_id: u64, user_id: u64) -> Mail {
		if self.externals.contains_key(&user_id) {
			let previous = self.externals[&user_id];
			self.run(previous, |room| room.leave(user_id));
			self.externals.remove(&user_id);
		}
		self.externals.insert(user_id, channel_id);
		self.run(channel_id, |room| room.join_external(user_id))
	}

	pub fn deliver(&mut self, user_id: u64, message: Wire) -> Mail {
		let Some(&channel_id) = self.externals.get(&user_id) else {
			return Mail::new();
		};
		self.run(channel_id, |room| room.deliver(user_id, message))
	}

	pub fn run(&mut self, channel_id: u64, act: impl FnOnce(&mut Harness) -> dave::Result<()>) -> Mail {
		let Some(room) = self.room(channel_id) else {
			return Mail::new();
		};
		if let Err(error) = act(room) {
			tracing::warn!(%error, "mock dave room rejected a message");
		}
		let externals: Vec<u64> = self
			.externals
			.iter()
			.filter(|(_, channel)| **channel == channel_id)
			.map(|(user, _)| *user)
			.collect();
		let mut mail = Mail::new();
		if let Some(room) = self.rooms.get_mut(&channel_id) {
			for user in externals {
				mail.extend(room.take_outbox(user).into_iter().map(|wire| (user, wire)));
			}
		}
		mail
	}

	pub fn get(&self, channel_id: u64) -> Option<&Harness> {
		self.rooms.get(&channel_id)
	}

	pub fn open(&self, channel_id: u64, sender: u64, payload: &[u8]) -> Option<Vec<u8>> {
		self.rooms
			.get(&channel_id)?
			.members()
			.find_map(|member| member.decrypt(sender, payload))
	}

	pub fn seal(&self, channel_id: u64, user_id: u64, frame: &[u8]) -> Option<Vec<u8>> {
		match self.rooms.get(&channel_id).and_then(|room| room.member(user_id)) {
			Some(member) => member.encrypt(frame),
			None => Some(frame.to_vec()),
		}
	}

	fn room(&mut self, channel_id: u64) -> Option<&mut Harness> {
		match self.rooms.entry(channel_id) {
			Entry::Occupied(room) => Some(room.into_mut()),
			Entry::Vacant(slot) => match Harness::new(channel_id) {
				Ok(room) => Some(slot.insert(room)),
				Err(error) => {
					tracing::warn!(%error, "mock dave room could not start");
					None
				}
			},
		}
	}
}
