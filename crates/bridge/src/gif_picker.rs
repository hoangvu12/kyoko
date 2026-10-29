use std::time::{Duration, Instant};

use protocol::{Attachment, Embed, EmbedKind, FavoriteGif, Gif, GifCategory, GifFormat};
use session::Command;
use state::State;

pub const GIF_SEARCH_DELAY: Duration = Duration::from_millis(300);
pub const FAVORITES_TITLE: &str = "Favorites";
pub const TRENDING_TITLE: &str = "Trending GIFs";
const MIN_TILE: f32 = 64.;
const MAX_TILE: f32 = 240.;

#[derive(Debug, Clone, PartialEq)]
pub enum GifView {
	Home {
		favorites: Vec<Gif>,
		trending: Vec<Gif>,
		categories: Vec<GifCategory>,
	},
	Results {
		query: String,
		browsing: bool,
		gifs: Option<Vec<Gif>>,
		favorites: Vec<String>,
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
	Category(String),
	Trending,
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
		if state.favorites_loaded() {
			Vec::new()
		} else {
			vec![Command::LoadFavorites]
		}
	}

	pub fn shown(&self, state: &State) -> Vec<Command> {
		if state.gif_trending().is_some() {
			Vec::new()
		} else {
			vec![Command::LoadGifTrending]
		}
	}

	pub fn typed(&mut self, _state: &State, text: &str, now: Instant) -> Vec<Command> {
		let query = normalized(text);
		if query == self.typed {
			return Vec::new();
		}
		self.typed.clone_from(&query);
		if query.is_empty() {
			self.due = None;
			if matches!(self.active, Some(Active::Query(_))) {
				self.active = None;
			}
			return Vec::new();
		}
		self.due = Some(now + GIF_SEARCH_DELAY);
		Vec::new()
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
		self.active = Some(Active::Query(query.clone()));
		searched(state, query)
	}

	pub fn browse_category(&mut self, state: &State, name: &str) -> Vec<Command> {
		let query = normalized(name);
		self.typed.clear();
		self.due = None;
		if query.is_empty() {
			return Vec::new();
		}
		self.active = Some(Active::Category(query.clone()));
		searched(state, query)
	}

	pub fn show_trending(&mut self, state: &State) -> Vec<Command> {
		self.typed.clear();
		self.due = None;
		self.active = Some(Active::Trending);
		let mut commands = Vec::new();
		if state.gif_trending_gifs().is_none() {
			commands.push(Command::LoadTrendingGifs);
		}
		commands
	}

	pub fn show_favorites(&mut self) {
		self.typed.clear();
		self.due = None;
		self.active = Some(Active::Favorites);
	}

	pub fn back(&mut self) {
		*self = GifSearch::default();
	}

	pub fn browsing(&self) -> bool {
		matches!(
			self.active,
			Some(Active::Category(_) | Active::Trending | Active::Favorites)
		)
	}

	pub fn active(&self) -> Option<&str> {
		match self.active.as_ref()? {
			Active::Query(query) | Active::Category(query) => Some(query),
			Active::Trending | Active::Favorites => None,
		}
	}

	pub fn view(&self, state: &State) -> GifView {
		let favorites = || state.favorite_gifs().iter().map(|gif| gif.url.clone()).collect();
		let results = |query: &str, browsing: bool, gifs: Option<Vec<Gif>>| GifView::Results {
			query: query.to_owned(),
			browsing,
			gifs,
			favorites: favorites(),
		};
		match &self.active {
			Some(Active::Query(query)) => results(query, false, state.gif_search(query).map(<[Gif]>::to_vec)),
			Some(Active::Category(query)) => {
				results(query, true, state.gif_search(query).map(<[Gif]>::to_vec))
			}
			Some(Active::Trending) => results(
				TRENDING_TITLE,
				true,
				state.gif_trending_gifs().map(<[Gif]>::to_vec),
			),
			Some(Active::Favorites) => results(FAVORITES_TITLE, true, Some(favorite_gifs(state))),
			None => {
				let trending = state.gif_trending();
				GifView::Home {
					favorites: favorite_gifs(state),
					trending: trending
						.map(|trending| trending.gifs.to_vec())
						.unwrap_or_default(),
					categories: trending
						.map(|trending| trending.categories.to_vec())
						.unwrap_or_default(),
				}
			}
		}
	}
}

fn searched(state: &State, query: String) -> Vec<Command> {
	if state.gif_search(&query).is_some() {
		Vec::new()
	} else {
		vec![Command::SearchGifs { query }]
	}
}

pub fn favorite_of_gif(gif: &Gif) -> FavoriteGif {
	let src = if gif.src.is_empty() {
		gif.gif_src.clone()
	} else {
		gif.src.clone()
	};
	FavoriteGif {
		url: gif.url.clone(),
		video: is_video_url(&src),
		src,
		width: gif.width,
		height: gif.height,
		order: 0,
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

pub fn is_video_url(url: &str) -> bool {
	let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
	path.ends_with(".mp4") || path.ends_with(".webm")
}
