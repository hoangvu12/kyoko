use std::collections::HashSet;

use protocol::Snowflake;

use crate::caps::low_water;
use crate::changes::Changes;
use crate::member_list::MemberSlot;
use crate::messages::Viewport;
use crate::state::State;

impl State {
	pub fn warm_channels(&self) -> &[Snowflake] {
		&self.warm
	}

	pub fn is_warm(&self, channel_id: Snowflake) -> bool {
		self.warm.contains(&channel_id)
	}

	pub(crate) fn is_open(&self, channel_id: Snowflake) -> bool {
		self.selected
			.is_some_and(|selected| selected.channel_id == channel_id)
	}

	pub(crate) fn warm_up(&mut self, channel_id: Snowflake, changes: &mut Changes) {
		self.warm.retain(|id| *id != channel_id);
		self.warm.push(channel_id);
		self.cool_down(changes);
	}

	pub(crate) fn leave_channel(&mut self, channel_id: Snowflake, changes: &mut Changes) {
		let limits = self.limits;
		if let Some(cache) = self.messages.get_mut(&channel_id)
			&& cache.leave(&limits)
		{
			changes.messages.insert(channel_id);
		}
	}

	pub(crate) fn viewport_changed(
		&mut self,
		channel_id: Snowflake,
		viewport: Viewport,
		changes: &mut Changes,
	) {
		let limits = self.limits;
		if let Some(cache) = self.messages.get_mut(&channel_id)
			&& cache.set_viewport(viewport, &limits)
		{
			changes.messages.insert(channel_id);
		}
	}

	pub(crate) fn enforce_caps(&mut self, changes: &mut Changes) {
		let members: usize = self.guilds.values().map(|guild| guild.members.len()).sum();
		let over_members = members > self.limits.members;
		let over_users = self.users.len() > self.limits.users;
		if over_members || over_users {
			let in_view = self.people_in_view();
			if over_members {
				self.evict_members(members, &in_view, changes);
			}
			if over_users {
				self.evict_users(&in_view, changes);
			}
		}
		let me = self.current_user_id();
		self.evict_references();
		for user_id in self
			.lazy
			.profiles
			.evict_to(self.limits.profiles, |id| Some(*id) == me)
		{
			changes.loaded.insert(user_id);
		}
	}

	fn cool_down(&mut self, changes: &mut Changes) {
		let mut index = 0;
		while self.warm.len() > self.limits.warm() && index < self.warm.len() {
			let channel_id = self.warm[index];
			if self.must_stay_warm(channel_id) {
				index += 1;
				continue;
			}
			self.warm.remove(index);
			self.forget_references(channel_id);
			if self.messages.remove(&channel_id).is_some() {
				changes.messages.insert(channel_id);
			}
		}
	}

	fn must_stay_warm(&self, channel_id: Snowflake) -> bool {
		self.is_open(channel_id)
			|| self
				.messages
				.get(&channel_id)
				.is_some_and(|cache| !cache.pending.is_empty())
	}

	fn people_in_view(&self) -> HashSet<(Option<Snowflake>, Snowflake)> {
		let mut people = HashSet::new();
		for (&channel_id, cache) in &self.messages {
			let guild_id = self.channel_guild(channel_id);
			for message in &cache.ordered {
				people.insert((guild_id, message.author.id));
				people.extend(message.mentions.iter().map(|user| (guild_id, user.id)));
			}
			people.extend(
				cache
					.pending
					.iter()
					.map(|pending| (guild_id, pending.message.author.id)),
			);
		}
		for (&channel_id, typing) in &self.typing {
			let guild_id = self.channel_guild(channel_id);
			people.extend(typing.keys().map(|user_id| (guild_id, *user_id)));
		}
		for guild in self.guilds.values() {
			people.extend(
				guild
					.voice_states
					.keys()
					.map(|user_id| (Some(guild.id), *user_id)),
			);
		}
		if let Some(guild) = self
			.selected
			.and_then(|selected| selected.guild_id)
			.and_then(|guild_id| self.guilds.get(&guild_id))
		{
			people.extend(guild.member_list.slots.iter().filter_map(|slot| match slot {
				MemberSlot::Member(user_id) => Some((Some(guild.id), *user_id)),
				_ => None,
			}));
		}
		people
	}

	fn evict_members(
		&mut self,
		total: usize,
		in_view: &HashSet<(Option<Snowflake>, Snowflake)>,
		changes: &mut Changes,
	) {
		let me = self.current_user_id();
		let mut candidates: Vec<(u64, Snowflake, Snowflake)> = self
			.guilds
			.values()
			.flat_map(|guild| {
				guild
					.members
					.stamps()
					.filter(move |(user_id, _)| {
						Some(*user_id) != me && !in_view.contains(&(Some(guild.id), *user_id))
					})
					.map(move |(user_id, stamp)| (stamp, guild.id, user_id))
			})
			.collect();
		candidates.sort_unstable();
		let excess = total - low_water(self.limits.members);
		for (_, guild_id, user_id) in candidates.into_iter().take(excess) {
			if let Some(guild) = self.guilds.get_mut(&guild_id) {
				guild.members.remove(&user_id);
				changes.members.insert(guild_id);
			}
		}
	}

	fn evict_users(&mut self, in_view: &HashSet<(Option<Snowflake>, Snowflake)>, changes: &mut Changes) {
		let mut keep: HashSet<Snowflake> = in_view.iter().map(|(_, user_id)| *user_id).collect();
		keep.extend(self.current_user_id());
		keep.extend(self.relationships.keys().copied());
		for channel in self.private_channels.values() {
			keep.extend(channel.recipient_ids.iter().copied());
		}
		for guild in self.guilds.values() {
			keep.extend(guild.members.keys().copied());
		}
		let evicted = self
			.users
			.evict_to(self.limits.users, |user_id| keep.contains(user_id));
		for user_id in evicted {
			if self.presences.remove(&user_id).is_some() {
				changes.presence.insert(user_id);
			}
		}
	}
}
