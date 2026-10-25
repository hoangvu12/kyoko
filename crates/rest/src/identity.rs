use std::sync::Arc;
use std::time::Instant;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use protocol::fingerprint::{Clock, heartbeat_session_expired, plausible_installation_id};
use protocol::{Fingerprint, IdentifyProperties};

use crate::cookies::CookieJar;

#[derive(Clone, Debug)]
pub struct ClientIdentity {
	inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
	fingerprint: RwLock<Stamped>,
	installation_id: RwLock<Option<String>>,
	cookies: Arc<CookieJar>,
	clock: Clock,
}

#[derive(Debug)]
struct Stamped {
	fingerprint: Fingerprint,
	super_properties: String,
	heartbeat_started: Instant,
}

impl Stamped {
	fn new(fingerprint: Fingerprint, heartbeat_started: Instant) -> Self {
		Self {
			super_properties: fingerprint.super_properties(),
			fingerprint,
			heartbeat_started,
		}
	}

	fn renew_heartbeat_session(&mut self, now: Instant) {
		self.fingerprint.renew_heartbeat_session();
		self.super_properties = self.fingerprint.super_properties();
		self.heartbeat_started = now;
	}
}

impl ClientIdentity {
	pub fn new(fingerprint: Fingerprint, installation_id: Option<String>, cookies: CookieJar) -> Self {
		Self::with_clock(fingerprint, installation_id, cookies, Clock::system())
	}

	pub fn with_clock(
		fingerprint: Fingerprint,
		installation_id: Option<String>,
		cookies: CookieJar,
		clock: Clock,
	) -> Self {
		Self {
			inner: Arc::new(Inner {
				fingerprint: RwLock::new(Stamped::new(fingerprint, clock.now())),
				installation_id: RwLock::new(installation_id.filter(|id| plausible_installation_id(id))),
				cookies: Arc::new(cookies),
				clock,
			}),
		}
	}

	fn current(&self) -> RwLockReadGuard<'_, Stamped> {
		let now = self.inner.clock.now();
		let stamped = self.inner.fingerprint.read();
		if !heartbeat_session_expired(stamped.heartbeat_started, now) {
			return stamped;
		}
		drop(stamped);
		let mut stamped = self.inner.fingerprint.write();
		if heartbeat_session_expired(stamped.heartbeat_started, now) {
			stamped.renew_heartbeat_session(now);
		}
		RwLockWriteGuard::downgrade(stamped)
	}

	pub fn fingerprint(&self) -> Fingerprint {
		self.current().fingerprint.clone()
	}

	pub fn super_properties(&self) -> String {
		self.current().super_properties.clone()
	}

	pub fn identify_properties(&self) -> IdentifyProperties {
		let installation_id = self.installation_id();
		self.current()
			.fingerprint
			.identify_properties(installation_id.as_deref())
	}

	pub fn set_build_number(&self, build_number: u64) {
		let mut stamped = self.inner.fingerprint.write();
		let mut fingerprint = stamped.fingerprint.clone();
		fingerprint.set_build_number(build_number);
		*stamped = Stamped::new(fingerprint, stamped.heartbeat_started);
	}

	pub fn installation_id(&self) -> Option<String> {
		self.inner.installation_id.read().clone()
	}

	pub fn adopt_installation_id(&self, id: &str) -> bool {
		if !plausible_installation_id(id) {
			return false;
		}
		let mut current = self.inner.installation_id.write();
		if current.is_some() {
			return false;
		}
		*current = Some(id.to_owned());
		true
	}

	pub fn cookies(&self) -> &Arc<CookieJar> {
		&self.inner.cookies
	}
}

impl Default for ClientIdentity {
	fn default() -> Self {
		Self::new(
			Fingerprint::web_client("en-US", protocol::fingerprint::FALLBACK_BUILD_NUMBER),
			None,
			CookieJar::default(),
		)
	}
}
