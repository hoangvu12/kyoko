use openh264::OpenH264API;
use openh264::decoder::Decoder;
use openh264::encoder::{BitRate, Encoder, EncoderConfig, FrameRate};
use openh264::formats::{YUVBuffer, YUVSource};

use crate::text;

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

pub fn round_trip() -> Result<String, String> {
	let config = EncoderConfig::new()
		.bitrate(BitRate::from_bps(500_000))
		.max_frame_rate(FrameRate::from_hz(30.0));
	let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), config).map_err(text)?;
	let mut decoder = Decoder::new().map_err(text)?;
	let pixels = WIDTH * HEIGHT * 3 / 2;
	let frame = YUVBuffer::from_vec(vec![128; pixels], WIDTH, HEIGHT);
	let bitstream = encoder.encode(&frame).map_err(text)?.to_vec();
	let decoded = decoder
		.decode(&bitstream)
		.map_err(text)?
		.map(|image| image.dimensions())
		.ok_or_else(|| "the keyframe did not decode".to_owned())?;
	Ok(format!(
		"{WIDTH}x{HEIGHT} keyframe took {} bytes, decoded at {}x{}",
		bitstream.len(),
		decoded.0,
		decoded.1
	))
}
