use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use futures_util::FutureExt;
use futures_util::future::{BoxFuture, Shared};
use parking_lot::Mutex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use tokio::sync::Semaphore;
use url::Url;

use crate::error::{MediaError, Result, redact_url};

pub const DEFAULT_ALLOWED_HOSTS: [&str; 8] = [
	"cdn.discordapp.com",
	"media.discordapp.net",
	"images-ext-1.discordapp.net",
	"images-ext-2.discordapp.net",
	"media.tenor.com",
	"c.tenor.com",
	"i.scdn.co",
	"static.klipy.com",
];

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_CONCURRENCY: usize = 6;
const HTTPS_PORT: u16 = 443;
const LOOPBACK_HOST: &str = "127.0.0.1";

type SharedFetch = Shared<BoxFuture<'static, Result<Bytes>>>;

pub struct MediaFetcherBuilder {
	hosts: HashSet<String>,
	max_bytes: usize,
	loopback_port: Option<u16>,
	user_agent: String,
	headers: Vec<(String, String)>,
	concurrency: usize,
}

impl MediaFetcherBuilder {
	pub fn allow_host(mut self, host: impl Into<String>) -> Self {
		self.hosts.insert(host.into().to_ascii_lowercase());
		self
	}

	pub fn max_bytes(mut self, max_bytes: usize) -> Self {
		self.max_bytes = max_bytes;
		self
	}

	pub fn allow_loopback_http(mut self, port: u16) -> Self {
		self.loopback_port = Some(port);
		self
	}

	pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
		self.user_agent = user_agent.into();
		self
	}

	pub fn headers<N, V>(mut self, headers: impl IntoIterator<Item = (N, V)>) -> Self
	where
		N: Into<String>,
		V: Into<String>,
	{
		self.headers = headers
			.into_iter()
			.map(|(name, value)| (name.into(), value.into()))
			.collect();
		self
	}

	pub fn concurrency(mut self, requests: usize) -> Self {
		self.concurrency = requests.max(1);
		self
	}

	pub fn build(self) -> Result<MediaFetcher> {
		let hosts = Arc::new(Allowed {
			hosts: self.hosts,
			loopback_port: self.loopback_port,
		});
		let redirect_hosts = hosts.clone();
		let policy = Policy::custom(move |attempt| {
			if attempt.previous().len() >= MAX_REDIRECTS {
				return attempt.error("too many redirects");
			}
			if is_allowed(&redirect_hosts, attempt.url()) {
				attempt.follow()
			} else {
				attempt.error("redirect to a disallowed host")
			}
		});
		let client = reqwest::Client::builder()
			.https_only(self.loopback_port.is_none())
			.timeout(REQUEST_TIMEOUT)
			.redirect(policy)
			.referer(false)
			.user_agent(self.user_agent)
			.default_headers(header_map(&self.headers))
			.build()
			.map_err(|error| MediaError::Fetch(error.without_url().to_string()))?;
		Ok(MediaFetcher {
			inner: Arc::new(FetcherInner {
				client,
				hosts,
				max_bytes: self.max_bytes,
				inflight: Mutex::new(HashMap::new()),
				permits: Semaphore::new(self.concurrency),
			}),
		})
	}
}

#[derive(Clone)]
pub struct MediaFetcher {
	inner: Arc<FetcherInner>,
}

struct FetcherInner {
	client: reqwest::Client,
	hosts: Arc<Allowed>,
	max_bytes: usize,
	inflight: Mutex<HashMap<String, SharedFetch>>,
	permits: Semaphore,
}

impl MediaFetcher {
	pub fn builder() -> MediaFetcherBuilder {
		MediaFetcherBuilder {
			hosts: DEFAULT_ALLOWED_HOSTS
				.iter()
				.map(|host| (*host).to_owned())
				.collect(),
			max_bytes: DEFAULT_MAX_BYTES,
			loopback_port: None,
			user_agent: DEFAULT_USER_AGENT.to_owned(),
			headers: Vec::new(),
			concurrency: DEFAULT_CONCURRENCY,
		}
	}

	pub fn new() -> Result<Self> {
		Self::builder().build()
	}

	pub async fn fetch(&self, url: &str) -> Result<Bytes> {
		let parsed = self.inner.validate(url)?;
		let shared = {
			let mut inflight = self.inner.inflight.lock();
			match inflight.get(url) {
				Some(existing) => existing.clone(),
				None => {
					let inner = self.inner.clone();
					let key = url.to_owned();
					let shared = async move {
						let result = inner.download(parsed).await;
						inner.inflight.lock().remove(&key);
						result
					}
					.boxed()
					.shared();
					inflight.insert(url.to_owned(), shared.clone());
					shared
				}
			}
		};
		shared.await
	}
}

impl FetcherInner {
	fn validate(&self, url: &str) -> Result<Url> {
		let parsed =
			Url::parse(url).map_err(|_| MediaError::Fetch(format!("invalid url {}", redact_url(url))))?;
		if is_allowed(&self.hosts, &parsed) {
			Ok(parsed)
		} else {
			Err(MediaError::DisallowedHost(redact_url(url).to_owned()))
		}
	}

	async fn download(&self, url: Url) -> Result<Bytes> {
		let _permit = self
			.permits
			.acquire()
			.await
			.map_err(|_| MediaError::Fetch("the fetcher closed".to_owned()))?;
		let shown = redact_url(url.as_str()).to_owned();
		let mut response = self
			.client
			.get(url)
			.send()
			.await
			.map_err(|error| MediaError::Fetch(format!("{shown}: {}", error.without_url())))?;
		if !response.status().is_success() {
			return Err(MediaError::Status(response.status().as_u16()));
		}
		let cap = self.max_bytes;
		let declared = response
			.content_length()
			.and_then(|length| usize::try_from(length).ok());
		if declared.is_some_and(|length| length > cap) {
			return Err(MediaError::Limit(format!("{shown}: body exceeds {cap} bytes")));
		}
		let mut body = BytesMut::with_capacity(declared.unwrap_or(0).min(cap));
		while let Some(chunk) = response
			.chunk()
			.await
			.map_err(|error| MediaError::Fetch(format!("{shown}: {}", error.without_url())))?
		{
			if body.len() + chunk.len() > cap {
				return Err(MediaError::Limit(format!("{shown}: body exceeds {cap} bytes")));
			}
			body.extend_from_slice(&chunk);
		}
		Ok(body.freeze())
	}
}

fn header_map(headers: &[(String, String)]) -> HeaderMap {
	headers
		.iter()
		.filter_map(|(name, value)| {
			Some((
				HeaderName::from_bytes(name.as_bytes()).ok()?,
				HeaderValue::from_str(value).ok()?,
			))
		})
		.collect()
}

struct Allowed {
	hosts: HashSet<String>,
	loopback_port: Option<u16>,
}

fn is_allowed(allowed: &Allowed, url: &Url) -> bool {
	if !url.username().is_empty() || url.password().is_some() {
		return false;
	}
	if url.scheme() == "http" {
		return allowed.loopback_port.is_some()
			&& url.host_str() == Some(LOOPBACK_HOST)
			&& url.port() == allowed.loopback_port;
	}
	url.scheme() == "https"
		&& url.port().is_none_or(|port| port == HTTPS_PORT)
		&& url.host_str().is_some_and(|host| allowed.hosts.contains(host))
}
