use std::sync::OnceLock;
use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	IMFActivate, IMFAttributes, IMFMediaBuffer, IMFMediaType, IMFSample, IMFTransform,
	MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE, MF_MT_FRAME_SIZE,
	MF_MT_MINIMUM_DISPLAY_APERTURE, MF_VERSION, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
	MFSTARTUP_FULL, MFStartup, MFT_ENUM_FLAG, MFT_OUTPUT_DATA_BUFFER, MFT_REGISTER_TYPE_INFO, MFTEnumEx,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::core::GUID;

use super::VideoError;

const APERTURE_BYTES: usize = 16;
const APERTURE_AREA_OFFSET: usize = 8;
const MAX_PIXELS: usize = 7680 * 4320;

pub fn device_error(error: windows::core::Error) -> VideoError {
	VideoError::Device(error.message())
}

struct ComScope(bool);

impl ComScope {
	fn enter() -> Self {
		Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
	}
}

impl Drop for ComScope {
	fn drop(&mut self) {
		if self.0 {
			unsafe { CoUninitialize() };
		}
	}
}

thread_local! {
	static COM: ComScope = ComScope::enter();
}

pub fn prepare() -> Result<(), VideoError> {
	static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
	COM.with(|_| ());
	STARTED
		.get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|error| error.message()))
		.clone()
		.map_err(VideoError::Device)
}

pub fn size_of(attributes: &IMFAttributes, key: &GUID) -> Option<(u32, u32)> {
	let packed = unsafe { attributes.GetUINT64(key) }.ok()?;
	Some(((packed >> 32) as u32, packed as u32))
}

pub fn enumerate(
	category: GUID,
	flags: MFT_ENUM_FLAG,
	input: Option<MFT_REGISTER_TYPE_INFO>,
	output: Option<MFT_REGISTER_TYPE_INFO>,
) -> Result<Vec<IMFActivate>, VideoError> {
	let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
	let mut count = 0u32;
	unsafe {
		MFTEnumEx(
			category,
			flags,
			input.as_ref().map(std::ptr::from_ref),
			output.as_ref().map(std::ptr::from_ref),
			&mut activates,
			&mut count,
		)
	}
	.map_err(device_error)?;
	let mut found = Vec::with_capacity(count as usize);
	for index in 0..count as usize {
		if let Some(activate) = unsafe { std::ptr::read(activates.add(index)) } {
			found.push(activate);
		}
	}
	unsafe { CoTaskMemFree(Some(activates.cast_const().cast())) };
	Ok(found)
}

pub fn sample_from(data: &[u8], timestamp: Duration) -> windows::core::Result<IMFSample> {
	let length = u32::try_from(data.len()).map_err(|_| windows::core::Error::empty())?;
	unsafe {
		let buffer = MFCreateMemoryBuffer(length)?;
		let mut target = std::ptr::null_mut();
		buffer.Lock(&mut target, None, None)?;
		std::ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
		buffer.Unlock()?;
		buffer.SetCurrentLength(length)?;
		let sample = MFCreateSample()?;
		sample.AddBuffer(&buffer)?;
		sample.SetSampleTime(hundred_nanos(timestamp))?;
		Ok(sample)
	}
}

fn hundred_nanos(duration: Duration) -> i64 {
	i64::try_from(duration.as_nanos() / 100).unwrap_or(i64::MAX)
}

pub fn read_buffer(sample: &IMFSample) -> windows::core::Result<Vec<u8>> {
	unsafe {
		let buffer = sample.ConvertToContiguousBuffer()?;
		copy_out(&buffer)
	}
}

unsafe fn copy_out(buffer: &IMFMediaBuffer) -> windows::core::Result<Vec<u8>> {
	let mut source = std::ptr::null_mut();
	let mut length = 0u32;
	unsafe {
		buffer.Lock(&mut source, None, Some(&mut length))?;
		let data = std::slice::from_raw_parts(source, length as usize).to_vec();
		buffer.Unlock()?;
		Ok(data)
	}
}

pub fn display_size(media_type: &IMFMediaType) -> (usize, usize) {
	let mut aperture = [0u8; APERTURE_BYTES];
	if unsafe { media_type.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut aperture, None) }.is_ok() {
		let width = i32::from_le_bytes(
			aperture[APERTURE_AREA_OFFSET..APERTURE_AREA_OFFSET + 4]
				.try_into()
				.unwrap_or_default(),
		);
		let height = i32::from_le_bytes(
			aperture[APERTURE_AREA_OFFSET + 4..]
				.try_into()
				.unwrap_or_default(),
		);
		if width > 0 && height > 0 {
			return (width as usize & !1, height as usize & !1);
		}
	}
	let (width, height) = size_of(media_type, &MF_MT_FRAME_SIZE).unwrap_or_default();
	(width as usize & !1, height as usize & !1)
}

pub fn within_pixel_cap(width: usize, height: usize) -> bool {
	width.saturating_mul(height) <= MAX_PIXELS
}

pub fn packed_nv12(data: &[u8], width: usize, height: usize, stride: usize) -> Option<Vec<u8>> {
	let aligned_height = (data.len() * 2 / (3 * stride.max(1))).max(height);
	if width == 0 || height == 0 || data.len() < stride * aligned_height * 3 / 2 {
		return None;
	}
	let (luma, chroma) = data.split_at(stride * aligned_height);
	let mut packed = Vec::with_capacity(width * height * 3 / 2);
	for row in 0..height {
		packed.extend_from_slice(&luma[row * stride..][..width]);
	}
	for row in 0..height / 2 {
		packed.extend_from_slice(&chroma[row * stride..][..width]);
	}
	Some(packed)
}

pub fn media_type() -> windows::core::Result<IMFMediaType> {
	unsafe { MFCreateMediaType() }
}

pub enum Output {
	Sample(IMFSample),
	NeedInput,
	StreamChange,
}

pub fn process_output(transform: &IMFTransform, allocate: Option<u32>) -> windows::core::Result<Output> {
	let mut buffer = MFT_OUTPUT_DATA_BUFFER {
		dwStreamID: 0,
		pSample: std::mem::ManuallyDrop::new(None),
		dwStatus: 0,
		pEvents: std::mem::ManuallyDrop::new(None),
	};
	if let Some(bytes) = allocate {
		unsafe {
			let media = MFCreateMemoryBuffer(bytes)?;
			let sample = MFCreateSample()?;
			sample.AddBuffer(&media)?;
			buffer.pSample = std::mem::ManuallyDrop::new(Some(sample));
		}
	}
	let mut status = 0u32;
	let result = unsafe { transform.ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status) };
	let sample = std::mem::ManuallyDrop::into_inner(std::mem::replace(
		&mut buffer.pSample,
		std::mem::ManuallyDrop::new(None),
	));
	drop(std::mem::ManuallyDrop::into_inner(std::mem::replace(
		&mut buffer.pEvents,
		std::mem::ManuallyDrop::new(None),
	)));
	match result {
		Ok(()) => Ok(sample.map_or(Output::NeedInput, Output::Sample)),
		Err(error) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(Output::NeedInput),
		Err(error) if error.code() == MF_E_TRANSFORM_STREAM_CHANGE => Ok(Output::StreamChange),
		Err(error) => Err(error),
	}
}
