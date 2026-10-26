use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;

use crate::cache::{DiskCache, SizeHint};
use crate::decode::{Limits, decode};
use crate::error::{MediaError, Result, redact_url};
use crate::fetch::MediaFetcher;
use crate::resize::resize_decoded;
use crate::shape::Shape;
use crate::types::Decoded;

pub const DEFAULT_DECODE_WORKERS: usize = 2;
const SHRINK_THRESHOLD: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Variant {
	pub hint: SizeHint,
	pub shape: Shape,
}

impl Variant {
	pub const fn new(hint: SizeHint, shape: Shape) -> Self {
		Self { hint, shape }
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
	Anywhere,
	DiskOnly,
}

#[derive(Clone)]
pub struct MediaService {
	inner: Arc<ServiceInner>,
}

struct ServiceInner {
	fetcher: MediaFetcher,
	disk: Arc<DiskCache>,
	limits: Limits,
	decoders: Semaphore,
}

impl MediaService {
	pub fn new(fetcher: MediaFetcher, disk_dir: impl Into<PathBuf>, limits: Limits) -> Self {
		Self::with_decoders(fetcher, disk_dir, limits, DEFAULT_DECODE_WORKERS)
	}

	pub fn with_decoders(
		fetcher: MediaFetcher,
		disk_dir: impl Into<PathBuf>,
		limits: Limits,
		workers: usize,
	) -> Self {
		Self {
			inner: Arc::new(ServiceInner {
				fetcher,
				disk: Arc::new(DiskCache::new(disk_dir.into())),
				limits,
				decoders: Semaphore::new(workers.max(1)),
			}),
		}
	}

	pub fn disk(&self) -> &DiskCache {
		&self.inner.disk
	}

	pub async fn load(&self, url: &str, hint: SizeHint) -> Result<Decoded> {
		self.load_variant(url, Variant::new(hint, Shape::Square), Source::Anywhere)
			.await?
			.ok_or_else(|| MediaError::Fetch(format!("{} was not loaded", redact_url(url))))
	}

	pub async fn bytes(&self, url: &str) -> Result<Bytes> {
		if let Some(bytes) = self.cached_bytes(url).await {
			return Ok(bytes);
		}
		let bytes = self.inner.fetcher.fetch(url).await?;
		self.store_on_disk(url, bytes.clone()).await;
		Ok(bytes)
	}

	pub async fn load_variant(&self, url: &str, variant: Variant, source: Source) -> Result<Option<Decoded>> {
		let (bytes, from_disk) = match (self.cached_bytes(url).await, source) {
			(Some(bytes), _) => (bytes, true),
			(None, Source::DiskOnly) => return Ok(None),
			(None, Source::Anywhere) => (self.inner.fetcher.fetch(url).await?, false),
		};
		let decoded = self.decode(bytes.clone(), variant).await?;
		if !from_disk {
			self.store_on_disk(url, bytes).await;
		}
		Ok(Some(decoded))
	}

	async fn decode(&self, bytes: Bytes, variant: Variant) -> Result<Decoded> {
		let _permit = self
			.inner
			.decoders
			.acquire()
			.await
			.map_err(|_| MediaError::Decode("the decoders closed".to_owned()))?;
		let limits = self.inner.limits.clone();
		spawn_blocking(move || {
			decode(&bytes, &limits).map(|decoded| {
				let mut decoded = shrink_to_hint(decoded, variant.hint);
				variant.shape.apply(&mut decoded);
				decoded
			})
		})
		.await
		.map_err(|error| MediaError::Decode(format!("decode task failed: {error}")))?
	}

	async fn cached_bytes(&self, url: &str) -> Option<Bytes> {
		let disk = self.inner.disk.clone();
		let owned = url.to_owned();
		spawn_blocking(move || disk.get(&owned))
			.await
			.ok()
			.and_then(|result| result.ok())
			.flatten()
	}

	async fn store_on_disk(&self, url: &str, bytes: Bytes) {
		let disk = self.inner.disk.clone();
		let owned = url.to_owned();
		let stored = spawn_blocking(move || disk.put(&owned, &bytes)).await;
		if let Ok(Err(error)) = stored {
			tracing::debug!(url = redact_url(url), %error, "media disk write failed");
		}
	}
}

fn shrink_to_hint(decoded: Decoded, hint: SizeHint) -> Decoded {
	if hint.is_original() {
		return decoded;
	}
	let (width, height) = decoded.dimensions();
	if width > hint.width.saturating_mul(SHRINK_THRESHOLD)
		|| height > hint.height.saturating_mul(SHRINK_THRESHOLD)
	{
		resize_decoded(decoded, hint.width, hint.height)
	} else {
		decoded
	}
}
