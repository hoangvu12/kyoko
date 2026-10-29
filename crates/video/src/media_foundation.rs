use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	IMFSample, IMFTransform, MF_E_NOTACCEPTING, MF_LOW_LATENCY, MF_MT_DEFAULT_STRIDE, MF_MT_MAJOR_TYPE,
	MF_MT_SUBTYPE, MFMediaType_Video, MFT_CATEGORY_VIDEO_DECODER, MFT_ENUM_FLAG_SORTANDFILTER,
	MFT_ENUM_FLAG_SYNCMFT, MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
	MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES,
	MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO, MFVideoFormat_H264, MFVideoFormat_NV12,
};

use super::mf;
use super::{VideoError, VideoFrame};

const FALLBACK_OUTPUT_BYTES: u32 = 8 * 1024 * 1024;
const MAX_OUTPUT_TYPES: u32 = 64;

const H264_INPUT: MFT_REGISTER_TYPE_INFO = MFT_REGISTER_TYPE_INFO {
	guidMajorType: MFMediaType_Video,
	guidSubtype: MFVideoFormat_H264,
};

struct OutputLayout {
	width: usize,
	height: usize,
	stride: usize,
}

pub struct MediaFoundationDecoder {
	transform: IMFTransform,
	layout: OutputLayout,
	allocate_bytes: Option<u32>,
}

unsafe impl Send for MediaFoundationDecoder {}

fn failure(error: windows::core::Error) -> VideoError {
	VideoError::Decode(error.message())
}

pub fn decoders_present() -> bool {
	mf::prepare().is_ok()
		&& mf::enumerate(MFT_CATEGORY_VIDEO_DECODER, flags(), Some(H264_INPUT), None)
			.is_ok_and(|found| !found.is_empty())
}

fn flags() -> windows::Win32::Media::MediaFoundation::MFT_ENUM_FLAG {
	MFT_ENUM_FLAG_SORTANDFILTER | MFT_ENUM_FLAG_SYNCMFT
}

impl MediaFoundationDecoder {
	pub fn new() -> Result<Self, VideoError> {
		mf::prepare()?;
		let mut last = VideoError::NotFound;
		for activate in mf::enumerate(MFT_CATEGORY_VIDEO_DECODER, flags(), Some(H264_INPUT), None)? {
			let transform = match unsafe { activate.ActivateObject::<IMFTransform>() } {
				Ok(transform) => transform,
				Err(error) => {
					last = failure(error);
					continue;
				}
			};
			match Self::configure(transform) {
				Ok(decoder) => return Ok(decoder),
				Err(error) => last = error,
			}
		}
		Err(last)
	}

	fn configure(transform: IMFTransform) -> Result<Self, VideoError> {
		unsafe {
			if let Ok(attributes) = transform.GetAttributes() {
				let _ = attributes.SetUINT32(&MF_LOW_LATENCY, 1);
			}
			let input = mf::media_type().map_err(failure)?;
			input
				.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
				.map_err(failure)?;
			input
				.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
				.map_err(failure)?;
			transform.SetInputType(0, &input, 0).map_err(failure)?;
		}
		let mut decoder = Self {
			transform,
			layout: OutputLayout {
				width: 0,
				height: 0,
				stride: 0,
			},
			allocate_bytes: None,
		};
		decoder.select_output()?;
		unsafe {
			decoder
				.transform
				.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
				.map_err(failure)?;
			decoder
				.transform
				.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
				.map_err(failure)?;
		}
		Ok(decoder)
	}

	fn select_output(&mut self) -> Result<(), VideoError> {
		let chosen = (0..MAX_OUTPUT_TYPES)
			.map_while(|index| unsafe { self.transform.GetOutputAvailableType(0, index) }.ok())
			.find(|candidate| {
				unsafe { candidate.GetGUID(&MF_MT_SUBTYPE) }
					.is_ok_and(|subtype| subtype == MFVideoFormat_NV12)
			})
			.ok_or(VideoError::Decode("the decoder offers no NV12 output".to_owned()))?;
		unsafe { self.transform.SetOutputType(0, &chosen, 0) }.map_err(failure)?;
		let (width, height) = mf::display_size(&chosen);
		let stride =
			unsafe { chosen.GetUINT32(&MF_MT_DEFAULT_STRIDE) }.map_or(width, |stride| stride as i32 as usize);
		self.layout = OutputLayout {
			width,
			height,
			stride: stride.max(width),
		};
		let info = unsafe { self.transform.GetOutputStreamInfo(0) }.map_err(failure)?;
		let provides = info.dwFlags
			& (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32
			!= 0;
		let bytes = if info.cbSize == 0 {
			FALLBACK_OUTPUT_BYTES
		} else {
			info.cbSize
		};
		self.allocate_bytes = (!provides).then_some(bytes);
		Ok(())
	}

	pub fn decode(&mut self, frame: &[u8], timestamp: Duration) -> Result<Option<VideoFrame>, VideoError> {
		mf::prepare()?;
		let sample = mf::sample_from(frame, timestamp).map_err(failure)?;
		let mut latest = None;
		match unsafe { self.transform.ProcessInput(0, &sample, 0) } {
			Ok(()) => {}
			Err(error) if error.code() == MF_E_NOTACCEPTING => {
				self.drain(timestamp, &mut latest)?;
				unsafe { self.transform.ProcessInput(0, &sample, 0) }.map_err(failure)?;
			}
			Err(error) => return Err(failure(error)),
		}
		self.drain(timestamp, &mut latest)?;
		Ok(latest)
	}

	fn drain(&mut self, timestamp: Duration, latest: &mut Option<VideoFrame>) -> Result<(), VideoError> {
		loop {
			match mf::process_output(&self.transform, self.allocate_bytes).map_err(failure)? {
				mf::Output::Sample(_) if !mf::within_pixel_cap(self.layout.width, self.layout.height) => {}
				mf::Output::Sample(sample) => *latest = Some(self.convert(&sample, timestamp)?),
				mf::Output::NeedInput => return Ok(()),
				mf::Output::StreamChange => self.select_output()?,
			}
		}
	}

	fn convert(&self, sample: &IMFSample, timestamp: Duration) -> Result<VideoFrame, VideoError> {
		let data = mf::read_buffer(sample).map_err(failure)?;
		let OutputLayout {
			width,
			height,
			stride,
		} = self.layout;
		let packed = mf::packed_nv12(&data, width, height, stride)
			.ok_or_else(|| VideoError::Decode("the decoder returned a short frame".to_owned()))?;
		Ok(VideoFrame {
			width: width as u32,
			height: height as u32,
			nv12: packed,
			timestamp,
		})
	}
}

impl Drop for MediaFoundationDecoder {
	fn drop(&mut self) {
		unsafe {
			let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
		}
	}
}
