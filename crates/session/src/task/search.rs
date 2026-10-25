use std::sync::Arc;
use std::time::Duration;

use protocol::Snowflake;
use rest::api::{SearchQuery, SearchReply};
use rest::{RestError, RestTuning};
use state::Changes;
use tokio::time::Instant;

use super::failures::send_failure;
use super::{Core, Internal};
use crate::search::{Page, SearchFailure, SearchResults, SearchStatus};

const INDEX_TRIES: u32 = 5;
const INDEX_WAIT: Duration = Duration::from_secs(2);
const INDEX_WAIT_LIMIT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct Wanted {
	serial: u64,
	guild_id: Option<Snowflake>,
	channel_id: Option<Snowflake>,
	query: SearchQuery,
	offset: u32,
	tries: u32,
}

pub(crate) enum Outcome {
	Found(Page),
	Indexing(Option<f64>),
	Failed(RestError),
}

pub(super) struct Searcher {
	delay: Duration,
	serial: u64,
	wanted: Option<Wanted>,
	due: Option<Instant>,
	in_flight: bool,
}

impl Searcher {
	pub(super) fn new(tuning: &RestTuning) -> Self {
		Self {
			delay: tuning.search_delay,
			serial: 0,
			wanted: None,
			due: None,
			in_flight: false,
		}
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		if self.in_flight {
			return None;
		}
		self.due
	}

	fn index_wait(&self, retry_after: Option<f64>) -> Duration {
		retry_after
			.filter(|seconds| seconds.is_finite() && *seconds >= 0.)
			.map_or(INDEX_WAIT, Duration::from_secs_f64)
			.clamp(self.delay, INDEX_WAIT_LIMIT.max(self.delay))
	}
}

impl Core {
	pub(super) fn search(
		&mut self,
		guild_id: Option<Snowflake>,
		channel_id: Option<Snowflake>,
		query: SearchQuery,
	) {
		if guild_id.is_none() && channel_id.is_none() {
			return;
		}
		self.searcher.serial += 1;
		let serial = self.searcher.serial;
		self.searcher.wanted = Some(Wanted {
			serial,
			guild_id,
			channel_id,
			query,
			offset: 0,
			tries: 0,
		});
		self.searcher.due = Some(Instant::now() + self.searcher.delay);
		self.publish_search(Some(SearchResults::started(serial, guild_id, channel_id)));
	}

	pub(super) fn search_more(&mut self) {
		let current = self.search.lock().clone();
		let Some(current) = current.filter(|current| current.has_more()) else {
			return;
		};
		let Some(wanted) = self
			.searcher
			.wanted
			.as_mut()
			.filter(|wanted| wanted.serial == current.serial)
		else {
			return;
		};
		if self.searcher.due.is_some() {
			return;
		}
		wanted.offset = current.messages.len() as u32;
		wanted.tries = 0;
		self.searcher.due = Some(Instant::now());
		let mut more = (*current).clone();
		more.status = SearchStatus::Searching;
		self.publish_search(Some(more));
	}

	pub(super) fn close_search(&mut self) {
		self.searcher.wanted = None;
		self.searcher.due = None;
		self.publish_search(None);
	}

	pub(super) fn flush_search(&mut self) {
		if self.searcher.in_flight {
			return;
		}
		self.searcher.due = None;
		let Some(wanted) = self.searcher.wanted.clone() else {
			return;
		};
		self.searcher.in_flight = true;
		let query = SearchQuery {
			offset: Some(wanted.offset),
			..wanted.query.clone()
		};
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let outcome = match rest
				.search_messages(wanted.guild_id, wanted.channel_id, query)
				.await
			{
				Ok(SearchReply::Found(response)) => Outcome::Found(Page::from_response(&response)),
				Ok(SearchReply::Indexing { retry_after }) => Outcome::Indexing(retry_after),
				Err(error) => Outcome::Failed(error),
			};
			let _ = internal.send(Internal::Searched {
				serial: wanted.serial,
				offset: wanted.offset,
				outcome,
			});
		});
	}

	pub(super) fn on_searched(&mut self, serial: u64, offset: u32, outcome: Outcome) {
		self.searcher.in_flight = false;
		let current = self.search.lock().clone();
		let Some(mut results) = current.map(|current| (*current).clone()) else {
			return;
		};
		let Some(wanted) = self
			.searcher
			.wanted
			.as_mut()
			.filter(|wanted| wanted.serial == serial && wanted.offset == offset)
		else {
			return;
		};
		if results.serial != serial {
			return;
		}
		match outcome {
			Outcome::Found(page) => {
				if offset == 0 {
					results.messages.clear();
				}
				results.add_page(page);
			}
			Outcome::Indexing(retry_after) => {
				wanted.tries += 1;
				if wanted.tries >= INDEX_TRIES {
					tracing::info!(tries = wanted.tries, "the search index was still not ready");
					results.status = SearchStatus::Failed(SearchFailure::StillIndexing);
				} else {
					let wait = self.searcher.index_wait(retry_after);
					self.searcher.due = Some(Instant::now() + wait);
					results.status = SearchStatus::Indexing;
				}
			}
			Outcome::Failed(error) => {
				tracing::warn!(%error, "search failed");
				results.status = SearchStatus::Failed(SearchFailure::Refused(send_failure(&error)));
			}
		}
		self.publish_search(Some(results));
	}

	fn publish_search(&mut self, results: Option<SearchResults>) {
		*self.search.lock() = results.map(Arc::new);
		self.notifier.push(Changes {
			search: true,
			..Changes::default()
		});
	}
}
