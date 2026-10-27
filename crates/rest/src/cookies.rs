use std::io::BufReader;

use parking_lot::Mutex;
use tokio::sync::Notify;
use url::Url;
use wreq::cookie::{CookieStore, Cookies};
use wreq::header::HeaderValue;
use wreq::{Uri, Version};

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
		let cookies: Vec<_> = set_cookies
			.into_iter()
			.filter_map(|value| cookie_store::RawCookie::parse(value.to_owned()).ok())
			.collect();
		self.store_cookies(cookies, url);
	}

	pub async fn changed(&self) {
		self.changed.notified().await;
	}

	fn store_cookies(&self, cookies: Vec<cookie_store::RawCookie<'static>>, url: &Url) {
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

	fn request_pairs(&self, url: &Url) -> Vec<String> {
		self.store
			.lock()
			.get_request_values(url)
			.map(|(name, value)| format!("{name}={value}"))
			.collect()
	}
}

fn url_of(uri: &Uri) -> Option<Url> {
	Url::parse(&uri.to_string()).ok()
}

fn sensitive(text: &str) -> Option<HeaderValue> {
	let mut value = HeaderValue::from_str(text).ok()?;
	value.set_sensitive(true);
	Some(value)
}

impl CookieStore for CookieJar {
	fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, uri: &Uri) {
		let Some(url) = url_of(uri) else {
			return;
		};
		let cookies = cookie_headers
			.filter_map(|value| value.to_str().ok())
			.filter_map(|value| cookie_store::RawCookie::parse(value.to_owned()).ok())
			.collect();
		self.store_cookies(cookies, &url);
	}

	fn cookies(&self, uri: &Uri, version: Version) -> Cookies {
		let Some(url) = url_of(uri) else {
			return Cookies::Empty;
		};
		let pairs = self.request_pairs(&url);
		if pairs.is_empty() {
			return Cookies::Empty;
		}
		if version == Version::HTTP_2 {
			return Cookies::Uncompressed(pairs.iter().filter_map(|pair| sensitive(pair)).collect());
		}
		sensitive(&pairs.join("; ")).map_or(Cookies::Empty, Cookies::Compressed)
	}
}
