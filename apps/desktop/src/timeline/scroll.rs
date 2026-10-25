use std::time::{Duration, Instant};

use protocol::Snowflake;
use state::Viewport;

pub const AT_BOTTOM: f32 = 2.;
pub const NEAR_TOP_ROWS: usize = 14;
pub const RETRY: Duration = Duration::from_secs(3);
const ANCHOR_ROWS: usize = 8;

pub trait Extent {
	fn row_offset(&self, index: usize) -> f32;
	fn content_height(&self) -> f32;
	fn row_at(&self, offset: f32) -> Option<usize>;
}

impl Extent for [f32] {
	fn row_offset(&self, index: usize) -> f32 {
		self[..index.min(self.len())].iter().sum()
	}

	fn content_height(&self) -> f32 {
		self.iter().sum()
	}

	fn row_at(&self, offset: f32) -> Option<usize> {
		let mut top = 0.;
		for (index, height) in self.iter().enumerate() {
			top += height;
			if offset < top {
				return Some(index);
			}
		}
		self.len().checked_sub(1)
	}
}

pub fn limit(extent: &(impl Extent + ?Sized), viewport: f32) -> f32 {
	(extent.content_height() - viewport).max(0.)
}

pub fn at_bottom(extent: &(impl Extent + ?Sized), offset: f32, viewport: f32) -> bool {
	limit(extent, viewport) - offset.min(limit(extent, viewport)) <= AT_BOTTOM
}

pub fn visible(extent: &(impl Extent + ?Sized), offset: f32, viewport: f32) -> Option<(usize, usize)> {
	let offset = offset.clamp(0., limit(extent, viewport));
	let first = extent.row_at(offset)?;
	let last = extent
		.row_at((offset + viewport - 1.).max(offset))
		.unwrap_or(first)
		.max(first);
	Some((first, last))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anchor<K> {
	rows: Vec<(K, f32)>,
}

impl<K: Clone + PartialEq> Anchor<K> {
	pub fn capture(keys: &[K], extent: &(impl Extent + ?Sized), offset: f32, viewport: f32) -> Option<Self> {
		let offset = offset.clamp(0., limit(extent, viewport));
		let (first, _) = visible(extent, offset, viewport)?;
		let rows = keys
			.iter()
			.enumerate()
			.skip(first)
			.take(ANCHOR_ROWS)
			.map(|(index, key)| (key.clone(), offset - extent.row_offset(index)))
			.collect::<Vec<_>>();
		(!rows.is_empty()).then_some(Self { rows })
	}

	pub fn restore(&self, keys: &[K], extent: &(impl Extent + ?Sized)) -> Option<f32> {
		self.rows.iter().find_map(|(key, within)| {
			let index = keys.iter().position(|candidate| candidate == key)?;
			Some((extent.row_offset(index) + within).max(0.))
		})
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
	Older,
	Newer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ends {
	pub has_older: bool,
	pub detached: bool,
	pub oldest: Option<Snowflake>,
	pub newest: Option<Snowflake>,
}

#[derive(Debug, Default)]
pub struct Paging {
	older: Option<(Snowflake, Instant)>,
	newer: Option<(Snowflake, Instant)>,
}

impl Paging {
	pub fn reset(&mut self) {
		*self = Self::default();
	}

	pub fn wanted(
		&mut self,
		ends: Ends,
		first_visible: usize,
		rows: usize,
		bottom: bool,
		now: Instant,
	) -> Option<Page> {
		let near_top = first_visible < NEAR_TOP_ROWS || rows < NEAR_TOP_ROWS;
		if ends.has_older
			&& near_top
			&& let Some(oldest) = ends.oldest
			&& Self::stale(self.older, oldest, now)
		{
			self.older = Some((oldest, now));
			return Some(Page::Older);
		}
		if ends.detached
			&& bottom && let Some(newest) = ends.newest
			&& Self::stale(self.newer, newest, now)
		{
			self.newer = Some((newest, now));
			return Some(Page::Newer);
		}
		None
	}

	fn stale(last: Option<(Snowflake, Instant)>, edge: Snowflake, now: Instant) -> bool {
		last.is_none_or(|(id, at)| id != edge || now.duration_since(at) >= RETRY)
	}
}

#[derive(Debug, Default)]
pub struct Reporter {
	sent: Option<(Snowflake, Viewport)>,
	waiting: Option<(Snowflake, Viewport)>,
}

impl Reporter {
	pub fn reset(&mut self) {
		*self = Self::default();
	}

	pub fn offer(&mut self, channel_id: Snowflake, viewport: Viewport) -> bool {
		let scheduled = self.waiting.is_some();
		if self.sent == Some((channel_id, viewport)) {
			self.waiting = None;
			return false;
		}
		self.waiting = Some((channel_id, viewport));
		!scheduled
	}

	pub fn flush(&mut self) -> Option<(Snowflake, Viewport)> {
		let waiting = self.waiting.take()?;
		if self.sent == Some(waiting) {
			return None;
		}
		self.sent = Some(waiting);
		Some(waiting)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const VIEW: f32 = 100.;

	fn heights(count: usize, each: f32) -> Vec<f32> {
		vec![each; count]
	}

	#[test]
	fn the_first_visible_row_stays_put_when_rows_are_prepended() {
		let keys: Vec<u32> = (10..30).collect();
		let before = heights(20, 30.);
		let offset = 95.;
		let anchor = Anchor::capture(&keys, before.as_slice(), offset, VIEW).expect("an anchor");
		let mut grown: Vec<u32> = (0..10).collect();
		grown.extend(&keys);
		let mut after = vec![17.; 10];
		after.extend(&before);
		let restored = anchor
			.restore(&grown, after.as_slice())
			.expect("a restored offset");
		let first = 13usize;
		let screen_before = before.as_slice().row_offset(first - 10) - offset;
		let screen_after = after.as_slice().row_offset(first) - restored;
		assert!((screen_before - screen_after).abs() < 0.001);
		assert!((restored - (170. + 95.)).abs() < 0.001);
	}

	#[test]
	fn an_anchor_falls_back_to_the_next_surviving_row() {
		let keys: Vec<u32> = (0..10).collect();
		let before = heights(10, 40.);
		let anchor = Anchor::capture(&keys, before.as_slice(), 90., VIEW).expect("an anchor");
		let survivors: Vec<u32> = keys.iter().copied().filter(|key| *key != 2).collect();
		let after = heights(9, 40.);
		let restored = anchor.restore(&survivors, after.as_slice()).expect("an offset");
		let screen_before = before.as_slice().row_offset(3) - 90.;
		let screen_after = after.as_slice().row_offset(2) - restored;
		assert!((screen_before - screen_after).abs() < 0.001);
	}

	#[test]
	fn the_bottom_is_within_two_points_of_the_end() {
		let rows = heights(10, 50.);
		assert!(at_bottom(rows.as_slice(), 400., VIEW));
		assert!(at_bottom(rows.as_slice(), 398.5, VIEW));
		assert!(!at_bottom(rows.as_slice(), 390., VIEW));
		assert!(at_bottom(heights(1, 50.).as_slice(), 0., VIEW));
		assert!(at_bottom(rows.as_slice(), f32::MAX, VIEW));
	}

	#[test]
	fn visible_rows_span_the_viewport() {
		let rows = heights(10, 30.);
		assert_eq!(visible(rows.as_slice(), 0., VIEW), Some((0, 3)));
		assert_eq!(visible(rows.as_slice(), 45., VIEW), Some((1, 4)));
		assert_eq!(visible(rows.as_slice(), 1000., VIEW), Some((6, 9)));
		assert_eq!(visible([].as_slice(), 0., VIEW), None);
	}

	fn ends() -> Ends {
		Ends {
			has_older: true,
			detached: false,
			oldest: Some(Snowflake(100)),
			newest: Some(Snowflake(900)),
		}
	}

	#[test]
	fn older_pages_are_asked_for_once_near_the_top() {
		let mut paging = Paging::default();
		let now = Instant::now();
		assert_eq!(paging.wanted(ends(), 40, 60, false, now), None);
		assert_eq!(paging.wanted(ends(), 5, 60, false, now), Some(Page::Older));
		assert_eq!(paging.wanted(ends(), 3, 60, false, now), None);
		assert_eq!(
			paging.wanted(ends(), 3, 60, false, now + RETRY),
			Some(Page::Older)
		);
		let moved = Ends {
			oldest: Some(Snowflake(50)),
			..ends()
		};
		assert_eq!(paging.wanted(moved, 3, 60, false, now + RETRY), Some(Page::Older));
		let started = Ends {
			has_older: false,
			..moved
		};
		assert_eq!(paging.wanted(started, 0, 60, false, now + RETRY * 3), None);
	}

	#[test]
	fn a_short_channel_keeps_loading_older_pages() {
		let mut paging = Paging::default();
		assert_eq!(
			paging.wanted(ends(), 8, 9, true, Instant::now()),
			Some(Page::Older)
		);
	}

	#[test]
	fn a_detached_window_loads_newer_at_the_bottom() {
		let mut paging = Paging::default();
		let now = Instant::now();
		let detached = Ends {
			detached: true,
			has_older: false,
			..ends()
		};
		assert_eq!(paging.wanted(detached, 40, 60, false, now), None);
		assert_eq!(paging.wanted(detached, 40, 60, true, now), Some(Page::Newer));
		assert_eq!(paging.wanted(detached, 40, 60, true, now), None);
		let attached = Ends {
			detached: false,
			..detached
		};
		assert_eq!(paging.wanted(attached, 40, 60, true, now + RETRY), None);
	}

	fn viewport(first: u64, last: u64, at_bottom: bool) -> Viewport {
		Viewport {
			first: Snowflake(first),
			last: Snowflake(last),
			at_bottom,
		}
	}

	#[test]
	fn viewport_reports_coalesce_and_skip_repeats() {
		let mut reporter = Reporter::default();
		let channel = Snowflake(7);
		assert!(reporter.offer(channel, viewport(1, 5, true)));
		assert!(!reporter.offer(channel, viewport(2, 6, false)));
		assert_eq!(reporter.flush(), Some((channel, viewport(2, 6, false))));
		assert_eq!(reporter.flush(), None);
		assert!(!reporter.offer(channel, viewport(2, 6, false)));
		assert_eq!(reporter.flush(), None);
		assert!(reporter.offer(channel, viewport(3, 7, true)));
		assert!(!reporter.offer(channel, viewport(2, 6, false)));
		assert_eq!(reporter.flush(), None);
	}
}
