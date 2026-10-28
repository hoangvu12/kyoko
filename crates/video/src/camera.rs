use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	IMFActivate, IMFMediaSource, IMFSourceReader, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
	MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
	MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
	MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
	MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
	MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MFCreateAttributes,
	MFCreateSourceReaderFromMediaSource, MFEnumDeviceSources, MFMediaType_Video, MFVideoFormat_NV12,
	MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::PWSTR;

use super::capture::{CameraInfo, VideoSource};
use super::{VideoError, VideoFrame, mf, mf_send};

const VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const EMPTY_READS_BEFORE_TIMEOUT: u32 = 200;

fn failure(error: windows::core::Error) -> VideoError {
	mf::device_error(error)
}

fn text_of(activate: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
	let mut value = PWSTR::null();
	let mut length = 0u32;
	unsafe { activate.GetAllocatedString(key, &mut value, &mut length) }.ok()?;
	let text = unsafe { value.to_string() }.ok();
	unsafe { CoTaskMemFree(Some(value.0.cast_const().cast())) };
	text
}

fn devices() -> Result<Vec<IMFActivate>, VideoError> {
	mf::prepare()?;
	unsafe {
		let mut attributes = None;
		MFCreateAttributes(&mut attributes, 1).map_err(failure)?;
		let attributes = attributes.ok_or(VideoError::NotFound)?;
		attributes
			.SetGUID(
				&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
				&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
			)
			.map_err(failure)?;
		let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
		let mut count = 0u32;
		MFEnumDeviceSources(&attributes, &mut activates, &mut count).map_err(failure)?;
		let mut found = Vec::with_capacity(count as usize);
		for index in 0..count as usize {
			if let Some(activate) = std::ptr::read(activates.add(index)) {
				found.push(activate);
			}
		}
		CoTaskMemFree(Some(activates.cast_const().cast()));
		Ok(found)
	}
}

pub struct CameraSource {
	reader: IMFSourceReader,
	width: u32,
	height: u32,
	stride: usize,
	empty_reads: u32,
}

unsafe impl Send for CameraSource {}

impl CameraSource {
	pub fn list() -> Result<Vec<CameraInfo>, VideoError> {
		Ok(devices()?
			.iter()
			.filter_map(|activate| {
				Some(CameraInfo {
					id: text_of(activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)?,
					name: text_of(activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME).unwrap_or_default(),
				})
			})
			.collect())
	}

	pub fn open(id: &str, width: u32, height: u32, fps: u32) -> Result<Self, VideoError> {
		let activate = devices()?
			.into_iter()
			.find(|activate| {
				text_of(activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).as_deref()
					== Some(id)
			})
			.ok_or(VideoError::NotFound)?;
		unsafe {
			let source: IMFMediaSource = activate.ActivateObject().map_err(failure)?;
			let mut attributes = None;
			MFCreateAttributes(&mut attributes, 2).map_err(failure)?;
			let attributes = attributes.ok_or(VideoError::NotFound)?;
			attributes
				.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
				.map_err(failure)?;
			attributes
				.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
				.map_err(failure)?;
			let reader = MFCreateSourceReaderFromMediaSource(&source, &attributes).map_err(failure)?;
			let requested = mf::media_type().map_err(failure)?;
			requested
				.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
				.map_err(failure)?;
			requested
				.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
				.map_err(failure)?;
			requested
				.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
				.map_err(failure)?;
			mf_send::set_size(&requested, &MF_MT_FRAME_SIZE, width, height).map_err(failure)?;
			mf_send::set_size(&requested, &MF_MT_FRAME_RATE, fps, 1).map_err(failure)?;
			reader
				.SetCurrentMediaType(VIDEO_STREAM, None, &requested)
				.map_err(failure)?;
			let current = reader.GetCurrentMediaType(VIDEO_STREAM).map_err(failure)?;
			let (actual_width, actual_height) =
				mf::size_of(&current, &MF_MT_FRAME_SIZE).unwrap_or((width, height));
			let stride = current
				.GetUINT32(&MF_MT_DEFAULT_STRIDE)
				.map_or(actual_width as usize, |stride| {
					(stride as i32).unsigned_abs() as usize
				});
			Ok(Self {
				reader,
				width: actual_width,
				height: actual_height,
				stride: stride.max(actual_width as usize),
				empty_reads: 0,
			})
		}
	}
}

impl VideoSource for CameraSource {
	fn next_frame(&mut self) -> Result<VideoFrame, VideoError> {
		mf::prepare()?;
		loop {
			let mut flags = 0u32;
			let mut stamp = 0i64;
			let mut sample = None;
			unsafe {
				self.reader.ReadSample(
					VIDEO_STREAM,
					0,
					None,
					Some(&mut flags),
					Some(&mut stamp),
					Some(&mut sample),
				)
			}
			.map_err(failure)?;
			if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
				return Err(VideoError::Device("camera stream ended".to_owned()));
			}
			let Some(sample) = sample else {
				self.empty_reads += 1;
				if self.empty_reads > EMPTY_READS_BEFORE_TIMEOUT {
					return Err(VideoError::Timeout);
				}
				continue;
			};
			self.empty_reads = 0;
			let data = mf::read_buffer(&sample).map_err(failure)?;
			return Ok(VideoFrame {
				width: self.width,
				height: self.height,
				nv12: mf::packed_nv12(&data, self.width as usize, self.height as usize, self.stride)
					.ok_or(VideoError::InvalidFrame("camera returned a short frame"))?,
				timestamp: Duration::from_nanos(stamp.max(0) as u64 * 100),
			});
		}
	}

	fn size(&self) -> (u32, u32) {
		(self.width, self.height)
	}
}
