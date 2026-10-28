use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, S_OK};
use windows::Win32::Media::MediaFoundation::{
	IMFAsyncCallback, IMFAsyncResult, IMFByteStream, IMFByteStream_Impl, MFBYTESTREAM_IS_READABLE,
	MFBYTESTREAM_IS_SEEKABLE, MFBYTESTREAM_SEEK_ORIGIN, MFCreateAsyncResult, MFInvokeCallback, msoBegin,
	msoCurrent,
};
use windows::core::{BOOL, IUnknown, Interface, Ref, Result, implement};

use crate::file::ByteSource;

const UNKNOWN_LENGTH: u64 = u64::MAX;

struct Pending(IMFAsyncResult);

unsafe impl Send for Pending {}

struct Target(*mut u8, usize);

unsafe impl Send for Target {}

struct Job {
	target: Target,
	position: u64,
	result: Pending,
}

struct Shared {
	source: Arc<dyn ByteSource>,
	position: Mutex<u64>,
	finished: Mutex<HashMap<usize, Option<u32>>>,
}

impl Shared {
	fn read(&self, position: u64, target: &mut [u8]) -> Option<u32> {
		let mut filled = 0;
		while filled < target.len() {
			match self
				.source
				.read_at(position + filled as u64, &mut target[filled..])
			{
				Ok(0) => break,
				Ok(read) => filled += read,
				Err(error) => {
					tracing::debug!(%error, "a video read failed");
					return (filled > 0).then_some(filled as u32);
				}
			}
		}
		Some(filled as u32)
	}

	fn position(&self) -> u64 {
		*self.position.lock().unwrap_or_else(PoisonError::into_inner)
	}

	fn set_position(&self, position: u64) {
		*self.position.lock().unwrap_or_else(PoisonError::into_inner) = position;
	}

	fn length(&self) -> Option<u64> {
		self.source.length().ok().flatten()
	}
}

#[implement(IMFByteStream)]
pub struct SourceStream {
	shared: Arc<Shared>,
	jobs: Mutex<Option<Sender<Job>>>,
}

impl SourceStream {
	pub fn create(source: Arc<dyn ByteSource>) -> IMFByteStream {
		let shared = Arc::new(Shared {
			source,
			position: Mutex::new(0),
			finished: Mutex::new(HashMap::new()),
		});
		let (sender, jobs) = mpsc::channel::<Job>();
		let worker = shared.clone();
		let spawned = std::thread::Builder::new()
			.name("video-read".to_owned())
			.spawn(move || {
				for job in jobs {
					let Target(pointer, length) = job.target;
					let target = unsafe { std::slice::from_raw_parts_mut(pointer, length) };
					let read = worker.read(job.position, target);
					if let Some(count) = read {
						worker.set_position(job.position + u64::from(count));
					}
					worker
						.finished
						.lock()
						.unwrap_or_else(PoisonError::into_inner)
						.insert(job.result.0.as_raw() as usize, read);
					unsafe {
						let _ = job.result.0.SetStatus(if read.is_some() { S_OK } else { E_FAIL });
						let _ = MFInvokeCallback(&job.result.0);
					}
				}
			});
		let jobs = match spawned {
			Ok(_) => Some(sender),
			Err(error) => {
				tracing::warn!(%error, "the video reader thread did not start");
				None
			}
		};
		Self {
			shared,
			jobs: Mutex::new(jobs),
		}
		.into()
	}
}

impl IMFByteStream_Impl for SourceStream_Impl {
	fn GetCapabilities(&self) -> Result<u32> {
		Ok(MFBYTESTREAM_IS_READABLE | MFBYTESTREAM_IS_SEEKABLE)
	}

	fn GetLength(&self) -> Result<u64> {
		Ok(self.shared.length().unwrap_or(UNKNOWN_LENGTH))
	}

	fn SetLength(&self, _length: u64) -> Result<()> {
		Err(E_NOTIMPL.into())
	}

	fn GetCurrentPosition(&self) -> Result<u64> {
		Ok(self.shared.position())
	}

	fn SetCurrentPosition(&self, position: u64) -> Result<()> {
		self.shared.set_position(position);
		Ok(())
	}

	fn IsEndOfStream(&self) -> Result<BOOL> {
		let ended = self
			.shared
			.length()
			.is_some_and(|length| self.shared.position() >= length);
		Ok(ended.into())
	}

	fn Read(&self, buffer: *mut u8, length: u32, read: *mut u32) -> Result<()> {
		if buffer.is_null() || read.is_null() {
			return Err(E_INVALIDARG.into());
		}
		let position = self.shared.position();
		let target = unsafe { std::slice::from_raw_parts_mut(buffer, length as usize) };
		let count = self.shared.read(position, target).ok_or(E_FAIL)?;
		self.shared.set_position(position + u64::from(count));
		unsafe { read.write(count) };
		Ok(())
	}

	fn BeginRead(
		&self,
		buffer: *mut u8,
		length: u32,
		callback: Ref<IMFAsyncCallback>,
		state: Ref<IUnknown>,
	) -> Result<()> {
		if buffer.is_null() {
			return Err(E_INVALIDARG.into());
		}
		let result = unsafe { MFCreateAsyncResult(None, callback.as_ref(), state.as_ref())? };
		let job = Job {
			target: Target(buffer, length as usize),
			position: self.shared.position(),
			result: Pending(result),
		};
		let jobs = self.jobs.lock().unwrap_or_else(PoisonError::into_inner);
		match jobs.as_ref() {
			Some(sender) if sender.send(job).is_ok() => Ok(()),
			_ => Err(E_FAIL.into()),
		}
	}

	fn EndRead(&self, result: Ref<IMFAsyncResult>) -> Result<u32> {
		let result = result.ok()?;
		let read = self
			.shared
			.finished
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&(result.as_raw() as usize));
		read.flatten().ok_or_else(|| E_FAIL.into())
	}

	fn Write(&self, _buffer: *const u8, _length: u32) -> Result<u32> {
		Err(E_NOTIMPL.into())
	}

	fn BeginWrite(
		&self,
		_buffer: *const u8,
		_length: u32,
		_callback: Ref<IMFAsyncCallback>,
		_state: Ref<IUnknown>,
	) -> Result<()> {
		Err(E_NOTIMPL.into())
	}

	fn EndWrite(&self, _result: Ref<IMFAsyncResult>) -> Result<u32> {
		Err(E_NOTIMPL.into())
	}

	fn Seek(&self, origin: MFBYTESTREAM_SEEK_ORIGIN, offset: i64, _flags: u32) -> Result<u64> {
		let base = match origin {
			origin if origin == msoBegin => 0,
			origin if origin == msoCurrent => self.shared.position(),
			_ => return Err(E_INVALIDARG.into()),
		};
		let position = base.checked_add_signed(offset).ok_or(E_INVALIDARG)?;
		self.shared.set_position(position);
		Ok(position)
	}

	fn Flush(&self) -> Result<()> {
		Ok(())
	}

	fn Close(&self) -> Result<()> {
		self.jobs.lock().unwrap_or_else(PoisonError::into_inner).take();
		Ok(())
	}
}
