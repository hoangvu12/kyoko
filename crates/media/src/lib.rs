pub mod blurhash;
pub mod cache;
pub mod cdn;
mod decode;
mod error;
pub mod fetch;
mod gallery;
mod lottie;
pub mod placeholder;
mod queue;
mod resize;
mod service;
pub mod shape;
pub mod signed;
mod types;

pub use cache::{DiskCache, SizeHint};
pub use decode::{Limits, Plan, Prepare, decode, decode_planned};
pub use error::{MediaError, Result, redact_url};
pub use fetch::{MediaFetcher, MediaFetcherBuilder, Purpose};
pub use gallery::{
	ANIMATION_SHARE, Animations, DEFAULT_BUDGET_BYTES, Gallery, GalleryStats, ImageName, ImageRequest,
	Origin, Pixels, Playback, Status, Update,
};
pub use placeholder::placeholder_from_discord;
pub use queue::{HTTP1_PER_HOST, HTTP2_PER_HOST, Protocol, Rank, Ticket};
pub use resize::{resize_decoded, resize_to_fit, shrink_image};
pub use service::{DEFAULT_DECODE_WORKERS, MediaService, Source, Variant};
pub use shape::Shape;
pub use types::{Animation, Decoded, DecodedImage, Frame, Replay, Streamed};
