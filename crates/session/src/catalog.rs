use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, VerifyingKey};
use parking_lot::Mutex;
use reqwest::{Client, Response, redirect};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

const CATALOG_PUBLIC_KEY: Option<&str> = None;

const CATALOG_VERSION: u32 = 1;
const MAX_THEME_ID_LEN: usize = 64;
const MAX_THEME_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_THEME_BYTES: u64 = 256 * 1024;
const MAX_SIGNATURE_BYTES: usize = 1024;
const MAX_REDIRECTS: usize = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const SIGNATURE_SUFFIX: &str = ".sig";

fn catalog_key() -> Option<VerifyingKey> {
	let bytes: [u8; 32] = STANDARD.decode(CATALOG_PUBLIC_KEY?).ok()?.try_into().ok()?;
	VerifyingKey::from_bytes(&bytes).ok()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeEntry {
	pub id: String,
	pub name: String,
	pub version: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub author: String,
	pub url: String,
	pub sha256: String,
	pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeCatalog {
	pub version: u32,
	pub generated_at: String,
	#[serde(deserialize_with = "lenient_theme_entries")]
	pub themes: Vec<ThemeEntry>,
}

#[derive(Default)]
pub(crate) struct ThemeCatalogs {
	catalog: Mutex<Option<Arc<ThemeCatalog>>>,
	error: Mutex<Option<String>>,
}

impl ThemeCatalogs {
	pub(crate) fn catalog(&self) -> Option<Arc<ThemeCatalog>> {
		self.catalog.lock().clone()
	}

	pub(crate) fn error(&self) -> Option<String> {
		self.error.lock().clone()
	}

	pub(crate) fn record_fetch(&self, outcome: Result<ThemeCatalog>) {
		match outcome {
			Ok(catalog) => {
				*self.catalog.lock() = Some(Arc::new(catalog));
				*self.error.lock() = None;
			}
			Err(error) => self.record_error(Some(error)),
		}
	}

	pub(crate) fn record_error(&self, error: Option<anyhow::Error>) {
		*self.error.lock() = error.map(|error| format!("{error:#}"));
	}
}

fn is_valid_theme_id(id: &str) -> bool {
	(1..=MAX_THEME_ID_LEN).contains(&id.len())
		&& id
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn lenient_theme_entries<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ThemeEntry>, D::Error> {
	let mut entries: Vec<ThemeEntry> = lenient_entries(deserializer)?;
	entries.retain(|entry| is_valid_theme_id(&entry.id));
	Ok(entries)
}

fn lenient_entries<'de, T: DeserializeOwned, D: Deserializer<'de>>(
	deserializer: D,
) -> Result<Vec<T>, D::Error> {
	let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
	Ok(raw
		.into_iter()
		.filter_map(|value| serde_json::from_value(value).ok())
		.collect())
}

fn verify_signature(bytes: &[u8], signature_base64: &[u8], key: &VerifyingKey) -> Result<()> {
	let raw = STANDARD
		.decode(signature_base64.trim_ascii())
		.context("signature is not base64")?;
	let signature = Signature::from_slice(&raw).context("signature is not 64 bytes")?;
	key.verify_strict(bytes, &signature)
		.context("signature does not match the catalog")
}

fn verify_theme_catalog(bytes: &[u8], signature_base64: &[u8], key: &VerifyingKey) -> Result<ThemeCatalog> {
	verify_signature(bytes, signature_base64, key)?;
	let catalog: ThemeCatalog = serde_json::from_slice(bytes).context("catalog is not valid json")?;
	ensure!(catalog.version == CATALOG_VERSION, "unsupported catalog version");
	Ok(catalog)
}

fn client() -> Result<Client> {
	Client::builder()
		.https_only(true)
		.redirect(redirect::Policy::limited(MAX_REDIRECTS))
		.timeout(REQUEST_TIMEOUT)
		.user_agent(protocol::fingerprint::user_agent())
		.build()
		.context("could not build the http client")
}

async fn get(client: &Client, url: &str) -> Result<Response> {
	let response = client.get(url).send().await.context("request failed")?;
	ensure!(
		response.status().is_success(),
		"server answered {}",
		response.status()
	);
	Ok(response)
}

async fn get_limited(client: &Client, url: &str, limit: usize) -> Result<Vec<u8>> {
	let mut response = get(client, url).await?;
	let mut body = Vec::new();
	while let Some(chunk) = response.chunk().await.context("download interrupted")? {
		ensure!(
			body.len() + chunk.len() <= limit,
			"response is larger than {limit} bytes"
		);
		body.extend_from_slice(&chunk);
	}
	Ok(body)
}

async fn fetch_signed(url: &str, limit: usize) -> Result<(Vec<u8>, Vec<u8>)> {
	let client = client()?;
	let bytes = get_limited(&client, url, limit).await?;
	let signature = get_limited(&client, &format!("{url}{SIGNATURE_SUFFIX}"), MAX_SIGNATURE_BYTES).await?;
	Ok((bytes, signature))
}

pub(crate) async fn fetch_theme_catalog(url: &str) -> Result<ThemeCatalog> {
	let key = catalog_key().context("the catalog key is not configured")?;
	let (bytes, signature) = fetch_signed(url, MAX_THEME_CATALOG_BYTES).await?;
	verify_theme_catalog(&bytes, &signature, &key)
}

async fn download_theme(entry: &ThemeEntry) -> Result<Vec<u8>> {
	ensure!(
		entry.length > 0 && entry.length <= MAX_THEME_BYTES,
		"catalog length for {} is out of range",
		entry.id
	);
	let client = client()?;
	let bytes = get_limited(&client, &entry.url, MAX_THEME_BYTES as usize).await?;
	check_theme(entry, &bytes)?;
	Ok(bytes)
}

fn check_theme(entry: &ThemeEntry, bytes: &[u8]) -> Result<()> {
	ensure!(
		bytes.len() as u64 == entry.length,
		"download is {} bytes, the catalog says {}",
		bytes.len(),
		entry.length
	);
	ensure!(
		hex::encode(Sha256::digest(bytes)).eq_ignore_ascii_case(&entry.sha256),
		"download does not match the catalog hash"
	);
	let theme: serde_json::Value = serde_json::from_slice(bytes).context("theme is not valid json")?;
	ensure!(theme.is_object(), "theme is not a json object");
	Ok(())
}

fn write_theme(themes_dir: &Path, entry: &ThemeEntry, bytes: &[u8]) -> Result<()> {
	ensure!(
		is_valid_theme_id(&entry.id),
		"theme id {:?} is not allowed",
		entry.id
	);
	std::fs::create_dir_all(themes_dir).context("could not create the themes folder")?;
	store::write_atomic(&themes_dir.join(format!("{}.json", entry.id)), bytes)
		.context("could not write the theme")
}

pub(crate) async fn install_theme(
	catalog: Option<Arc<ThemeCatalog>>,
	themes_dir: PathBuf,
	id: &str,
) -> Result<()> {
	let entry = catalog
		.and_then(|catalog| catalog.themes.iter().find(|entry| entry.id == id).cloned())
		.context("theme is not in the catalog")?;
	let bytes = download_theme(&entry).await?;
	tokio::task::spawn_blocking(move || write_theme(&themes_dir, &entry, &bytes))
		.await
		.context("install task failed")?
}
