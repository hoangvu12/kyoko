use std::time::{Duration, Instant};

use protocol::{Gif, GifCategory};
use session::Command;
use state::State;

pub const GIF_SEARCH_DELAY: Duration = Duration::from_millis(300);
pub const GIF_SUGGESTION_CHIPS: usize = 8;
const MIN_SUGGEST: usize = 2;
const MIN_TILE: f32 = 64.;
const MAX_TILE: f32 = 240.;

#[derive(Debug, Clone, PartialEq)]
pub enum GifView {
	Home {
		suggestions: Vec<String>,
		categories: Vec<GifCategory>,
		trending: Vec<Gif>,
	},
	Results {
		query: String,
		gifs: Option<Vec<Gif>>,
	},
}

#[derive(Debug, Clone, PartialEq)]
pub struct GifTile {
	pub gif: Gif,
	pub height: f32,
}

#[derive(Debug, Default)]
pub struct GifSearch {
	typed: String,
	due: Option<Instant>,
	active: Option<String>,
}

fn normalized(text: &str) -> String {
	text.trim().to_lowercase()
}

impl GifSearch {
	pub fn open(&mut self, state: &State) -> Vec<Command> {
		*self = GifSearch::default();
		match state.gif_trending() {
			Some(_) => Vec::new(),
			None => vec![Command::LoadGifTrending],
		}
	}

	pub fn typed(&mut self, state: &State, text: &str, now: Instant) -> Vec<Command> {
		let query = normalized(text);
		if query == self.typed {
			return Vec::new();
		}
		self.typed.clone_from(&query);
		if query.is_empty() {
			self.due = None;
			self.active = None;
			return Vec::new();
		}
		self.due = Some(now + GIF_SEARCH_DELAY);
		let suggest = query.chars().count() >= MIN_SUGGEST && state.gif_suggestions(&query).is_none();
		if suggest {
			vec![Command::SuggestGifs { query }]
		} else {
			Vec::new()
		}
	}

	pub fn due_at(&self) -> Option<Instant> {
		self.due
	}

	pub fn poll(&mut self, state: &State, now: Instant) -> Vec<Command> {
		match self.due {
			Some(due) if due <= now => {
				let query = self.typed.clone();
				self.submit(state, &query)
			}
			_ => Vec::new(),
		}
	}

	pub fn submit(&mut self, state: &State, text: &str) -> Vec<Command> {
		let query = normalized(text);
		self.typed.clone_from(&query);
		self.due = None;
		if query.is_empty() {
			self.active = None;
			return Vec::new();
		}
		let known = state.gif_search(&query).is_some();
		self.active = Some(query.clone());
		if known {
			Vec::new()
		} else {
			vec![Command::SearchGifs { query }]
		}
	}

	pub fn active(&self) -> Option<&str> {
		self.active.as_deref()
	}

	pub fn view(&self, state: &State) -> GifView {
		if let Some(query) = &self.active {
			return GifView::Results {
				query: query.clone(),
				gifs: state.gif_search(query).map(<[Gif]>::to_vec),
			};
		}
		let trending = state.gif_trending();
		GifView::Home {
			suggestions: state
				.gif_suggestions(&self.typed)
				.filter(|_| !self.typed.is_empty())
				.map(|terms| terms.iter().take(GIF_SUGGESTION_CHIPS).cloned().collect())
				.unwrap_or_default(),
			categories: trending
				.map(|trending| trending.categories.to_vec())
				.unwrap_or_default(),
			trending: trending
				.map(|trending| trending.gifs.to_vec())
				.unwrap_or_default(),
		}
	}
}

pub fn gif_tile_height(gif: &Gif, column: f32) -> f32 {
	let ratio = if gif.width == 0 {
		1.
	} else {
		gif.height as f32 / gif.width as f32
	};
	(column * ratio).clamp(MIN_TILE, MAX_TILE)
}

pub fn gif_masonry(gifs: &[Gif], column: f32, gap: f32) -> [Vec<GifTile>; 2] {
	let mut columns: [Vec<GifTile>; 2] = [Vec::new(), Vec::new()];
	let mut heights = [0_f32; 2];
	for gif in gifs {
		let height = gif_tile_height(gif, column);
		let side = usize::from(heights[0] > heights[1]);
		heights[side] += height + gap;
		columns[side].push(GifTile {
			gif: gif.clone(),
			height,
		});
	}
	columns
}

pub fn gif_still(gif: &Gif) -> &str {
	if gif.preview.is_empty() {
		&gif.gif_src
	} else {
		&gif.preview
	}
}

pub fn gif_motion(gif: &Gif) -> Option<&str> {
	(!gif.gif_src.is_empty()).then_some(gif.gif_src.as_str())
}

pub fn is_video_url(url: &str) -> bool {
	let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
	path.ends_with(".mp4") || path.ends_with(".webm")
}
