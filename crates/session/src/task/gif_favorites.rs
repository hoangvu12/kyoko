use std::time::Duration;

use protocol::{FavoriteGif, encode_favorite_gifs};
use rest::RestTuning;
use state::Loaded;
use tokio::time::Instant;

use super::{Core, Internal};

pub(super) struct GifFavorites {
	delay: Duration,
	waiting: Vec<(FavoriteGif, bool)>,
	due: Option<Instant>,
	sent: Option<Vec<FavoriteGif>>,
	confirmed: Option<Vec<FavoriteGif>>,
}

impl GifFavorites {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.settings_delay,
			waiting: Vec::new(),
			due: None,
			sent: None,
			confirmed: None,
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		self.due.filter(|_| self.sent.is_none())
	}
}

pub(super) fn toggled(current: &[FavoriteGif], gif: FavoriteGif, favorite: bool) -> Vec<FavoriteGif> {
	let mut next: Vec<FavoriteGif> = current
		.iter()
		.filter(|known| known.url != gif.url)
		.cloned()
		.collect();
	if favorite {
		let order = current.iter().map(|known| known.order).max().unwrap_or(0) + 1;
		next.insert(0, FavoriteGif { order, ..gif });
	}
	next
}

impl Core {
	pub(super) fn set_gif_favorite(&mut self, gif: FavoriteGif, favorite: bool) {
		if gif.url.is_empty() {
			return;
		}
		if !self.state.read().favorites_loaded() {
			self.gif_favorites.waiting.push((gif, favorite));
			self.want_frecency();
			return;
		}
		self.apply_gif_favorite(gif, favorite);
	}

	fn apply_gif_favorite(&mut self, gif: FavoriteGif, favorite: bool) {
		let current = self.state.read().favorite_gifs().to_vec();
		let next = toggled(&current, gif, favorite);
		if next == current {
			return;
		}
		if self.gif_favorites.confirmed.is_none() {
			self.gif_favorites.confirmed = Some(current);
		}
		self.on_loaded(Loaded::FavoriteGifs(next));
		self.gif_favorites.due = Some(Instant::now() + self.gif_favorites.delay);
	}

	pub(super) fn frecency_arrived(&mut self) {
		for (gif, favorite) in std::mem::take(&mut self.gif_favorites.waiting) {
			self.apply_gif_favorite(gif, favorite);
		}
	}

	pub(super) fn flush_gif_favorites(&mut self) {
		self.gif_favorites.due = None;
		let favorites = self.state.read().favorite_gifs().to_vec();
		let settings = encode_favorite_gifs(&favorites);
		self.gif_favorites.sent = Some(favorites);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let saved = match rest.patch_frecency_settings(settings).await {
				Ok(_) => true,
				Err(error) => {
					tracing::warn!(%error, "the favourite GIFs were not saved");
					false
				}
			};
			let _ = internal.send(Internal::GifFavoritesSaved(saved));
		});
	}

	pub(super) fn on_gif_favorites_saved(&mut self, saved: bool) {
		let sent = self.gif_favorites.sent.take();
		let waiting = self.gif_favorites.due.is_some();
		match (saved, waiting) {
			(true, true) => self.gif_favorites.confirmed = sent,
			(true, false) => self.gif_favorites.confirmed = None,
			(false, true) => {}
			(false, false) => {
				if let Some(previous) = self.gif_favorites.confirmed.take() {
					self.on_loaded(Loaded::FavoriteGifs(previous));
				}
				self.reject("The favourite GIFs could not be saved");
			}
		}
	}
}
