mod clip;
mod deck;
mod decode;
mod error;
mod opus;
mod output;
mod playhead;

pub use clip::Clip;
pub use deck::{Deck, DeckStatus};
pub use decode::{DEFAULT_MAX_SAMPLES, Limits, decode, duration};
pub use error::{AudioError, Result};
pub use output::{Output, SILENT_RATE, Sink};
