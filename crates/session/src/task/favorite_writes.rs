use std::time::Duration;

use protocol::{FavoriteGif, encode_favorite_emojis, encode_favorite_gifs};
use rest::RestTuning;
use state::Loaded;
use tokio::time::Instant;

use super::{Core, Internal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FavoriteKind {
	Gifs,
	Emojis,
}

pub(super) struct Writer<T> {
	delay: Duration,
	waiting: Vec<(T, bool)>,
	due: Option<Instant>,
	sent: Option<Vec<T>>,
	confirmed: Option<Vec<T>>,
}

impl<T: Clone + PartialEq> Writer<T> {
	fn new(delay: Duration) -> Self {
		Self {
			delay,
			waiting: Vec::new(),
			due: None,
			sent: None,
			confirmed: None,
		}
	}

	fn deadline(&self) -> Option<Instant> {
		self.due.filter(|_| self.sent.is_none())
	}

	fn changed(&mut self, current: Vec<T>) {
		if self.confirmed.is_none() {
			self.confirmed = Some(current);
		}
		self.due = Some(Instant::now() + self.delay);
	}

	fn settled(&mut self, saved: bool) -> Option<Option<Vec<T>>> {
		let sent = self.sent.take();
		let waiting = self.due.is_some();
		match (saved, waiting) {
			(true, true) => self.confirmed = sent,
			(true, false) => self.confirmed = None,
			(false, true) => {}
			(false, false) => return Some(self.confirmed.take()),
		}
		None
	}
}

pub(super) struct FavoriteWrites {
	gifs: Writer<FavoriteGif>,
	emojis: Writer<String>,
}

impl FavoriteWrites {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			gifs: Writer::new(tuning.settings_delay),
			emojis: Writer::new(tuning.settings_delay),
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		[self.gifs.deadline(), self.emojis.deadline()]
			.into_iter()
			.flatten()
			.min()
	}

	fn due(&self, now: Instant) -> Vec<FavoriteKind> {
		let mut due = Vec::new();
		if self.gifs.deadline().is_some_and(|at| at <= now) {
			due.push(FavoriteKind::Gifs);
		}
		if self.emojis.deadline().is_some_and(|at| at <= now) {
			due.push(FavoriteKind::Emojis);
		}
		due
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

pub(super) fn toggled_emoji(current: &[String], entry: &str, favorite: bool) -> Vec<String> {
	let mut next: Vec<String> = current.iter().filter(|known| *known != entry).cloned().collect();
	if favorite {
		next.push(entry.to_owned());
	}
	next
}

fn rejected(kind: FavoriteKind) -> &'static str {
	match kind {
		FavoriteKind::Gifs => "The favourite GIFs could not be saved",
		FavoriteKind::Emojis => "The favourite emoji could not be saved",
	}
}

impl Core {
	pub(super) fn set_gif_favorite(&mut self, gif: FavoriteGif, favorite: bool) {
		if gif.url.is_empty() {
			return;
		}
		if !self.state.read().favorites_loaded() {
			self.favorite_writes.gifs.waiting.push((gif, favorite));
			self.want_frecency();
			return;
		}
		self.apply_gif_favorite(gif, favorite);
	}

	pub(super) fn set_emoji_favorite(&mut self, entry: String, favorite: bool) {
		if entry.is_empty() {
			return;
		}
		if !self.state.read().favorites_loaded() {
			self.favorite_writes.emojis.waiting.push((entry, favorite));
			self.want_frecency();
			return;
		}
		self.apply_emoji_favorite(&entry, favorite);
	}

	fn apply_gif_favorite(&mut self, gif: FavoriteGif, favorite: bool) {
		let current = self.state.read().favorite_gifs().to_vec();
		let next = toggled(&current, gif, favorite);
		if next == current {
			return;
		}
		self.favorite_writes.gifs.changed(current);
		self.on_loaded(Loaded::FavoriteGifs(next));
	}

	fn apply_emoji_favorite(&mut self, entry: &str, favorite: bool) {
		let current = self.state.read().favorite_emojis().to_vec();
		let next = toggled_emoji(&current, entry, favorite);
		if next == current {
			return;
		}
		self.favorite_writes.emojis.changed(current);
		self.on_loaded(Loaded::FavoriteEmojis(next));
	}

	pub(super) fn frecency_arrived(&mut self) {
		for (gif, favorite) in std::mem::take(&mut self.favorite_writes.gifs.waiting) {
			self.apply_gif_favorite(gif, favorite);
		}
		for (entry, favorite) in std::mem::take(&mut self.favorite_writes.emojis.waiting) {
			self.apply_emoji_favorite(&entry, favorite);
		}
	}

	pub(super) fn flush_favorites(&mut self) {
		for kind in self.favorite_writes.due(Instant::now()) {
			self.flush_favorite(kind);
		}
	}

	fn flush_favorite(&mut self, kind: FavoriteKind) {
		let settings = {
			let state = self.state.read();
			let writes = &mut self.favorite_writes;
			match kind {
				FavoriteKind::Gifs => {
					writes.gifs.due = None;
					let favorites = state.favorite_gifs().to_vec();
					let settings = encode_favorite_gifs(&favorites);
					writes.gifs.sent = Some(favorites);
					settings
				}
				FavoriteKind::Emojis => {
					writes.emojis.due = None;
					let favorites = state.favorite_emojis().to_vec();
					let settings = encode_favorite_emojis(&favorites);
					writes.emojis.sent = Some(favorites);
					settings
				}
			}
		};
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let saved = match rest.patch_frecency_settings(settings).await {
				Ok(_) => true,
				Err(error) => {
					tracing::warn!(%error, ?kind, "the favourites were not saved");
					false
				}
			};
			let _ = internal.send(Internal::FavoritesSaved(kind, saved));
		});
	}

	pub(super) fn on_favorites_saved(&mut self, kind: FavoriteKind, saved: bool) {
		let failed = match kind {
			FavoriteKind::Gifs => self
				.favorite_writes
				.gifs
				.settled(saved)
				.map(|previous| previous.map(Loaded::FavoriteGifs)),
			FavoriteKind::Emojis => self
				.favorite_writes
				.emojis
				.settled(saved)
				.map(|previous| previous.map(Loaded::FavoriteEmojis)),
		};
		let Some(previous) = failed else {
			return;
		};
		if let Some(previous) = previous {
			self.on_loaded(previous);
		}
		self.reject(rejected(kind));
	}
}
