use protocol::expressions::unicode::by_shortcode;
use protocol::{FavoriteGif, Snowflake, decode_favorite_emojis, decode_favorite_gifs};

use crate::changes::Changes;
use crate::state::State;

const VARIATION_SELECTOR: char = '\u{fe0f}';

#[derive(Debug, Clone, PartialEq, Eq)]
enum Favorite {
	Custom(Snowflake),
	Unicode(String),
	Unknown,
}

impl Favorite {
	fn of(entry: &str) -> Favorite {
		if let Ok(id) = entry.parse::<u64>() {
			return Favorite::Custom(Snowflake(id));
		}
		match by_shortcode(entry) {
			Some(emoji) => Favorite::Unicode(bare(emoji.emoji)),
			None if !entry.is_ascii() => Favorite::Unicode(bare(entry)),
			None => Favorite::Unknown,
		}
	}
}

fn bare(glyph: &str) -> String {
	glyph
		.chars()
		.filter(|character| *character != VARIATION_SELECTOR)
		.collect()
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Favorites {
	entries: Vec<String>,
	resolved: Vec<Favorite>,
	gifs: Vec<FavoriteGif>,
	loaded: bool,
	generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiCandidate {
	pub key: String,
	pub name: String,
	pub id: Option<Snowflake>,
	pub animated: bool,
	pub guild_id: Option<Snowflake>,
	pub unicode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiRanking {
	pub guild_id: Option<Snowflake>,
	pub query: String,
	pub favorites: u64,
	pub order: Vec<String>,
}

impl State {
	pub fn favorite_emojis(&self) -> &[String] {
		&self.lazy.expressions.favorites.entries
	}

	pub fn favorites_loaded(&self) -> bool {
		self.lazy.expressions.favorites.loaded
	}

	pub fn favorites_generation(&self) -> u64 {
		self.lazy.expressions.favorites.generation
	}

	pub fn favorite_position(&self, id: Option<Snowflake>, unicode: Option<&str>) -> Option<usize> {
		let wanted = match (id, unicode) {
			(Some(id), _) => Favorite::Custom(id),
			(None, Some(glyph)) => Favorite::Unicode(bare(glyph)),
			(None, None) => return None,
		};
		self.lazy
			.expressions
			.favorites
			.resolved
			.iter()
			.position(|favorite| *favorite == wanted)
	}

	pub fn emoji_ranking(&self, guild_id: Option<Snowflake>, query: &str) -> Option<&[String]> {
		self.lazy
			.expressions
			.emoji_ranking
			.as_ref()
			.filter(|ranking| {
				ranking.guild_id == guild_id
					&& ranking.query == query
					&& ranking.favorites == self.favorites_generation()
			})
			.map(|ranking| ranking.order.as_slice())
	}

	pub fn favorite_gifs(&self) -> &[FavoriteGif] {
		&self.lazy.expressions.favorites.gifs
	}

	pub fn is_favorite_gif(&self, url: &str) -> bool {
		self.favorite_gifs().iter().any(|gif| gif.url == url)
	}

	pub(crate) fn frecency_loaded(&mut self, encoded: &str) -> Changes {
		let favorites = decode_favorite_emojis(encoded).unwrap_or_default();
		let gifs = decode_favorite_gifs(encoded).unwrap_or_default();
		self.lazy.expressions.favorites.loaded = true;
		let mut changes = self.set_favorites(favorites);
		changes.merge(self.set_favorite_gifs(gifs));
		changes
	}

	pub(crate) fn frecency_updated(&mut self, encoded: &str, partial: bool) -> Changes {
		let mut changes = match decode_favorite_emojis(encoded) {
			Some(favorites) => self.set_favorites(favorites),
			None if !partial => self.set_favorites(Vec::new()),
			None => Changes::default(),
		};
		changes.merge(match decode_favorite_gifs(encoded) {
			Some(gifs) => self.set_favorite_gifs(gifs),
			None if !partial => self.set_favorite_gifs(Vec::new()),
			None => Changes::default(),
		});
		changes
	}

	pub(crate) fn favorite_gifs_changed(&mut self, gifs: Vec<FavoriteGif>) -> Changes {
		self.set_favorite_gifs(gifs)
	}

	fn set_favorite_gifs(&mut self, gifs: Vec<FavoriteGif>) -> Changes {
		let favorites = &mut self.lazy.expressions.favorites;
		if favorites.gifs == gifs {
			return Changes::default();
		}
		favorites.gifs = gifs;
		Changes {
			favorite_gifs: true,
			..Changes::default()
		}
	}

	pub(crate) fn emoji_ranked(&mut self, ranking: &EmojiRanking) -> Changes {
		self.lazy.expressions.emoji_ranking = Some(ranking.clone());
		Changes {
			emoji_ranking: true,
			..Changes::default()
		}
	}

	fn set_favorites(&mut self, entries: Vec<String>) -> Changes {
		let favorites = &mut self.lazy.expressions.favorites;
		if favorites.entries == entries {
			return Changes::default();
		}
		favorites.resolved = entries.iter().map(|entry| Favorite::of(entry)).collect();
		favorites.entries = entries;
		favorites.generation += 1;
		Changes {
			emoji_ranking: true,
			favorite_emojis: true,
			..Changes::default()
		}
	}

	pub(crate) fn favorite_emojis_changed(&mut self, entries: Vec<String>) -> Changes {
		self.set_favorites(entries)
	}

	pub fn is_favorite_emoji(&self, id: Option<Snowflake>, unicode: Option<&str>) -> bool {
		self.favorite_position(id, unicode).is_some()
	}
}
