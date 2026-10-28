use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use futures_util::future::{BoxFuture, Shared};
use futures_util::{FutureExt, StreamExt};
use net::{Destination, Trust};
use parking_lot::Mutex;
use url::Url;
use wreq::header::{HeaderMap, HeaderName, HeaderValue};
use wreq::redirect::Policy;

use crate::error::{MediaError, Result, redact_url};
use crate::queue::{Gate, PerHost, Protocol, Ticket};

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
const POOL_IDLE: Duration = Duration::from_secs(90);
const HTTPS_PORT: u16 = 443;
const LOOPBACK_HOST: &str = "127.0.0.1";

type SharedFetch = Shared<BoxFuture<'static, Result<Bytes>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Purpose {
	Image,
	Audio,
}

pub struct MediaFetcherBuilder {
	hosts: HashSet<String>,
	max_bytes: usize,
	loopback_port: Option<u16>,
	user_agent: String,
	headers: Vec<(String, String)>,
	audio_headers: Vec<(String, String)>,
	per_host: PerHost,
	loopback_http2: bool,
	trust: Trust,
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

	pub fn only_loopback(mut self, port: u16) -> Self {
		self.hosts.clear();
		self.allow_loopback_http(port)
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
		self.headers = owned(headers);
		self
	}

	pub fn audio_headers<N, V>(mut self, headers: impl IntoIterator<Item = (N, V)>) -> Self
	where
		N: Into<String>,
		V: Into<String>,
	{
		self.audio_headers = owned(headers);
		self
	}

	pub fn per_host(mut self, http1: usize, http2: usize) -> Self {
		self.per_host = PerHost {
			http1: http1.max(1),
			http2: http2.max(1),
		};
		self
	}

	pub fn loopback_http2(mut self) -> Self {
		self.loopback_http2 = true;
		self
	}

	pub fn trust(mut self, trust: Trust) -> Self {
		self.trust = trust;
		self
	}

	pub fn build(self) -> Result<MediaFetcher> {
		let only_loopback = self.hosts.is_empty();
		let hosts = Arc::new(Allowed {
			hosts: self.hosts,
			loopback_port: self.loopback_port,
		});
		let redirect_hosts = hosts.clone();
		let policy = Policy::custom(move |attempt| {
			if attempt.previous.len() > MAX_REDIRECTS {
				return attempt.error("too many redirects");
			}
			let allowed =
				Url::parse(&attempt.uri.to_string()).is_ok_and(|target| is_allowed(&redirect_hosts, &target));
			if allowed {
				attempt.follow()
			} else {
				attempt.error("redirect to a disallowed host")
			}
		});
		let mut client = net::client_builder(Destination::Image, &self.trust)
			.map_err(|error| MediaError::Fetch(error.without_uri().to_string()))?
			.https_only(self.loopback_port.is_none())
			.timeout(REQUEST_TIMEOUT)
			.pool_idle_timeout(POOL_IDLE)
			.redirect(policy)
			.referer(false)
			.user_agent(self.user_agent);
		if self.loopback_http2 && only_loopback {
			client = client.http2_only();
		}
		let client = client
			.build()
			.map_err(|error| MediaError::Fetch(error.without_uri().to_string()))?;
		Ok(MediaFetcher {
			inner: Arc::new(FetcherInner {
				client,
				image_headers: header_map(&self.headers),
				audio_headers: header_map(&self.audio_headers),
				hosts,
				max_bytes: self.max_bytes,
				inflight: Mutex::new(HashMap::new()),
				gate: Gate::new(self.per_host, POOL_IDLE),
			}),
		})
	}
}

#[derive(Clone)]
pub struct MediaFetcher {
	inner: Arc<FetcherInner>,
}

struct FetcherInner {
	client: wreq::Client,
	image_headers: HeaderMap,
	audio_headers: HeaderMap,
	hosts: Arc<Allowed>,
	max_bytes: usize,
	inflight: Mutex<HashMap<(Purpose, String), SharedFetch>>,
	gate: Arc<Gate>,
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
			audio_headers: Vec::new(),
			per_host: PerHost::default(),
			loopback_http2: false,
			trust: Trust::Bundled,
		}
	}

	pub fn new() -> Result<Self> {
		Self::builder().build()
	}

	pub async fn fetch(&self, url: &str) -> Result<Bytes> {
		self.fetch_for(url, Purpose::Image).await
	}

	pub async fn fetch_for(&self, url: &str, purpose: Purpose) -> Result<Bytes> {
		self.fetch_queued(url, purpose, Ticket::urgent()).await
	}

	pub async fn fetch_queued(&self, url: &str, purpose: Purpose, ticket: Arc<Ticket>) -> Result<Bytes> {
		let parsed = self.inner.validate(url)?;
		let key = (purpose, url.to_owned());
		let shared = {
			let mut inflight = self.inner.inflight.lock();
			match inflight.get(&key) {
				Some(existing) => existing.clone(),
				None => {
					let inner = self.inner.clone();
					let owned = key.clone();
					let shared = async move {
						let result = inner.download(parsed, purpose, ticket).await;
						inner.inflight.lock().remove(&owned);
						result
					}
					.boxed()
					.shared();
					inflight.insert(key, shared.clone());
					shared
				}
			}
		};
		shared.await
	}

	pub fn reschedule(&self) {
		self.inner.gate.reschedule();
	}

	pub fn waiting(&self) -> usize {
		self.inner.gate.waiting()
	}

	pub fn protocol(&self, url: &str) -> Protocol {
		Url::parse(url).map_or(Protocol::Unknown, |parsed| {
			self.inner.gate.protocol(&host_key(&parsed))
		})
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

	async fn download(&self, url: Url, purpose: Purpose, ticket: Arc<Ticket>) -> Result<Bytes> {
		let host = host_key(&url);
		let _slot = self.gate.acquire(&host, ticket).await?;
		let shown = redact_url(url.as_str()).to_owned();
		let response = self
			.client
			.get(url.as_str())
			.headers(match purpose {
				Purpose::Image => self.image_headers.clone(),
				Purpose::Audio => self.audio_headers.clone(),
			})
			.send()
			.await
			.map_err(|error| MediaError::Fetch(format!("{shown}: {}", error.without_uri())))?;
		if Url::parse(&response.uri().to_string()).is_ok_and(|landed| host_key(&landed) == host) {
			self.gate.learn(&host, response.version());
		}
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
		let mut chunks = response.bytes_stream();
		while let Some(chunk) = chunks.next().await {
			let chunk =
				chunk.map_err(|error| MediaError::Fetch(format!("{shown}: {}", error.without_uri())))?;
			if body.len() + chunk.len() > cap {
				return Err(MediaError::Limit(format!("{shown}: body exceeds {cap} bytes")));
			}
			body.extend_from_slice(&chunk);
		}
		Ok(body.freeze())
	}
}

fn host_key(url: &Url) -> String {
	format!(
		"{}://{}:{}",
		url.scheme(),
		url.host_str().unwrap_or_default(),
		url.port_or_known_default().unwrap_or_default()
	)
}

fn owned<N, V>(headers: impl IntoIterator<Item = (N, V)>) -> Vec<(String, String)>
where
	N: Into<String>,
	V: Into<String>,
{
	headers
		.into_iter()
		.map(|(name, value)| (name.into(), value.into()))
		.collect()
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
	let Some(host) = url.host_str() else {
		return false;
	};
	url.scheme() == "https"
		&& match url.port() {
			None | Some(HTTPS_PORT) => allowed.hosts.contains(host),
			Some(port) => allowed.hosts.contains(&format!("{host}:{port}")),
		}
}
