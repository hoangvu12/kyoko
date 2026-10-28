use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use dave::Wire;
use dave::testing::Harness;

pub(super) type Mail = Vec<(u64, u64, Wire)>;

#[derive(Default)]
pub(super) struct Rooms {
	rooms: HashMap<u64, Harness>,
	groups: HashMap<u64, u64>,
	externals: HashSet<(u64, u64)>,
}

impl Rooms {
	pub fn set_group(&mut self, channel_id: u64, group: u64) {
		self.groups.insert(channel_id, group);
	}

	pub fn join_external(&mut self, channel_id: u64, user_id: u64) -> Mail {
		let elsewhere: Vec<u64> = self
			.externals
			.iter()
			.filter(|(channel, user)| {
				*user == user_id && *channel != channel_id && !self.groups.contains_key(channel)
			})
			.map(|(channel, _)| *channel)
			.collect();
		let joins_voice = !self.groups.contains_key(&channel_id);
		if joins_voice {
			for previous in elsewhere {
				self.run(previous, |room| room.leave(user_id));
				self.externals.remove(&(previous, user_id));
			}
		}
		if self.externals.contains(&(channel_id, user_id)) {
			self.run(channel_id, |room| room.leave(user_id));
		}
		self.externals.insert((channel_id, user_id));
		self.run(channel_id, |room| room.join_external(user_id))
	}

	pub fn deliver(&mut self, channel_id: u64, user_id: u64, message: Wire) -> Mail {
		if !self.externals.contains(&(channel_id, user_id)) {
			return Mail::new();
		}
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
			.filter(|(channel, _)| *channel == channel_id)
			.map(|(_, user)| *user)
			.collect();
		let mut mail = Mail::new();
		if let Some(room) = self.rooms.get_mut(&channel_id) {
			for user in externals {
				mail.extend(
					room.take_outbox(user)
						.into_iter()
						.map(|wire| (channel_id, user, wire)),
				);
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

	pub fn open_video(&self, channel_id: u64, sender: u64, frame: &[u8]) -> Option<Vec<u8>> {
		self.rooms
			.get(&channel_id)?
			.members()
			.find_map(|member| member.decrypt_as(dave::Codec::H264, sender, frame))
	}

	pub fn seal(&self, channel_id: u64, user_id: u64, frame: &[u8]) -> Option<Vec<u8>> {
		match self.rooms.get(&channel_id).and_then(|room| room.member(user_id)) {
			Some(member) => member.encrypt(frame),
			None => Some(frame.to_vec()),
		}
	}

	pub fn seal_video(&self, channel_id: u64, user_id: u64, frame: &[u8]) -> Option<Vec<u8>> {
		match self.rooms.get(&channel_id).and_then(|room| room.member(user_id)) {
			Some(member) => member.encrypt_as(dave::Codec::H264, frame),
			None => Some(frame.to_vec()),
		}
	}

	fn room(&mut self, channel_id: u64) -> Option<&mut Harness> {
		let group = self.groups.get(&channel_id).copied().unwrap_or(channel_id);
		match self.rooms.entry(channel_id) {
			Entry::Occupied(room) => Some(room.into_mut()),
			Entry::Vacant(slot) => match Harness::new(group) {
				Ok(room) => Some(slot.insert(room)),
				Err(error) => {
					tracing::warn!(%error, "mock dave room could not start");
					None
				}
			},
		}
	}
}
