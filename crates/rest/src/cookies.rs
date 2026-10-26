use std::io::BufReader;

use parking_lot::Mutex;
use reqwest::header::HeaderValue;
use tokio::sync::Notify;
use url::Url;

pub struct CookieJar {
	store: Mutex<cookie_store::CookieStore>,
	changed: Notify,
}

impl Default for CookieJar {
	fn default() -> Self {
		Self::from_store(cookie_store::CookieStore::new())
	}
}

impl std::fmt::Debug for CookieJar {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("CookieJar(redacted)")
	}
}

impl CookieJar {
	fn from_store(store: cookie_store::CookieStore) -> Self {
		Self {
			store: Mutex::new(store),
			changed: Notify::new(),
		}
	}

	pub fn load(saved: &[u8]) -> Self {
		match cookie_store::serde::json::load(BufReader::new(saved)) {
			Ok(store) => Self::from_store(store),
			Err(_) => {
				tracing::warn!("saved cookies are unreadable, starting with an empty jar");
				Self::default()
			}
		}
	}

	pub fn save(&self) -> Vec<u8> {
		let mut saved = Vec::new();
		if cookie_store::serde::json::save(&self.store.lock(), &mut saved).is_err() {
			saved.clear();
		}
		saved
	}

	pub fn adopt<'a>(&self, set_cookies: impl IntoIterator<Item = &'a str>, url: &Url) {
		let values: Vec<HeaderValue> = set_cookies
			.into_iter()
			.filter_map(|value| HeaderValue::from_str(value).ok())
			.collect();
		reqwest::cookie::CookieStore::set_cookies(self, &mut values.iter(), url);
	}

	pub async fn changed(&self) {
		self.changed.notified().await;
	}
}

impl reqwest::cookie::CookieStore for CookieJar {
	fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
		let cookies: Vec<_> = cookie_headers
			.filter_map(|value| value.to_str().ok())
			.filter_map(|value| cookie_store::RawCookie::parse(value.to_owned()).ok())
			.collect();
		if cookies.is_empty() {
			return;
		}
		let names = cookies
			.iter()
			.map(|cookie| cookie.name())
			.collect::<Vec<_>>()
			.join(",");
		tracing::debug!(%names, host = url.host_str().unwrap_or_default(), "cookies set");
		self.store.lock().store_response_cookies(cookies.into_iter(), url);
		self.changed.notify_one();
	}

	fn cookies(&self, url: &Url) -> Option<HeaderValue> {
		let joined = self
			.store
			.lock()
			.get_request_values(url)
			.map(|(name, value)| format!("{name}={value}"))
			.collect::<Vec<_>>()
			.join("; ");
		if joined.is_empty() {
			return None;
		}
		let mut value = HeaderValue::from_str(&joined).ok()?;
		value.set_sensitive(true);
		Some(value)
	}
}
