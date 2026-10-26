use std::time::Duration;

use protocol::Snowflake;
use serde_json::Value;
use state::{EmojiCandidate, EmojiRanking, Loaded};
use tokio::time::Instant;

use super::{Core, Internal};
use crate::hooks::{EmojiRankRequest, RankedEmoji, guarded_blocking};

const RANK_DELAY: Duration = Duration::from_millis(100);
const FRECENCY_RETRY: Duration = Duration::from_secs(60);

struct Pending {
	guild_id: Option<Snowflake>,
	query: String,
	candidates: Vec<EmojiCandidate>,
}

#[derive(Default)]
pub(super) struct EmojiRanker {
	pending: Option<Pending>,
	due: Option<Instant>,
	running: bool,
	frecency_asked: Option<Instant>,
}

impl EmojiRanker {
	pub(super) fn deadline(&self) -> Option<Instant> {
		self.due.filter(|_| !self.running && self.pending.is_some())
	}
}

impl Core {
	pub(super) fn rank_emoji(
		&mut self,
		guild_id: Option<Snowflake>,
		query: String,
		candidates: Vec<EmojiCandidate>,
	) {
		self.want_frecency();
		let ranks = self.hooks.as_deref().is_some_and(|hooks| hooks.ranks_emoji());
		if !ranks || candidates.is_empty() {
			self.ranker.pending = None;
			return;
		}
		self.ranker.pending = Some(Pending {
			guild_id,
			query,
			candidates,
		});
		self.ranker.due = Some(Instant::now() + RANK_DELAY);
	}

	pub(super) fn flush_emoji_rank(&mut self) {
		let Some(pending) = self.ranker.pending.take() else {
			return;
		};
		self.ranker.due = None;
		self.ranker.running = true;
		let (favorites, emojis) = {
			let state = self.state.read();
			let emojis = pending
				.candidates
				.into_iter()
				.map(|emoji| RankedEmoji {
					favorite: state.favorite_position(emoji.id, emoji.unicode.as_deref()),
					emoji,
				})
				.collect();
			(state.favorites_generation(), emojis)
		};
		let request = EmojiRankRequest {
			guild_id: pending.guild_id,
			query: pending.query,
			emojis,
		};
		let hooks = self.hooks.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let guild_id = request.guild_id;
			let query = request.query.clone();
			let order = guarded_blocking(&hooks, "rank_emoji", move |hooks| hooks.rank_emoji(&request))
				.await
				.flatten();
			let ranking = order.map(|order| EmojiRanking {
				guild_id,
				query,
				favorites,
				order,
			});
			let _ = internal.send(Internal::EmojiRanked(ranking));
		});
	}

	pub(super) fn emoji_ranked(&mut self, ranking: Option<EmojiRanking>) {
		self.ranker.running = false;
		if self.ranker.pending.is_some() {
			self.ranker.due = Some(self.ranker.due.unwrap_or_else(Instant::now));
		}
		if let Some(ranking) = ranking {
			self.on_loaded(Loaded::EmojiRanked(ranking));
		}
	}

	pub(super) fn want_frecency(&mut self) {
		if self.state.read().favorites_loaded() {
			return;
		}
		let now = Instant::now();
		if self
			.ranker
			.frecency_asked
			.is_some_and(|asked| now.duration_since(asked) < FRECENCY_RETRY)
		{
			return;
		}
		self.ranker.frecency_asked = Some(now);
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			match rest.frecency_settings().await {
				Ok(body) => {
					let encoded = body
						.get("settings")
						.and_then(Value::as_str)
						.unwrap_or_default()
						.to_owned();
					let _ = internal.send(Internal::Loaded(Loaded::Frecency(encoded)));
				}
				Err(error) => tracing::warn!(%error, "favourite emoji were not loaded"),
			}
		});
	}
}
