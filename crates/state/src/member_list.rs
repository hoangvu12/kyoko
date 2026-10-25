use protocol::{MemberListGroup, Snowflake};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MemberSlot {
	#[default]
	Empty,
	Group {
		id: String,
		count: i64,
	},
	Member(Snowflake),
}

#[derive(Debug, Default)]
pub struct MemberList {
	pub id: String,
	pub groups: Vec<MemberListGroup>,
	pub slots: Vec<MemberSlot>,
	pub member_count: i64,
	pub online_count: i64,
}

impl MemberList {
	pub fn reset(&mut self, id: &str) {
		self.id = id.to_owned();
		self.slots.clear();
		self.groups.clear();
	}

	pub fn sync(&mut self, start: usize, end: usize, items: Vec<MemberSlot>) {
		let items_end = start + items.len();
		let range_len = (end + 1).saturating_sub(start);
		if items.len() < range_len {
			self.slots.truncate(items_end);
		}
		if self.slots.len() < items_end {
			self.slots.resize(items_end, MemberSlot::Empty);
		}
		for (offset, slot) in items.into_iter().enumerate() {
			self.slots[start + offset] = slot;
		}
	}

	pub fn insert(&mut self, index: usize, slot: MemberSlot) {
		if self.slots.len() < index {
			self.slots.resize(index, MemberSlot::Empty);
		}
		self.slots.insert(index, slot);
	}

	pub fn update(&mut self, index: usize, slot: MemberSlot) {
		if self.slots.len() <= index {
			self.slots.resize(index + 1, MemberSlot::Empty);
		}
		self.slots[index] = slot;
	}

	pub fn delete(&mut self, index: usize) {
		if index < self.slots.len() {
			self.slots.remove(index);
		}
	}

	pub fn invalidate(&mut self, start: usize, end: usize) {
		let end = end.min(self.slots.len().saturating_sub(1));
		if start > end || self.slots.is_empty() {
			return;
		}
		for slot in &mut self.slots[start..=end] {
			*slot = MemberSlot::Empty;
		}
	}

	pub fn row_count(&self) -> usize {
		let grouped: i64 = self
			.groups
			.iter()
			.filter(|group| group.count > 0)
			.map(|group| group.count + 1)
			.sum();
		self.slots.len().max(usize::try_from(grouped).unwrap_or(0))
	}

	pub fn rows(&self, first: usize, last: usize) -> &[MemberSlot] {
		let end = last.saturating_add(1).min(self.slots.len());
		&self.slots[first.min(end)..end]
	}

	pub fn group_count(&self, id: &str) -> Option<i64> {
		self.groups
			.iter()
			.find(|group| group.id == id)
			.map(|group| group.count)
	}
}
