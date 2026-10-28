use std::sync::Arc;
use std::time::Duration;

use windows::Win32::Media::MediaFoundation::{
	IMFAttributes, IMFMediaSource, IMFMediaType, IMFSample, IMFSourceReader, MF_MT_AUDIO_NUM_CHANNELS,
	MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_DEFAULT_STRIDE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_OBJECT_TYPE,
	MF_PD_DURATION, MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE,
	MF_RESOLUTION_MEDIASOURCE, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ANY_STREAM,
	MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_MEDIASOURCE,
	MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM, MFAudioFormat_Float,
	MFCreateAttributes, MFCreateSourceReaderFromMediaSource, MFCreateSourceResolver, MFMediaType_Audio,
	MFMediaType_Video, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::core::{GUID, HSTRING, IUnknown, Interface};

use crate::byte_stream::SourceStream;
use crate::file::{ByteSource, Packet, SoundTrack, Tracks, VideoTrack};
use crate::mf;
use crate::{VideoError, VideoFrame};

const MAX_STREAMS: u32 = 32;
const WANTED_CHANNELS: u32 = 2;
const HUNDRED_NANOS_PER_SECOND: u64 = 10_000_000;
const FLOAT_BYTES: usize = 4;

struct Layout {
	width: usize,
	height: usize,
	stride: usize,
}

pub struct SourceReader {
	reader: IMFSourceReader,
	video: Option<(u32, Layout)>,
	sound: Option<(u32, SoundTrack)>,
	duration: Option<Duration>,
	ended: Vec<u32>,
}

unsafe impl Send for SourceReader {}

fn failure(error: windows::core::Error) -> VideoError {
	VideoError::Decode(error.message())
}

impl SourceReader {
	pub fn open(source: Arc<dyn ByteSource>, name: &str) -> Result<Self, VideoError> {
		mf::prepare()?;
		let stream = SourceStream::create(source);
		let media_source = unsafe {
			let resolver = MFCreateSourceResolver().map_err(failure)?;
			let mut kind = MF_OBJECT_TYPE::default();
			let mut object: Option<IUnknown> = None;
			resolver
				.CreateObjectFromByteStream(
					&stream,
					&HSTRING::from(name),
					(MF_RESOLUTION_MEDIASOURCE
						| MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE)
						.0 as u32,
					None,
					&mut kind,
					&mut object,
				)
				.map_err(failure)?;
			object
				.ok_or_else(|| VideoError::Decode("no media source".to_owned()))?
				.cast::<IMFMediaSource>()
				.map_err(failure)?
		};
		let reader = unsafe {
			let mut attributes: Option<IMFAttributes> = None;
			MFCreateAttributes(&mut attributes, 1).map_err(failure)?;
			let attributes = attributes.ok_or_else(|| VideoError::Decode("no attributes".to_owned()))?;
			attributes
				.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
				.map_err(failure)?;
			MFCreateSourceReaderFromMediaSource(&media_source, &attributes).map_err(failure)?
		};
		let mut opened = Self {
			reader,
			video: None,
			sound: None,
			duration: None,
			ended: Vec::new(),
		};
		opened.choose_streams()?;
		opened.duration = opened.read_duration();
		Ok(opened)
	}

	pub fn tracks(&self) -> Tracks {
		Tracks {
			video: self.video.as_ref().map(|(_, layout)| VideoTrack {
				width: layout.width as u32,
				height: layout.height as u32,
			}),
			sound: self.sound.map(|(_, sound)| sound),
			duration: self.duration,
		}
	}

	fn choose_streams(&mut self) -> Result<(), VideoError> {
		unsafe {
			let _ = self
				.reader
				.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
		}
		let mut video = None;
		let mut sound = None;
		for index in 0..MAX_STREAMS {
			let Ok(native) = (unsafe { self.reader.GetNativeMediaType(index, 0) }) else {
				break;
			};
			let Ok(major) = (unsafe { native.GetGUID(&MF_MT_MAJOR_TYPE) }) else {
				continue;
			};
			if major == MFMediaType_Video && video.is_none() {
				video = Some(index);
			} else if major == MFMediaType_Audio && sound.is_none() {
				sound = Some(index);
			}
		}
		let video = video.ok_or(VideoError::NotFound)?;
		self.select_video(video)?;
		if let Some(index) = sound {
			match self.select_sound(index) {
				Ok(track) => self.sound = Some((index, track)),
				Err(error) => tracing::debug!(%error, "the video's sound track is not decodable"),
			}
		}
		Ok(())
	}

	fn select_video(&mut self, index: u32) -> Result<(), VideoError> {
		let wanted = media_type(MFMediaType_Video, MFVideoFormat_NV12).map_err(failure)?;
		unsafe {
			self.reader.SetStreamSelection(index, true).map_err(failure)?;
			self.reader
				.SetCurrentMediaType(index, None, &wanted)
				.map_err(|error| {
					VideoError::Decode(format!("no decoder for this video: {}", error.message()))
				})?;
		}
		self.video = Some((index, self.layout(index)?));
		Ok(())
	}

	fn layout(&self, index: u32) -> Result<Layout, VideoError> {
		let current = unsafe { self.reader.GetCurrentMediaType(index) }.map_err(failure)?;
		let (width, height) = mf::display_size(&current);
		if width == 0 || height == 0 {
			return Err(VideoError::InvalidFrame("the video has no size"));
		}
		let stride = unsafe { current.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
			.map_or(width, |stride| stride as i32 as usize);
		Ok(Layout {
			width,
			height,
			stride: stride.max(width),
		})
	}

	fn select_sound(&mut self, index: u32) -> Result<SoundTrack, VideoError> {
		unsafe {
			self.reader.SetStreamSelection(index, true).map_err(failure)?;
		}
		let stereo = media_type(MFMediaType_Audio, MFAudioFormat_Float).map_err(failure)?;
		let chosen = unsafe {
			let _ = stereo.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, WANTED_CHANNELS);
			self.reader
				.SetCurrentMediaType(index, None, &stereo)
				.or_else(|_| {
					let plain = media_type(MFMediaType_Audio, MFAudioFormat_Float)?;
					self.reader.SetCurrentMediaType(index, None, &plain)
				})
		};
		if let Err(error) = chosen {
			unsafe {
				let _ = self.reader.SetStreamSelection(index, false);
			}
			return Err(failure(error));
		}
		let current = unsafe { self.reader.GetCurrentMediaType(index) }.map_err(failure)?;
		let rate = unsafe { current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND) }.map_err(failure)?;
		let channels = unsafe { current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS) }.map_err(failure)?;
		match u16::try_from(channels) {
			Ok(channels) if rate > 0 && channels > 0 => Ok(SoundTrack { rate, channels }),
			_ => Err(VideoError::Decode("the sound track has no shape".to_owned())),
		}
	}

	fn read_duration(&self) -> Option<Duration> {
		let value = unsafe {
			self.reader
				.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
		}
		.ok()?;
		let hundreds = u64::try_from(&value).ok()?;
		(hundreds > 0).then(|| hundred_nanos(hundreds as i64))
	}

	pub fn packet(&mut self) -> Result<Packet, VideoError> {
		loop {
			let mut index = 0u32;
			let mut flags = 0u32;
			let mut timestamp = 0i64;
			let mut sample: Option<IMFSample> = None;
			unsafe {
				self.reader
					.ReadSample(
						MF_SOURCE_READER_ANY_STREAM.0 as u32,
						0,
						Some(&mut index),
						Some(&mut flags),
						Some(&mut timestamp),
						Some(&mut sample),
					)
					.map_err(failure)?;
			}
			if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0
				&& let Some((video, _)) = self.video
				&& video == index
			{
				self.video = Some((video, self.layout(video)?));
			}
			if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
				if !self.ended.contains(&index) {
					self.ended.push(index);
				}
				if self.finished() {
					return Ok(Packet::End);
				}
				continue;
			}
			let Some(sample) = sample else {
				continue;
			};
			let at = hundred_nanos(timestamp);
			if let Some((video, layout)) = &self.video
				&& *video == index
			{
				return Ok(Packet::Video(frame(&sample, layout, at)?));
			}
			if let Some((sound, _)) = self.sound
				&& sound == index
			{
				return Ok(Packet::Audio {
					timestamp: at,
					samples: floats(&sample)?,
				});
			}
		}
	}

	fn finished(&self) -> bool {
		let streams = self
			.video
			.as_ref()
			.map(|(index, _)| *index)
			.into_iter()
			.chain(self.sound.map(|(index, _)| index));
		streams.into_iter().all(|index| self.ended.contains(&index))
	}

	pub fn seek(&mut self, to: Duration) -> Result<(), VideoError> {
		let hundreds = i64::try_from(to.as_nanos() / 100).unwrap_or(i64::MAX);
		let position = PROPVARIANT::from(hundreds);
		unsafe { self.reader.SetCurrentPosition(&GUID::zeroed(), &position) }.map_err(failure)?;
		self.ended.clear();
		Ok(())
	}
}

fn media_type(major: GUID, subtype: GUID) -> windows::core::Result<IMFMediaType> {
	let media = mf::media_type()?;
	unsafe {
		media.SetGUID(&MF_MT_MAJOR_TYPE, &major)?;
		media.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
	}
	Ok(media)
}

fn hundred_nanos(value: i64) -> Duration {
	let value = value.max(0) as u64;
	Duration::new(
		value / HUNDRED_NANOS_PER_SECOND,
		((value % HUNDRED_NANOS_PER_SECOND) * 100) as u32,
	)
}

fn frame(sample: &IMFSample, layout: &Layout, timestamp: Duration) -> Result<VideoFrame, VideoError> {
	let data = mf::read_buffer(sample).map_err(failure)?;
	let nv12 = mf::packed_nv12(&data, layout.width, layout.height, layout.stride)
		.ok_or_else(|| VideoError::Decode("the decoder returned a short frame".to_owned()))?;
	Ok(VideoFrame {
		width: layout.width as u32,
		height: layout.height as u32,
		nv12,
		timestamp,
	})
}

fn floats(sample: &IMFSample) -> Result<Vec<f32>, VideoError> {
	let data = mf::read_buffer(sample).map_err(failure)?;
	let (whole, _) = data.as_chunks::<FLOAT_BYTES>();
	Ok(whole
		.iter()
		.map(|bytes| f32::from_le_bytes(*bytes))
		.map(|value| {
			if value.is_finite() {
				value.clamp(-1., 1.)
			} else {
				0.
			}
		})
		.collect())
}
