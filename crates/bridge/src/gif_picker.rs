use std::time::{Duration, Instant};

use protocol::{Attachment, Embed, EmbedKind, FavoriteGif, Gif, GifCategory, GifFormat};
use session::Command;
use state::State;

pub const GIF_SEARCH_DELAY: Duration = Duration::from_millis(300);
pub const GIF_SUGGESTION_CHIPS: usize = 8;
pub const FAVORITES_TITLE: &str = "Favorites";
const MIN_SUGGEST: usize = 2;
const MIN_TILE: f32 = 64.;
const MAX_TILE: f32 = 240.;

#[derive(Debug, Clone, PartialEq)]
pub enum GifView {
	Home {
		suggestions: Vec<String>,
		favorites: Vec<Gif>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum Active {
	Query(String),
	Favorites,
}

#[derive(Debug, Default)]
pub struct GifSearch {
	typed: String,
	due: Option<Instant>,
	active: Option<Active>,
}

fn normalized(text: &str) -> String {
	text.trim().to_lowercase()
}

impl GifSearch {
	pub fn open(&mut self, state: &State) -> Vec<Command> {
		*self = GifSearch::default();
		let mut commands = Vec::new();
		if state.gif_trending().is_none() {
			commands.push(Command::LoadGifTrending);
		}
		if !state.favorites_loaded() {
			commands.push(Command::LoadFavorites);
		}
		commands
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
		self.active = Some(Active::Query(query.clone()));
		if known {
			Vec::new()
		} else {
			vec![Command::SearchGifs { query }]
		}
	}

	pub fn show_favorites(&mut self) {
		self.typed.clear();
		self.due = None;
		self.active = Some(Active::Favorites);
	}

	pub fn active(&self) -> Option<&str> {
		match self.active.as_ref()? {
			Active::Query(query) => Some(query),
			Active::Favorites => Some(FAVORITES_TITLE),
		}
	}

	pub fn view(&self, state: &State) -> GifView {
		match &self.active {
			Some(Active::Query(query)) => {
				return GifView::Results {
					query: query.clone(),
					gifs: state.gif_search(query).map(<[Gif]>::to_vec),
				};
			}
			Some(Active::Favorites) => {
				return GifView::Results {
					query: FAVORITES_TITLE.to_owned(),
					gifs: Some(favorite_gifs(state)),
				};
			}
			None => {}
		}
		let trending = state.gif_trending();
		GifView::Home {
			suggestions: state
				.gif_suggestions(&self.typed)
				.filter(|_| !self.typed.is_empty())
				.map(|terms| terms.iter().take(GIF_SUGGESTION_CHIPS).cloned().collect())
				.unwrap_or_default(),
			favorites: favorite_gifs(state),
			categories: trending
				.map(|trending| trending.categories.to_vec())
				.unwrap_or_default(),
			trending: trending
				.map(|trending| trending.gifs.to_vec())
				.unwrap_or_default(),
		}
	}
}

pub fn favorite_gifs(state: &State) -> Vec<Gif> {
	state.favorite_gifs().iter().map(gif_of_favorite).collect()
}

fn dimension(value: Option<i64>) -> u32 {
	value.and_then(|value| u32::try_from(value).ok()).unwrap_or(0)
}

pub fn favorite_gif(embed: &Embed) -> Option<FavoriteGif> {
	if embed.kind != Some(EmbedKind::Gifv) {
		return None;
	}
	let url = embed.url.clone().filter(|url| !url.is_empty())?;
	let video = embed.video.as_ref()?;
	let src = video.url.clone().or_else(|| video.proxy_url.clone())?;
	Some(FavoriteGif {
		url,
		src,
		width: dimension(video.width),
		height: dimension(video.height),
		video: true,
		order: 0,
	})
}

pub fn favorite_gif_attachment(attachment: &Attachment) -> Option<FavoriteGif> {
	let gif = attachment
		.content_type
		.as_deref()
		.is_some_and(|kind| kind.eq_ignore_ascii_case("image/gif"))
		|| attachment.filename.to_ascii_lowercase().ends_with(".gif");
	let url = attachment.url.clone().filter(|url| gif && !url.is_empty())?;
	Some(FavoriteGif {
		src: url.clone(),
		url,
		width: dimension(attachment.width),
		height: dimension(attachment.height),
		video: false,
		order: 0,
	})
}

pub fn gif_of_favorite(favorite: &FavoriteGif) -> Gif {
	Gif {
		id: favorite.url.clone(),
		title: String::new(),
		url: favorite.url.clone(),
		src: favorite.src.clone(),
		gif_src: favorite.src.clone(),
		width: favorite.width,
		height: favorite.height,
		preview: String::new(),
		format: if favorite.video {
			GifFormat::Mp4
		} else {
			GifFormat::Gif
		},
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
