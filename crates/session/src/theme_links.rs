use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use parking_lot::Mutex;
use reqwest::{Certificate, Client, redirect};
use sha2::{Digest, Sha256};
use tokio::task::JoinSet;
use url::Url;

pub const MAX_THEME_BYTES: usize = 512 * 1024;
pub const MAX_THEME_LINKS: usize = 64;
const MAX_LINK_LENGTH: usize = 2048;
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REDIRECTS: usize = 3;
const CACHE_NAME_LENGTH: usize = 32;
const CACHE_EXTENSION: &str = "theme";

#[derive(Debug, Clone)]
pub struct ThemeLinkTuning {
	pub max_bytes: usize,
	pub timeout: Duration,
	pub trusted_roots: Vec<Vec<u8>>,
}

impl Default for ThemeLinkTuning {
	fn default() -> Self {
		Self {
			max_bytes: MAX_THEME_BYTES,
			timeout: FETCH_TIMEOUT,
			trusted_roots: Vec::new(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLinkCopy {
	pub url: String,
	pub text: Option<String>,
	pub error: Option<String>,
	pub cached: bool,
}

#[derive(Default)]
pub(crate) struct ThemeLinks {
	copies: Mutex<Arc<Vec<ThemeLinkCopy>>>,
	started: AtomicU64,
}

impl ThemeLinks {
	pub(crate) fn copies(&self) -> Arc<Vec<ThemeLinkCopy>> {
		self.copies.lock().clone()
	}

	pub(crate) fn begin(&self) -> u64 {
		self.started.fetch_add(1, Ordering::AcqRel) + 1
	}

	pub(crate) fn finish(&self, generation: u64, copies: Vec<ThemeLinkCopy>) -> bool {
		if self.started.load(Ordering::Acquire) != generation {
			return false;
		}
		*self.copies.lock() = Arc::new(copies);
		true
	}
}

pub fn unique_links(urls: &[String]) -> Vec<String> {
	let mut unique: Vec<String> = Vec::new();
	for url in urls.iter().map(|url| url.trim()).filter(|url| !url.is_empty()) {
		if !unique.iter().any(|seen| seen == url) {
			unique.push(url.to_owned());
		}
	}
	unique.truncate(MAX_THEME_LINKS);
	unique
}

pub fn cached_copies(cache_dir: &Path, urls: &[String]) -> Vec<ThemeLinkCopy> {
	unique_links(urls)
		.into_iter()
		.map(|url| {
			let text = read_cache(cache_dir, &url);
			ThemeLinkCopy {
				cached: text.is_some(),
				url,
				text,
				error: None,
			}
		})
		.collect()
}

pub(crate) async fn fetch_all(
	tuning: &ThemeLinkTuning,
	cache_dir: &Path,
	urls: &[String],
) -> Vec<ThemeLinkCopy> {
	let urls = unique_links(urls);
	let client = client(tuning).map(Arc::new);
	let mut tasks = JoinSet::new();
	for (index, url) in urls.iter().cloned().enumerate() {
		let client = client
			.as_ref()
			.map(Arc::clone)
			.map_err(|error| anyhow!("{error:#}"));
		let cache_dir = cache_dir.to_path_buf();
		let limit = tuning.max_bytes;
		tasks.spawn(async move {
			let outcome = match client {
				Ok(client) => fetch_one(&client, &url, limit).await,
				Err(error) => Err(error),
			};
			(index, settle(&cache_dir, url, outcome).await)
		});
	}
	let mut copies: Vec<Option<ThemeLinkCopy>> = vec![None; urls.len()];
	while let Some(joined) = tasks.join_next().await {
		if let Ok((index, copy)) = joined
			&& let Some(slot) = copies.get_mut(index)
		{
			*slot = Some(copy);
		}
	}
	copies
		.into_iter()
		.zip(urls)
		.map(|(copy, url)| {
			copy.unwrap_or(ThemeLinkCopy {
				url,
				text: None,
				error: Some("the download stopped unexpectedly".to_owned()),
				cached: false,
			})
		})
		.collect()
}

async fn settle(cache_dir: &Path, url: String, outcome: Result<String>) -> ThemeLinkCopy {
	match outcome {
		Ok(text) => {
			let written = write_cache(cache_dir.to_path_buf(), url.clone(), text.clone()).await;
			if let Err(error) = written {
				tracing::warn!(error = %format!("{error:#}"), "a theme link could not be cached");
			}
			ThemeLinkCopy {
				url,
				text: Some(text),
				error: None,
				cached: false,
			}
		}
		Err(error) => {
			let text = read_cache(cache_dir, &url);
			ThemeLinkCopy {
				cached: text.is_some(),
				url,
				text,
				error: Some(format!("{error:#}")),
			}
		}
	}
}

fn client(tuning: &ThemeLinkTuning) -> Result<Client> {
	let mut builder = Client::builder()
		.https_only(true)
		.redirect(redirect::Policy::limited(MAX_REDIRECTS))
		.timeout(tuning.timeout)
		.user_agent(protocol::fingerprint::user_agent());
	for root in &tuning.trusted_roots {
		builder = builder
			.add_root_certificate(Certificate::from_pem(root).context("a trusted root is not valid pem")?);
	}
	builder.build().context("could not build the http client")
}

pub fn check_link(url: &str) -> Result<Url> {
	ensure!(url.len() <= MAX_LINK_LENGTH, "the link is too long");
	let parsed = Url::parse(url).context("this is not a link")?;
	ensure!(parsed.scheme() == "https", "only https links are allowed");
	ensure!(parsed.host_str().is_some(), "the link has no host");
	ensure!(
		parsed.username().is_empty() && parsed.password().is_none(),
		"links with a user name or password are not allowed"
	);
	Ok(parsed)
}

async fn fetch_one(client: &Client, url: &str, limit: usize) -> Result<String> {
	let parsed = check_link(url)?;
	let mut response = client.get(parsed).send().await.map_err(describe)?;
	let status = response.status();
	ensure!(status.is_success(), "the server answered {status}");
	if let Some(length) = response.content_length() {
		ensure!(
			length as usize <= limit,
			"the file is larger than {} KB",
			limit / 1024
		);
	}
	let mut body = Vec::new();
	while let Some(chunk) = response.chunk().await.map_err(describe)? {
		ensure!(
			body.len() + chunk.len() <= limit,
			"the file is larger than {} KB",
			limit / 1024
		);
		body.extend_from_slice(&chunk);
	}
	let text = String::from_utf8(body).context("the file is not text")?;
	ensure!(
		!looks_like_a_page(&text),
		"the link is a web page; use a direct link to the theme file"
	);
	Ok(text)
}

fn describe(error: reqwest::Error) -> anyhow::Error {
	if error.is_timeout() {
		return anyhow!("the server took too long to answer");
	}
	if error.is_redirect() {
		return anyhow!("the link redirected too often or away from https");
	}
	if error.is_connect() {
		return anyhow!("could not connect to the server");
	}
	anyhow!("the download failed")
}

fn looks_like_a_page(text: &str) -> bool {
	let start = text.trim_start_matches('\u{feff}').trim_start();
	let head: String = start.chars().take(15).collect::<String>().to_ascii_lowercase();
	head.starts_with("<!doctype") || head.starts_with("<html")
}

fn cache_path(cache_dir: &Path, url: &str) -> PathBuf {
	let digest = hex::encode(Sha256::digest(url.as_bytes()));
	cache_dir.join(format!("{}.{CACHE_EXTENSION}", &digest[..CACHE_NAME_LENGTH]))
}

fn read_cache(cache_dir: &Path, url: &str) -> Option<String> {
	let text = std::fs::read_to_string(cache_path(cache_dir, url)).ok()?;
	(text.len() <= MAX_THEME_BYTES).then_some(text)
}

async fn write_cache(cache_dir: PathBuf, url: String, text: String) -> Result<()> {
	tokio::task::spawn_blocking(move || {
		std::fs::create_dir_all(&cache_dir).context("could not create the theme cache folder")?;
		store::write_atomic(&cache_path(&cache_dir, &url), text.as_bytes())
			.context("could not write the theme cache")
	})
	.await
	.context("the cache task failed")?
}
