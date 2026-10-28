use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	CODECAPI_AVEncCommonMeanBitRate, CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVGOPSize,
	CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMFSample, IMFTransform,
	MF_E_NOTACCEPTING, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
	MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_MPEG2_PROFILE,
	MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE, MFMediaType_Video, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG,
	MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_MESSAGE_COMMAND_FLUSH,
	MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
	MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO,
	MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive,
	eAVEncCommonRateControlMode_LowDelayVBR, eAVEncH264VProfile_ConstrainedBase,
};
use windows::core::Interface;

use super::annex_b as nal;

use super::encode::{EncodedFrame, EncoderConfig};
use super::{VideoError, mf, mf_send};

const INPUT: MFT_REGISTER_TYPE_INFO = MFT_REGISTER_TYPE_INFO {
	guidMajorType: MFMediaType_Video,
	guidSubtype: MFVideoFormat_NV12,
};
const OUTPUT: MFT_REGISTER_TYPE_INFO = MFT_REGISTER_TYPE_INFO {
	guidMajorType: MFMediaType_Video,
	guidSubtype: MFVideoFormat_H264,
};

pub struct MediaFoundationEncoder {
	transform: IMFTransform,
	codec: Option<ICodecAPI>,
	config: EncoderConfig,
	system_allocates_output: bool,
	output_bytes: u32,
	sequence_header: Option<Vec<u8>>,
}

unsafe impl Send for MediaFoundationEncoder {}

fn failure(error: windows::core::Error) -> VideoError {
	VideoError::Encode(error.message())
}

impl MediaFoundationEncoder {
	pub fn new(config: EncoderConfig) -> Result<Self, VideoError> {
		mf::prepare()?;
		let mut last = VideoError::NotFound;
		for activate in mf::enumerate(MFT_CATEGORY_VIDEO_ENCODER, flags(), Some(INPUT), Some(OUTPUT))? {
			let transform = match unsafe { activate.ActivateObject::<IMFTransform>() } {
				Ok(transform) => transform,
				Err(error) => {
					last = failure(error);
					continue;
				}
			};
			match Self::configure(transform, config) {
				Ok(encoder) => return Ok(encoder),
				Err(error) => last = error,
			}
		}
		Err(last)
	}

	fn configure(transform: IMFTransform, config: EncoderConfig) -> Result<Self, VideoError> {
		let codec = transform.cast::<ICodecAPI>().ok();
		if let Some(codec) = &codec {
			apply_codec_settings(codec, &config);
		}
		Self::negotiate(&transform, &config)?;
		if let Some(codec) = &codec {
			apply_codec_settings(codec, &config);
		}
		let info = unsafe { transform.GetOutputStreamInfo(0) }.map_err(failure)?;
		let provides = info.dwFlags
			& (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32
			!= 0;
		unsafe {
			transform
				.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
				.map_err(failure)?;
			transform
				.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
				.map_err(failure)?;
		}
		Ok(Self {
			transform,
			codec,
			config,
			system_allocates_output: provides,
			output_bytes: info.cbSize.max(config.width * config.height),
			sequence_header: None,
		})
	}

	fn negotiate(transform: &IMFTransform, config: &EncoderConfig) -> Result<(), VideoError> {
		unsafe {
			let output = mf::media_type().map_err(failure)?;
			output
				.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
				.map_err(failure)?;
			output
				.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
				.map_err(failure)?;
			output
				.SetUINT32(&MF_MT_AVG_BITRATE, config.bitrate)
				.map_err(failure)?;
			output
				.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
				.map_err(failure)?;
			output
				.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_ConstrainedBase.0 as u32)
				.map_err(failure)?;
			mf_send::set_size(&output, &MF_MT_FRAME_SIZE, config.width, config.height).map_err(failure)?;
			mf_send::set_size(&output, &MF_MT_FRAME_RATE, config.fps, 1).map_err(failure)?;
			mf_send::set_size(&output, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1).map_err(failure)?;
			transform.SetOutputType(0, &output, 0).map_err(failure)?;
			let input = mf::media_type().map_err(failure)?;
			input
				.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
				.map_err(failure)?;
			input
				.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
				.map_err(failure)?;
			input
				.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
				.map_err(failure)?;
			input
				.SetUINT32(&MF_MT_DEFAULT_STRIDE, config.width)
				.map_err(failure)?;
			mf_send::set_size(&input, &MF_MT_FRAME_SIZE, config.width, config.height).map_err(failure)?;
			mf_send::set_size(&input, &MF_MT_FRAME_RATE, config.fps, 1).map_err(failure)?;
			mf_send::set_size(&input, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1).map_err(failure)?;
			transform.SetInputType(0, &input, 0).map_err(failure)?;
		}
		Ok(())
	}

	pub fn encode(
		&mut self,
		nv12: &[u8],
		timestamp: Duration,
		keyframe: bool,
	) -> Result<Vec<EncodedFrame>, VideoError> {
		mf::prepare()?;
		if keyframe && let Some(codec) = &self.codec {
			unsafe { codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &mf_send::variant_u32(1)) }
				.map_err(failure)?;
		}
		let sample = mf_send::timed_sample(nv12, timestamp, mf_send::frame_duration(self.config.fps))
			.map_err(failure)?;
		let mut outputs = Vec::new();
		self.encode_sync(&sample, &mut outputs)?;
		Ok(outputs.into_iter().map(|data| self.finish(data)).collect())
	}

	fn finish(&mut self, mut data: Vec<u8>) -> EncodedFrame {
		let keyframe = nal::is_keyframe(&data);
		if keyframe && !has_parameter_sets(&data) {
			if self.sequence_header.is_none() {
				self.sequence_header = Some(self.read_sequence_header());
			}
			if let Some(header) = &self.sequence_header {
				data.splice(0..0, header.iter().copied());
			}
		}
		EncodedFrame { data, keyframe }
	}

	fn read_sequence_header(&self) -> Vec<u8> {
		let Ok(output) = (unsafe { self.transform.GetOutputCurrentType(0) }) else {
			return Vec::new();
		};
		let Ok(size) = (unsafe { output.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) }) else {
			return Vec::new();
		};
		let mut header = vec![0u8; size as usize];
		match unsafe { output.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut header, None) } {
			Ok(()) => header,
			Err(_) => Vec::new(),
		}
	}

	fn encode_sync(&mut self, sample: &IMFSample, outputs: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
		match unsafe { self.transform.ProcessInput(0, sample, 0) } {
			Ok(()) => {}
			Err(error) if error.code() == MF_E_NOTACCEPTING => {
				self.drain(outputs)?;
				unsafe { self.transform.ProcessInput(0, sample, 0) }.map_err(failure)?;
			}
			Err(error) => return Err(failure(error)),
		}
		self.drain(outputs)
	}

	fn drain(&mut self, outputs: &mut Vec<Vec<u8>>) -> Result<(), VideoError> {
		loop {
			match self.take_output() {
				Ok(Some(data)) => outputs.push(data),
				Ok(None) => return Ok(()),
				Err(error) => return Err(error),
			}
		}
	}

	fn take_output(&mut self) -> Result<Option<Vec<u8>>, VideoError> {
		let allocate = (!self.system_allocates_output).then_some(self.output_bytes);
		match mf::process_output(&self.transform, allocate).map_err(failure)? {
			mf::Output::Sample(sample) => {
				Ok(Some(mf::read_buffer(&sample).map_err(failure)?).filter(|data| !data.is_empty()))
			}
			mf::Output::NeedInput | mf::Output::StreamChange => Ok(None),
		}
	}
}

impl Drop for MediaFoundationEncoder {
	fn drop(&mut self) {
		unsafe {
			let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
		}
	}
}

fn apply_codec_settings(codec: &ICodecAPI, config: &EncoderConfig) {
	unsafe {
		let _ = codec.SetValue(&CODECAPI_AVLowLatencyMode, &mf_send::variant_bool(true));
		let _ = codec.SetValue(
			&CODECAPI_AVEncCommonRateControlMode,
			&mf_send::variant_u32(eAVEncCommonRateControlMode_LowDelayVBR.0 as u32),
		);
		let _ = codec.SetValue(
			&CODECAPI_AVEncCommonMeanBitRate,
			&mf_send::variant_u32(config.bitrate),
		);
		let _ = codec.SetValue(
			&CODECAPI_AVEncMPVGOPSize,
			&mf_send::variant_u32(config.fps * config.keyframe_seconds),
		);
	}
}

fn flags() -> MFT_ENUM_FLAG {
	MFT_ENUM_FLAG_SORTANDFILTER | MFT_ENUM_FLAG_SYNCMFT
}

pub fn encoders_present() -> bool {
	mf::prepare().is_ok()
		&& mf::enumerate(MFT_CATEGORY_VIDEO_ENCODER, flags(), Some(INPUT), Some(OUTPUT))
			.is_ok_and(|found| !found.is_empty())
}

fn has_parameter_sets(frame: &[u8]) -> bool {
	nal::units(frame)
		.iter()
		.any(|unit| nal::unit_type(unit[0]) == nal::SPS)
}
