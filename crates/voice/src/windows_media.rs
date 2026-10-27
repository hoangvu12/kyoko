use windows::Graphics::Capture::GraphicsCaptureSession;
use windows::Win32::Media::MediaFoundation::{
	IMFActivate, IMFTransform, MF_VERSION, MFMediaType_Video, MFSTARTUP_FULL, MFShutdown, MFStartup,
	MFT_CATEGORY_VIDEO_DECODER, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG, MFT_ENUM_FLAG_HARDWARE,
	MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_REGISTER_TYPE_INFO, MFTEnumEx,
	MFVideoFormat_H264, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::core::GUID;

use crate::text;

const NV12: MFT_REGISTER_TYPE_INFO = MFT_REGISTER_TYPE_INFO {
	guidMajorType: MFMediaType_Video,
	guidSubtype: MFVideoFormat_NV12,
};
const H264: MFT_REGISTER_TYPE_INFO = MFT_REGISTER_TYPE_INFO {
	guidMajorType: MFMediaType_Video,
	guidSubtype: MFVideoFormat_H264,
};

struct Started {
	owns_com: bool,
}

impl Started {
	fn new() -> Result<Started, String> {
		let owns_com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
		if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
			if owns_com {
				unsafe { CoUninitialize() };
			}
			return Err(error.message());
		}
		Ok(Started { owns_com })
	}
}

impl Drop for Started {
	fn drop(&mut self) {
		unsafe {
			let _ = MFShutdown();
			if self.owns_com {
				CoUninitialize();
			}
		}
	}
}

pub fn h264_transforms() -> Result<String, String> {
	let _started = Started::new()?;
	let sorted = MFT_ENUM_FLAG_SORTANDFILTER;
	let hardware_encoders = enumerate(
		MFT_CATEGORY_VIDEO_ENCODER,
		sorted | MFT_ENUM_FLAG_HARDWARE,
		Some(NV12),
		Some(H264),
	)?;
	let software_encoders = enumerate(
		MFT_CATEGORY_VIDEO_ENCODER,
		sorted | MFT_ENUM_FLAG_SYNCMFT,
		Some(NV12),
		Some(H264),
	)?;
	let decoders = enumerate(
		MFT_CATEGORY_VIDEO_DECODER,
		sorted | MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_HARDWARE,
		Some(H264),
		None,
	)?;
	let activated = software_encoders
		.first()
		.map(|activate| unsafe { activate.ActivateObject::<IMFTransform>() }.is_ok())
		.unwrap_or(false);
	Ok(format!(
		"H.264 encoders: {} hardware, {} software (first activates: {activated}); decoders: {}",
		hardware_encoders.len(),
		software_encoders.len(),
		decoders.len()
	))
}

pub fn capture_supported() -> Result<String, String> {
	let supported = GraphicsCaptureSession::IsSupported().map_err(text)?;
	Ok(format!("GraphicsCaptureSession::IsSupported = {supported}"))
}

fn enumerate(
	category: GUID,
	flags: MFT_ENUM_FLAG,
	input: Option<MFT_REGISTER_TYPE_INFO>,
	output: Option<MFT_REGISTER_TYPE_INFO>,
) -> Result<Vec<IMFActivate>, String> {
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
	.map_err(text)?;
	let found = (0..count as usize)
		.filter_map(|index| unsafe { std::ptr::read(activates.add(index)) })
		.collect();
	unsafe { CoTaskMemFree(Some(activates.cast_const().cast())) };
	Ok(found)
}
