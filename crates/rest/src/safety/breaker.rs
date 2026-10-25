use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use parking_lot::Mutex;
use tokio::time::Instant;

use crate::error::{RestError, Result};
use crate::tuning::RestTuning;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
	Allowed,
	Forbidden { missing_permissions: bool },
	Inconclusive,
}

#[derive(Default)]
struct Breaker {
	strikes: VecDeque<Instant>,
	open_until: Option<Instant>,
	trial: bool,
	missing_permissions: bool,
}

impl Breaker {
	fn suspended(&self, retry_in: Duration) -> RestError {
		RestError::Suspended {
			retry_in,
			missing_permissions: self.missing_permissions,
		}
	}
}

pub(crate) struct Breakers {
	strikes: usize,
	window: Duration,
	cool_down: Duration,
	routes: Mutex<HashMap<String, Breaker>>,
}

impl Breakers {
	pub fn new(tuning: &RestTuning) -> Self {
		Self {
			strikes: tuning.forbidden_strikes.max(1) as usize,
			window: tuning.forbidden_window,
			cool_down: tuning.breaker_cool_down,
			routes: Mutex::default(),
		}
	}

	pub fn admit(&self, route: &str) -> Result<bool> {
		let now = Instant::now();
		let mut routes = self.routes.lock();
		let Some(breaker) = routes.get_mut(route) else {
			return Ok(false);
		};
		let Some(open_until) = breaker.open_until else {
			return Ok(false);
		};
		if open_until > now {
			return Err(breaker.suspended(open_until - now));
		}
		if breaker.trial {
			return Err(breaker.suspended(Duration::ZERO));
		}
		breaker.trial = true;
		Ok(true)
	}

	pub fn settle(&self, route: &str, trial: bool, outcome: Outcome) {
		let now = Instant::now();
		let mut routes = self.routes.lock();
		match outcome {
			Outcome::Forbidden { missing_permissions } => {
				let breaker = routes.entry(route.to_owned()).or_default();
				breaker.missing_permissions = missing_permissions;
				breaker.trial = false;
				breaker
					.strikes
					.retain(|strike| now.duration_since(*strike) < self.window);
				breaker.strikes.push_back(now);
				if breaker.open_until.is_some() || breaker.strikes.len() >= self.strikes {
					tracing::warn!(route, "route suspended after repeated forbidden responses");
					breaker.open_until = Some(now + self.cool_down);
					breaker.strikes.clear();
				}
			}
			Outcome::Allowed => {
				routes.remove(route);
			}
			Outcome::Inconclusive => {
				if trial && let Some(breaker) = routes.get_mut(route) {
					breaker.trial = false;
				}
			}
		}
	}
}

pub(crate) struct Admission<'a> {
	breakers: &'a Breakers,
	route: &'a str,
	trial: bool,
	outcome: Outcome,
}

impl<'a> Admission<'a> {
	pub fn new(breakers: &'a Breakers, route: &'a str) -> Result<Self> {
		let trial = breakers.admit(route)?;
		Ok(Self {
			breakers,
			route,
			trial,
			outcome: Outcome::Inconclusive,
		})
	}

	pub fn record(&mut self, outcome: Outcome) {
		self.outcome = outcome;
	}
}

impl Drop for Admission<'_> {
	fn drop(&mut self) {
		self.breakers.settle(self.route, self.trial, self.outcome);
	}
}
