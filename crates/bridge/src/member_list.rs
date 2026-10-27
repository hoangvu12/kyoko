use std::time::{Duration, Instant};

use chrono::Utc;

use protocol::{MemberListGroup, Permissions, Snowflake};
use session::{Badge, Command, Formatting};
use state::{Changes, MemberList, MemberListRow, MemberSlot, State};

use crate::display::{Display, person};
use crate::dm_list::DmStatus;
use crate::hidden::{ModeratorNotes, moderator_notes};

pub const MEMBER_PUBLISH_GAP: Duration = Duration::from_millis(150);
const OVERSCAN: usize = 12;
const SHIFT_MARGIN: usize = 4;
const INITIAL_ROWS: usize = 24;
const MAX_ROWS: usize = 100_000;
pub const CROWN_MEMBER_LIMIT: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRowKind {
	Group,
	Member,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberHeights {
	pub member: f64,
	pub group: f64,
}

impl Default for MemberHeights {
	fn default() -> Self {
		Self {
			member: 42.,
			group: 24. + 12. * 1.618 + 4.,
		}
	}
}

impl MemberHeights {
	pub fn of(&self, kind: MemberRowKind) -> f64 {
		match kind {
			MemberRowKind::Group => self.group,
			MemberRowKind::Member => self.member,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberGroup {
	pub id: String,
	pub name: String,
	pub count: u32,
}

impl MemberGroup {
	pub fn title(&self) -> String {
		format!("{} — {}", self.name, self.count).to_uppercase()
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberEntry {
	pub user_id: Snowflake,
	pub name: String,
	pub extra: Option<String>,
	pub color: Option<u32>,
	pub status: DmStatus,
	pub activity: Option<String>,
	pub bot: bool,
	pub badge: Option<String>,
	pub notes: ModeratorNotes,
	pub crown: bool,
	pub decorations: Vec<Badge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberListEntry {
	Group(MemberGroup),
	Member(MemberEntry),
	Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberChange {
	pub layout: bool,
	pub window: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenMembers {
	pub guild_id: Snowflake,
	pub channel_id: Snowflake,
}

#[derive(Debug)]
pub struct MemberPanel {
	heights: MemberHeights,
	open: Option<OpenMembers>,
	layout: Vec<MemberRowKind>,
	tops: Vec<f64>,
	visible: Option<(usize, usize)>,
	last_scroll: Option<(f64, f64)>,
	reported: Option<(usize, usize)>,
	first: usize,
	entries: Vec<MemberListEntry>,
	restyled: bool,
	dirty_since: Option<Instant>,
	published_at: Option<Instant>,
	display: Display,
	moderator_notes: bool,
}

impl Default for MemberPanel {
	fn default() -> Self {
		Self::new(MemberHeights::default())
	}
}

impl MemberPanel {
	pub fn new(heights: MemberHeights) -> Self {
		Self {
			heights,
			open: None,
			layout: Vec::new(),
			tops: Vec::new(),
			visible: None,
			last_scroll: None,
			reported: None,
			first: 0,
			entries: Vec::new(),
			restyled: false,
			dirty_since: None,
			published_at: None,
			display: Display::default(),
			moderator_notes: false,
		}
	}

	pub fn set_display(&mut self, display: Display) {
		self.display = display;
	}

	pub fn reset(&mut self) {
		let display = std::mem::take(&mut self.display);
		let moderator_notes = self.moderator_notes;
		*self = Self::default();
		self.display = display;
		self.moderator_notes = moderator_notes;
	}

	pub fn shows_moderator_notes(&self) -> bool {
		self.moderator_notes
	}

	pub fn set_moderator_notes(&mut self, shown: bool, now: Instant) -> bool {
		if self.moderator_notes == shown {
			return false;
		}
		self.moderator_notes = shown;
		self.redraw(now)
	}

	pub fn redraw(&mut self, now: Instant) -> bool {
		if self.open.is_none() {
			return false;
		}
		self.mark(now);
		true
	}

	pub fn open(&self) -> Option<OpenMembers> {
		self.open
	}

	pub fn layout(&self) -> &[MemberRowKind] {
		&self.layout
	}

	pub fn heights(&self) -> impl Iterator<Item = f64> + '_ {
		self.layout.iter().map(|kind| self.heights.of(*kind))
	}

	pub fn first(&self) -> usize {
		self.first
	}

	pub fn entries(&self) -> &[MemberListEntry] {
		&self.entries
	}

	pub fn entry(&self, index: usize) -> Option<&MemberListEntry> {
		self.entries.get(index.checked_sub(self.first)?)
	}

	pub fn members(&self) -> impl Iterator<Item = &MemberEntry> {
		self.entries.iter().filter_map(|entry| match entry {
			MemberListEntry::Member(member) => Some(member),
			_ => None,
		})
	}

	pub fn show(&mut self, open: Option<OpenMembers>, now: Instant) -> bool {
		if self.open == open {
			return false;
		}
		self.open = open;
		self.visible = None;
		self.last_scroll = None;
		self.reported = None;
		self.mark(now);
		true
	}

	pub fn apply(&mut self, state: &State, changes: &Changes, now: Instant) -> bool {
		let Some(open) = self.open else {
			return false;
		};
		let presence = self
			.members()
			.any(|member| changes.presence.contains(member.user_id));
		if presence {
			tracing::debug!(guild_id = %open.guild_id, "a presence in the member list changed");
		}
		let relevant = changes.members.contains(open.guild_id) || changes.guild_list || presence;
		if relevant && state.guild(open.guild_id).is_some() {
			self.mark(now);
		}
		relevant
	}

	pub fn set_heights(&mut self, heights: MemberHeights, now: Instant) {
		if heights == self.heights {
			return;
		}
		self.heights = heights;
		self.tops = tops_of(&self.layout, heights);
		self.restyled = true;
		self.mark(now);
	}

	pub fn scroll(&mut self, offset: f64, height: f64, now: Instant) -> Option<Command> {
		self.last_scroll = Some((offset, height));
		let (first, last) = rows_at(&self.tops, offset, height)?;
		self.set_viewport(first, last, now)
	}

	pub fn set_viewport(&mut self, first: usize, last: usize, now: Instant) -> Option<Command> {
		let rows = (first, last.max(first));
		if self.visible == Some(rows) {
			return None;
		}
		self.visible = Some(rows);
		if !self.covers(rows) {
			self.mark(now);
		}
		let open = self.open?;
		if self.reported == Some(rows) {
			return None;
		}
		self.reported = Some(rows);
		Some(Command::SetMemberViewport {
			guild_id: open.guild_id,
			channel_id: open.channel_id,
			first: u32::try_from(rows.0).unwrap_or(u32::MAX),
			last: u32::try_from(rows.1).unwrap_or(u32::MAX),
		})
	}

	pub fn due(&self) -> Option<Instant> {
		let since = self.dirty_since?;
		Some(match self.published_at {
			Some(at) => since.max(at + MEMBER_PUBLISH_GAP),
			None => since,
		})
	}

	pub fn flush(&mut self, state: &State, now: Instant) -> Option<MemberChange> {
		if self.due().is_none_or(|due| due > now) {
			return None;
		}
		self.dirty_since = None;
		self.published_at = Some(now);
		let guild_id = self.open.map(|open| open.guild_id);
		let layout = guild_id
			.and_then(|guild_id| state.member_list(guild_id))
			.map(layout_of)
			.unwrap_or_default();
		let relaid = std::mem::take(&mut self.restyled) || layout != self.layout;
		if relaid {
			self.tops = tops_of(&layout, self.heights);
			self.layout = layout;
			if let Some(rows) = self
				.last_scroll
				.and_then(|(offset, height)| rows_at(&self.tops, offset, height))
			{
				self.visible = Some(rows);
			}
		}
		let (first, entries) = match (self.window(self.layout.len()), guild_id) {
			(Some((first, last)), Some(guild_id)) => {
				let entries = listed_entries(state, guild_id, &self.layout, first, last);
				(first, self.shown(state, guild_id, entries))
			}
			_ => (0, Vec::new()),
		};
		let change = MemberChange {
			layout: relaid,
			window: first != self.first || entries != self.entries,
		};
		self.first = first;
		self.entries = entries;
		(change.layout || change.window).then_some(change)
	}

	fn shown(
		&self,
		state: &State,
		guild_id: Snowflake,
		mut entries: Vec<MemberListEntry>,
	) -> Vec<MemberListEntry> {
		let names = self.display.formats(Formatting::Names);
		let decorated = self.display.formats(Formatting::Decorations);
		if !names && !decorated && !self.moderator_notes {
			return entries;
		}
		let now = Utc::now();
		for entry in &mut entries {
			let MemberListEntry::Member(member) = entry else {
				continue;
			};
			if self.moderator_notes {
				member.notes = moderator_notes(state, guild_id, member.user_id, now);
			}
			let Some(user) = state.user(member.user_id) else {
				continue;
			};
			if let Some(decorations) = self.display.decorations(state, Some(guild_id), user) {
				member.crown |= decorations.crown;
				member.decorations = decorations.members;
			}
			if !names {
				continue;
			}
			if let Some(shown) = self.display.name(&person(state, Some(guild_id), user)) {
				member.name = shown.name;
				member.extra = shown.extra;
			}
		}
		entries
	}

	fn mark(&mut self, now: Instant) {
		self.dirty_since.get_or_insert(now);
	}

	fn covers(&self, (first, last): (usize, usize)) -> bool {
		if self.entries.is_empty() {
			return self.layout.is_empty();
		}
		let end = self.first + self.entries.len() - 1;
		let wanted_first = first.saturating_sub(SHIFT_MARGIN);
		let wanted_last = (last + SHIFT_MARGIN).min(self.layout.len().saturating_sub(1));
		wanted_first >= self.first && wanted_last <= end
	}

	fn window(&self, len: usize) -> Option<(usize, usize)> {
		if len == 0 {
			return None;
		}
		let (first, last) = self.visible.unwrap_or((0, INITIAL_ROWS));
		let last = (last + OVERSCAN).min(len - 1);
		let first = first.saturating_sub(OVERSCAN).min(last);
		Some((first, last))
	}
}

fn listed_entries(
	state: &State,
	guild_id: Snowflake,
	layout: &[MemberRowKind],
	first: usize,
	last: usize,
) -> Vec<MemberListEntry> {
	let Some(list) = state.member_list(guild_id) else {
		return Vec::new();
	};
	let rows = state.member_list_rows(guild_id, first, last);
	(first..=last)
		.map(|index| match rows.get(index - first).cloned().flatten() {
			Some(row) => entry_of(state, guild_id, row),
			None if layout.get(index) == Some(&MemberRowKind::Group) => {
				inferred_group(state, guild_id, list, index)
					.map_or(MemberListEntry::Placeholder, MemberListEntry::Group)
			}
			None => MemberListEntry::Placeholder,
		})
		.collect()
}

pub fn layout_of(list: &MemberList) -> Vec<MemberRowKind> {
	let mut layout = Vec::new();
	for group in listed_groups(list) {
		if layout.len() >= MAX_ROWS {
			break;
		}
		layout.push(MemberRowKind::Group);
		let members = usize::try_from(group.count).unwrap_or(0);
		let room = MAX_ROWS - layout.len();
		layout.extend(std::iter::repeat_n(MemberRowKind::Member, members.min(room)));
	}
	layout.resize(list.row_count().min(MAX_ROWS), MemberRowKind::Member);
	for (index, slot) in list.slots.iter().enumerate().take(layout.len()) {
		match slot {
			MemberSlot::Group { .. } => layout[index] = MemberRowKind::Group,
			MemberSlot::Member(_) => layout[index] = MemberRowKind::Member,
			MemberSlot::Empty => {}
		}
	}
	layout
}

fn listed_groups(list: &MemberList) -> impl Iterator<Item = &MemberListGroup> {
	list.groups.iter().filter(|group| group.count > 0)
}

fn inferred_group(
	state: &State,
	guild_id: Snowflake,
	list: &MemberList,
	index: usize,
) -> Option<MemberGroup> {
	let mut header = 0usize;
	for group in listed_groups(list) {
		if header == index {
			return Some(MemberGroup {
				id: group.id.clone(),
				name: group_name(state, guild_id, &group.id),
				count: u32::try_from(group.count).unwrap_or(0),
			});
		}
		header = header.saturating_add(1 + usize::try_from(group.count).unwrap_or(0));
		if header > index {
			return None;
		}
	}
	None
}

fn group_name(state: &State, guild_id: Snowflake, id: &str) -> String {
	match id {
		"online" => "Online".to_owned(),
		"offline" => "Offline".to_owned(),
		_ => id
			.parse::<u64>()
			.ok()
			.and_then(|role_id| state.guild(guild_id)?.roles.get(&Snowflake(role_id)))
			.map_or_else(|| id.to_owned(), |role| role.name.clone()),
	}
}

fn entry_of(state: &State, guild_id: Snowflake, row: MemberListRow) -> MemberListEntry {
	match row {
		MemberListRow::Group { id, name, count } => MemberListEntry::Group(MemberGroup { id, name, count }),
		MemberListRow::Member(member) => MemberListEntry::Member(MemberEntry {
			user_id: member.user_id,
			name: member.display,
			extra: None,
			color: member.color,
			status: DmStatus::from_kind(&member.status),
			activity: member.activity,
			bot: is_bot(state, member.user_id),
			badge: member.badge,
			notes: ModeratorNotes::default(),
			crown: shows_crown(state, guild_id, member.user_id),
			decorations: Vec::new(),
		}),
	}
}

pub fn shows_crown(state: &State, guild_id: Snowflake, user_id: Snowflake) -> bool {
	let Some(guild) = state.guild(guild_id) else {
		return false;
	};
	let large = guild.member_count.is_some_and(|count| count > CROWN_MEMBER_LIMIT);
	let shown_admins = guild
		.roles
		.values()
		.any(|role| role.hoist && role.permissions.contains(Permissions::ADMINISTRATOR));
	guild.owner_id == Some(user_id) && !large && !shown_admins
}

fn is_bot(state: &State, user_id: Snowflake) -> bool {
	state.user(user_id).is_some_and(|user| user.bot == Some(true))
}

fn tops_of(layout: &[MemberRowKind], heights: MemberHeights) -> Vec<f64> {
	let mut top = 0.;
	layout
		.iter()
		.map(|kind| {
			let at = top;
			top += heights.of(*kind);
			at
		})
		.collect()
}

fn rows_at(tops: &[f64], offset: f64, height: f64) -> Option<(usize, usize)> {
	if tops.is_empty() || !offset.is_finite() || !height.is_finite() {
		return None;
	}
	let offset = offset.max(0.);
	let bottom = offset + height.max(0.);
	let first = tops.partition_point(|top| *top <= offset).saturating_sub(1);
	let last = tops
		.partition_point(|top| *top < bottom)
		.saturating_sub(1)
		.max(first);
	Some((first, last))
}
