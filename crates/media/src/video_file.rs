use std::fs::{File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::{Condvar, Mutex};
use tokio::runtime::Handle;
use tokio::sync::Notify;

use crate::error::{MediaError, Result, redact_url};
use crate::fetch::{MediaFetcher, RangeBody};
use crate::queue::Ticket;

pub const VIDEO_MAX_BYTES: u64 = 512 * 1024 * 1024;
pub const READ_AHEAD: u64 = 8 * 1024 * 1024;
const RESTART_GAP: u64 = 512 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const FOLDER: &str = "kyoko-video";
#[cfg(windows)]
const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoLimits {
	pub max_bytes: u64,
	pub read_ahead: u64,
}

impl Default for VideoLimits {
	fn default() -> Self {
		Self {
			max_bytes: VIDEO_MAX_BYTES,
			read_ahead: READ_AHEAD,
		}
	}
}

#[derive(Default)]
struct State {
	known: bool,
	length: Option<u64>,
	filled: Vec<(u64, u64)>,
	read_to: u64,
	waiting: Option<u64>,
	failure: Option<MediaError>,
	closed: bool,
	requests: u32,
}

impl State {
	fn filled_end(&self, position: u64) -> Option<u64> {
		self.filled
			.iter()
			.find(|(start, end)| *start <= position && position < *end)
			.map(|(_, end)| *end)
	}

	fn first_gap(&self, from: u64) -> u64 {
		let mut gap = from;
		while let Some(end) = self.filled_end(gap) {
			gap = end;
		}
		gap
	}

	fn fill(&mut self, start: u64, end: u64) {
		if start >= end {
			return;
		}
		self.filled.push((start, end));
		self.filled.sort_unstable();
		let mut merged: Vec<(u64, u64)> = Vec::with_capacity(self.filled.len());
		for (start, end) in self.filled.drain(..) {
			match merged.last_mut() {
				Some(last) if start <= last.1 => last.1 = last.1.max(end),
				_ => merged.push((start, end)),
			}
		}
		self.filled = merged;
	}

	fn ended(&self, position: u64) -> bool {
		self.length.is_some_and(|length| position >= length)
	}

	fn next_start(&self, read_ahead: u64) -> Option<u64> {
		if let Some(waiting) = self.waiting
			&& self.filled_end(waiting).is_none()
			&& !self.ended(waiting)
		{
			return Some(waiting);
		}
		let gap = self.first_gap(self.read_to);
		(!self.ended(gap) && gap < self.read_to.saturating_add(read_ahead)).then_some(gap)
	}
}

struct Inner {
	state: Mutex<State>,
	changed: Condvar,
	wake: Notify,
	file: File,
	limits: VideoLimits,
}

#[derive(Clone)]
pub struct VideoFile {
	inner: Arc<Inner>,
	_closer: Arc<Closer>,
}

struct Closer(Arc<Inner>);

impl Drop for Closer {
	fn drop(&mut self) {
		self.0.state.lock().closed = true;
		self.0.changed.notify_all();
		self.0.wake.notify_one();
	}
}

impl VideoFile {
	pub fn open(
		fetcher: MediaFetcher,
		runtime: &Handle,
		url: &str,
		ticket: Arc<Ticket>,
		limits: VideoLimits,
	) -> Result<Self> {
		let inner = Arc::new(Inner {
			state: Mutex::new(State::default()),
			changed: Condvar::new(),
			wake: Notify::new(),
			file: scratch_file()?,
			limits,
		});
		let task = inner.clone();
		let url = url.to_owned();
		runtime.spawn(async move { task.download(fetcher, url, ticket).await });
		Ok(Self {
			_closer: Arc::new(Closer(inner.clone())),
			inner,
		})
	}

	pub fn length(&self) -> io::Result<Option<u64>> {
		let mut state = self.inner.state.lock();
		while !state.known {
			if let Some(failure) = &state.failure {
				return Err(io::Error::other(failure.to_string()));
			}
			if self.inner.changed.wait_for(&mut state, READ_TIMEOUT).timed_out() {
				return Err(io::ErrorKind::TimedOut.into());
			}
		}
		Ok(state.length)
	}

	pub fn read_at(&self, position: u64, buffer: &mut [u8]) -> io::Result<usize> {
		if buffer.is_empty() {
			return Ok(0);
		}
		let mut state = self.inner.state.lock();
		loop {
			if let Some(failure) = &state.failure {
				return Err(io::Error::other(failure.to_string()));
			}
			if state.closed {
				return Err(io::ErrorKind::BrokenPipe.into());
			}
			if state.ended(position) {
				return Ok(0);
			}
			if let Some(end) = state.filled_end(position) {
				let count = (end - position).min(buffer.len() as u64) as usize;
				state.read_to = position + count as u64;
				if state.waiting == Some(position) {
					state.waiting = None;
				}
				drop(state);
				self.inner.wake.notify_one();
				return read_exact_at(&self.inner.file, position, &mut buffer[..count]).map(|()| count);
			}
			state.waiting = Some(position);
			self.inner.wake.notify_one();
			if self.inner.changed.wait_for(&mut state, READ_TIMEOUT).timed_out() {
				return Err(io::ErrorKind::TimedOut.into());
			}
		}
	}

	pub fn requests(&self) -> u32 {
		self.inner.state.lock().requests
	}

	pub fn failure(&self) -> Option<MediaError> {
		self.inner.state.lock().failure.clone()
	}

	pub fn downloaded(&self) -> u64 {
		let state = self.inner.state.lock();
		state.filled.iter().map(|(start, end)| end - start).sum()
	}
}

impl Inner {
	async fn download(self: Arc<Self>, fetcher: MediaFetcher, url: String, ticket: Arc<Ticket>) {
		loop {
			let woken = self.wake.notified();
			let start = {
				let state = self.state.lock();
				if state.closed || state.failure.is_some() {
					return;
				}
				state.next_start(self.limits.read_ahead)
			};
			let Some(start) = start else {
				woken.await;
				continue;
			};
			self.state.lock().requests += 1;
			let fetched = match fetcher.open_range(&url, start, ticket.clone()).await {
				Ok(body) => self.receive(body).await,
				Err(error) => Err(error),
			};
			if let Err(error) = fetched {
				tracing::debug!(url = redact_url(&url), %error, "a video download failed");
				self.state.lock().failure = Some(error);
				self.changed.notify_all();
				return;
			}
		}
	}

	async fn receive(&self, mut body: RangeBody) -> Result<()> {
		let cap = self.limits.max_bytes;
		{
			let mut state = self.state.lock();
			if !state.known {
				if body.length().is_some_and(|length| length > cap) {
					return Err(MediaError::Limit(format!("the video is over {cap} bytes")));
				}
				state.known = true;
				state.length = body.length();
				self.changed.notify_all();
			}
		}
		let mut position = body.start();
		while let Some(chunk) = body.chunk().await? {
			let end = position + chunk.len() as u64;
			if end > cap {
				return Err(MediaError::Limit(format!("the video is over {cap} bytes")));
			}
			write_all_at(&self.file, position, &chunk)?;
			let mut state = self.state.lock();
			state.fill(position, end);
			position = end;
			self.changed.notify_all();
			if state.closed {
				return Ok(());
			}
			let wanted_elsewhere = state.waiting.is_some_and(|waiting| {
				state.filled_end(waiting).is_none() && !(position..position + RESTART_GAP).contains(&waiting)
			});
			let far_enough = position >= state.read_to.saturating_add(self.limits.read_ahead);
			if wanted_elsewhere || far_enough || state.filled_end(position).is_some() || state.ended(position)
			{
				return Ok(());
			}
		}
		let mut state = self.state.lock();
		if state.length.is_none() {
			state.length = Some(position);
			self.changed.notify_all();
		}
		Ok(())
	}
}

fn scratch_file() -> io::Result<File> {
	let folder = std::env::temp_dir().join(FOLDER);
	std::fs::create_dir_all(&folder)?;
	let stamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |since| since.as_nanos());
	let path: PathBuf = folder.join(format!(
		"{}-{stamp:x}-{}.part",
		std::process::id(),
		NEXT_FILE.fetch_add(1, Ordering::Relaxed)
	));
	let mut options = OpenOptions::new();
	options.read(true).write(true).create_new(true);
	#[cfg(windows)]
	{
		use std::os::windows::fs::OpenOptionsExt;
		options.custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
	}
	let file = options.open(&path)?;
	#[cfg(not(windows))]
	std::fs::remove_file(&path)?;
	Ok(file)
}

#[cfg(windows)]
fn write_all_at(file: &File, mut position: u64, mut data: &[u8]) -> io::Result<()> {
	use std::os::windows::fs::FileExt;
	while !data.is_empty() {
		let written = file.seek_write(data, position)?;
		if written == 0 {
			return Err(io::ErrorKind::WriteZero.into());
		}
		data = &data[written..];
		position += written as u64;
	}
	Ok(())
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut position: u64, mut buffer: &mut [u8]) -> io::Result<()> {
	use std::os::windows::fs::FileExt;
	while !buffer.is_empty() {
		let read = file.seek_read(buffer, position)?;
		if read == 0 {
			return Err(io::ErrorKind::UnexpectedEof.into());
		}
		buffer = &mut buffer[read..];
		position += read as u64;
	}
	Ok(())
}

#[cfg(unix)]
fn write_all_at(file: &File, position: u64, data: &[u8]) -> io::Result<()> {
	use std::os::unix::fs::FileExt;
	file.write_all_at(data, position)
}

#[cfg(unix)]
fn read_exact_at(file: &File, position: u64, buffer: &mut [u8]) -> io::Result<()> {
	use std::os::unix::fs::FileExt;
	file.read_exact_at(buffer, position)
}
