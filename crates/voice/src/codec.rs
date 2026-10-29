use opus::{Application, Bitrate, Channels, Decoder, Encoder};

use crate::error::Result;

pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME_SAMPLES: usize = 960;
pub const STEREO_FRAME_SAMPLES: usize = FRAME_SAMPLES * 2;
pub const SILENCE_FRAME: [u8; 3] = [0xf8, 0xff, 0xfe];

pub(crate) const DEFAULT_BITRATE_BPS: u32 = 64_000;
const SOUND_BITRATE_BPS: i32 = 128_000;
pub(crate) const MIN_BITRATE_BPS: u32 = 6_000;
pub(crate) const MAX_BITRATE_BPS: u32 = 510_000;
const EXPECTED_LOSS_PERCENT: i32 = 10;
const MAX_PACKET_BYTES: usize = 1275;

pub struct VoiceEncoder {
	inner: Encoder,
	packet: [u8; MAX_PACKET_BYTES],
}

impl VoiceEncoder {
	pub fn new() -> Result<Self> {
		let mut inner = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip)?;
		inner.set_bitrate(Bitrate::Bits(DEFAULT_BITRATE_BPS as i32))?;
		inner.set_inband_fec(true)?;
		inner.set_packet_loss_perc(EXPECTED_LOSS_PERCENT)?;
		inner.set_dtx(true)?;
		Ok(Self {
			inner,
			packet: [0; MAX_PACKET_BYTES],
		})
	}

	pub fn set_bitrate(&mut self, bps: u32) -> Result<()> {
		let bps = bps.clamp(MIN_BITRATE_BPS, MAX_BITRATE_BPS);
		self.inner
			.set_bitrate(Bitrate::Bits(i32::try_from(bps).unwrap_or(i32::MAX)))?;
		Ok(())
	}

	pub fn encode(&mut self, mono: &[f32]) -> Result<&[u8]> {
		let length = self.inner.encode_float(mono, &mut self.packet)?;
		Ok(&self.packet[..length])
	}
}

pub struct SoundEncoder {
	inner: Encoder,
	packet: [u8; MAX_PACKET_BYTES],
}

impl SoundEncoder {
	pub fn new() -> Result<Self> {
		let mut inner = Encoder::new(SAMPLE_RATE, Channels::Stereo, Application::Audio)?;
		inner.set_bitrate(Bitrate::Bits(SOUND_BITRATE_BPS))?;
		inner.set_inband_fec(true)?;
		inner.set_packet_loss_perc(EXPECTED_LOSS_PERCENT)?;
		Ok(Self {
			inner,
			packet: [0; MAX_PACKET_BYTES],
		})
	}

	pub fn encode(&mut self, stereo: &[f32]) -> Result<&[u8]> {
		let length = self.inner.encode_float(stereo, &mut self.packet)?;
		Ok(&self.packet[..length])
	}
}

pub struct VoiceDecoder {
	inner: Decoder,
}

impl VoiceDecoder {
	pub fn new() -> Result<Self> {
		Ok(Self {
			inner: Decoder::new(SAMPLE_RATE, Channels::Stereo)?,
		})
	}

	pub fn decode(&mut self, packet: &[u8], recover_previous: bool, stereo: &mut [f32]) -> Result<()> {
		let samples = self.inner.decode_float(packet, stereo, recover_previous)? * 2;
		stereo[samples..].fill(0.0);
		Ok(())
	}
}
