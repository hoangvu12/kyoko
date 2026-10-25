use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};

static CLOCK: AtomicU64 = AtomicU64::new(0);

fn tick() -> u64 {
	CLOCK.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn low_water(cap: usize) -> usize {
	cap - cap / 8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLimits {
	pub messages_per_channel: usize,
	pub history_ceiling: usize,
	pub warm_channels: usize,
	pub members: usize,
	pub users: usize,
	pub profiles: usize,
	pub referenced_messages: usize,
	pub kept_deleted: usize,
	pub kept_edited: usize,
	pub kept_versions: usize,
}

impl Default for CacheLimits {
	fn default() -> Self {
		Self {
			messages_per_channel: 150,
			history_ceiling: 1000,
			warm_channels: 8,
			members: 5000,
			users: 10_000,
			profiles: 128,
			referenced_messages: 256,
			kept_deleted: 100,
			kept_edited: 100,
			kept_versions: 10,
		}
	}
}

impl CacheLimits {
	pub fn window(&self) -> usize {
		self.messages_per_channel.max(1)
	}

	pub fn ceiling(&self) -> usize {
		self.history_ceiling.max(self.window())
	}

	pub fn warm(&self) -> usize {
		self.warm_channels.max(1)
	}
}

#[derive(Debug, Clone)]
pub struct Recent<K, V> {
	entries: HashMap<K, (V, u64)>,
}

impl<K, V> Default for Recent<K, V> {
	fn default() -> Self {
		Self {
			entries: HashMap::new(),
		}
	}
}

impl<K: Eq + Hash + Copy, V> Recent<K, V> {
	pub fn get(&self, key: &K) -> Option<&V> {
		self.entries.get(key).map(|(value, _)| value)
	}

	pub fn contains_key(&self, key: &K) -> bool {
		self.entries.contains_key(key)
	}

	pub fn len(&self) -> usize {
		self.entries.len()
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	pub fn keys(&self) -> impl Iterator<Item = &K> {
		self.entries.keys()
	}

	pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
		self.entries.iter().map(|(key, (value, _))| (key, value))
	}

	pub fn insert(&mut self, key: K, value: V) -> Option<V> {
		self.entries
			.insert(key, (value, tick()))
			.map(|(previous, _)| previous)
	}

	pub fn touch(&mut self, key: &K) -> bool {
		match self.entries.get_mut(key) {
			Some((_, stamp)) => {
				*stamp = tick();
				true
			}
			None => false,
		}
	}

	pub fn get_or_insert_with(&mut self, key: K, value: impl FnOnce() -> V) -> &mut V {
		match self.entries.entry(key) {
			Entry::Occupied(occupied) => {
				let (value, stamp) = occupied.into_mut();
				*stamp = tick();
				value
			}
			Entry::Vacant(vacant) => &mut vacant.insert((value(), tick())).0,
		}
	}

	pub fn remove(&mut self, key: &K) -> Option<V> {
		self.entries.remove(key).map(|(value, _)| value)
	}

	pub fn clear(&mut self) {
		self.entries.clear();
	}

	pub(crate) fn stamps(&self) -> impl Iterator<Item = (K, u64)> + '_ {
		self.entries.iter().map(|(key, (_, stamp))| (*key, *stamp))
	}

	pub(crate) fn evict_to(&mut self, cap: usize, keep: impl Fn(&K) -> bool) -> Vec<K> {
		if self.entries.len() <= cap {
			return Vec::new();
		}
		let mut candidates: Vec<(u64, K)> = self
			.stamps()
			.filter(|(key, _)| !keep(key))
			.map(|(key, stamp)| (stamp, key))
			.collect();
		candidates.sort_unstable_by_key(|(stamp, _)| *stamp);
		let excess = self.entries.len() - low_water(cap);
		let evicted: Vec<K> = candidates.into_iter().take(excess).map(|(_, key)| key).collect();
		for key in &evicted {
			self.entries.remove(key);
		}
		evicted
	}
}
