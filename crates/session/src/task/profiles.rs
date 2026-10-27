use std::collections::{HashMap, HashSet};
use std::time::Duration;

use protocol::Snowflake;
use rest::RestError;
use state::{Loaded, LocalEvent};
use tokio::time::Instant;

use super::{Core, Internal};

pub(crate) const PROFILE_TTL: Duration = Duration::from_secs(300);

type Key = (Snowflake, Option<Snowflake>);

#[derive(Default)]
pub(super) struct Profiles {
	fetched: HashMap<Key, Instant>,
	in_flight: HashSet<Key>,
}

impl Profiles {
	fn wanted(&mut self, key: Key, now: Instant) -> bool {
		if self.in_flight.contains(&key) {
			return false;
		}
		let fresh = self
			.fetched
			.get(&key)
			.is_some_and(|at| now.duration_since(*at) < PROFILE_TTL);
		if fresh {
			return false;
		}
		self.fetched.retain(|_, at| now.duration_since(*at) < PROFILE_TTL);
		self.in_flight.insert(key);
		true
	}

	fn finished(&mut self, key: Key, now: Instant, outcome: &ProfileOutcome) {
		self.in_flight.remove(&key);
		if !matches!(outcome, ProfileOutcome::Failed) {
			self.fetched.insert(key, now);
		}
	}

	fn fetching(&self, user_id: Snowflake) -> bool {
		self.in_flight.iter().any(|(user, _)| *user == user_id)
	}
}

pub(crate) enum ProfileOutcome {
	Loaded(Loaded),
	Unavailable,
	Failed,
}

impl Core {
	pub(super) fn load_profile(&mut self, user_id: Snowflake, guild_id: Option<Snowflake>) {
		let key = (user_id, guild_id);
		if !self.profiles.wanted(key, Instant::now()) {
			return;
		}
		self.apply_local(LocalEvent::ProfileRequested { user_id });
		let rest = self.rest.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let outcome = match rest.user_profile(user_id, guild_id).await {
				Ok(raw) => ProfileOutcome::Loaded(Loaded::Profile { user_id, raw }),
				Err(RestError::NotFound | RestError::Forbidden | RestError::MissingPermissions) => {
					ProfileOutcome::Unavailable
				}
				Err(error) => {
					tracing::debug!(%error, "the profile was not loaded");
					ProfileOutcome::Failed
				}
			};
			let _ = internal.send(Internal::Profile {
				user_id,
				guild_id,
				outcome,
			});
		});
	}

	pub(super) fn on_profile(
		&mut self,
		user_id: Snowflake,
		guild_id: Option<Snowflake>,
		outcome: ProfileOutcome,
	) {
		self.profiles
			.finished((user_id, guild_id), Instant::now(), &outcome);
		match outcome {
			ProfileOutcome::Loaded(loaded) => self.on_loaded(loaded),
			ProfileOutcome::Unavailable => {
				tracing::debug!("the profile is not available, showing what is known")
			}
			ProfileOutcome::Failed => {}
		}
		if !self.profiles.fetching(user_id) {
			self.apply_local(LocalEvent::ProfileSettled { user_id });
		}
	}
}
