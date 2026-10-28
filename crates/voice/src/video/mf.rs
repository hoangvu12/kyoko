use std::sync::OnceLock;
use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	IMFActivate, IMFAttributes, IMFMediaBuffer, IMFMediaType, IMFSample, IMFTransform,
	MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE, MF_VERSION, MFCreateMediaType,
	MFCreateMemoryBuffer, MFCreateSample, MFSTARTUP_FULL, MFStartup, MFT_ENUM_FLAG, MFT_OUTPUT_DATA_BUFFER,
	MFT_REGISTER_TYPE_INFO, MFTEnumEx,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::core::GUID;

use super::VideoError;

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
