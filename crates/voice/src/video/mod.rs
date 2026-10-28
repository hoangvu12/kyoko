pub(crate) mod hub;
pub(crate) mod outgoing;
pub(crate) mod packetize;
pub(crate) mod receive;
pub(crate) mod rtp;
pub(crate) mod send;
pub(crate) mod wants;

use std::sync::Arc;

use protocol::Snowflake;

pub use ::video::{
	CameraInfo, CameraPreview, EncodedFrame, EncoderConfig, H264Decoder, H264Encoder, Picture, TestPattern,
	VideoCodecs, VideoError, VideoFrame, VideoQuality, VideoSource, available, cameras, fit, open_camera,
	scale_nv12,
};
pub use hub::VideoStats;
pub use outgoing::VideoLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoStream {
	pub user_id: Snowflake,
	pub ssrc: u32,
}

pub trait VideoSink: Send + Sync {
	fn frame(&self, stream: VideoStream, frame: Arc<VideoFrame>);

	fn ended(&self, _user_id: Snowflake) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoWant {
	pub user_id: Snowflake,
	pub quality: u8,
	pub pixels: u64,
}
