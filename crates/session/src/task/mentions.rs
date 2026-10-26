use std::time::Duration;

use protocol::Snowflake;
use rest::api::MentionsQuery;
use state::{Loaded, LocalEvent};
use tokio::time::Instant;

use super::Core;

const REFETCH_GAP: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(super) struct Mentions {
	fetched: Option<(Instant, MentionsQuery)>,
}

impl Mentions {
	fn due(&mut self, now: Instant, query: MentionsQuery) -> bool {
		let fresh = self
			.fetched
			.is_some_and(|(at, asked)| asked == query && now.duration_since(at) < REFETCH_GAP);
		if fresh {
			return false;
		}
		self.fetched = Some((now, query));
		true
	}
}

impl Core {
	pub(super) fn load_recent_mentions(&mut self, query: MentionsQuery) {
		if !self.mentions.due(Instant::now(), query) {
			tracing::debug!("recent mentions were fetched moments ago");
			return;
		}
		self.spawn_load("recent mentions", move |rest| async move {
			let messages = rest.recent_mentions(query).await?;
			if messages.dropped() > 0 {
				tracing::debug!(
					dropped = messages.dropped(),
					"skipped undecodable recent mentions"
				);
			}
			Ok(Loaded::RecentMentions {
				messages: messages.into_vec(),
			})
		});
	}

	pub(super) fn remove_recent_mention(&mut self, message_id: Snowflake) {
		self.apply_local(LocalEvent::RecentMentionRemoved { message_id });
		self.spawn_action("remove mention", move |rest| async move {
			rest.delete_recent_mention(message_id).await
		});
	}
}
