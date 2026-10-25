use std::sync::Arc;

use parking_lot::RwLock;
use protocol::Fingerprint;
use protocol::fingerprint::plausible_installation_id;

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
}

#[derive(Debug)]
struct Stamped {
	fingerprint: Fingerprint,
	super_properties: String,
}

impl Stamped {
	fn new(fingerprint: Fingerprint) -> Self {
		Self {
			super_properties: fingerprint.super_properties(),
			fingerprint,
		}
	}
}

impl ClientIdentity {
	pub fn new(fingerprint: Fingerprint, installation_id: Option<String>, cookies: CookieJar) -> Self {
		Self {
			inner: Arc::new(Inner {
				fingerprint: RwLock::new(Stamped::new(fingerprint)),
				installation_id: RwLock::new(installation_id.filter(|id| plausible_installation_id(id))),
				cookies: Arc::new(cookies),
			}),
		}
	}

	pub fn fingerprint(&self) -> Fingerprint {
		self.inner.fingerprint.read().fingerprint.clone()
	}

	pub fn super_properties(&self) -> String {
		self.inner.fingerprint.read().super_properties.clone()
	}

	pub fn set_build_number(&self, build_number: u64) {
		let mut stamped = self.inner.fingerprint.write();
		let mut fingerprint = stamped.fingerprint.clone();
		fingerprint.set_build_number(build_number);
		*stamped = Stamped::new(fingerprint);
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
