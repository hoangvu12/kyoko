use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::clip::{Clip, frames_to_duration};
use crate::feed::Feed;
use crate::playhead::Playhead;

type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckStatus {
	pub id: u64,
	pub position: Duration,
	pub duration: Duration,
	pub playing: bool,
	pub ended: bool,
	pub speed: f32,
	pub muted: bool,
}

#[derive(Clone, Default)]
pub struct Deck {
	inner: Arc<Mutex<Inner>>,
	notify: Arc<Mutex<Option<Notify>>>,
	feed: Arc<Mutex<Option<Feed>>>,
}

#[derive(Default)]
struct Inner {
	voice: Option<Voice>,
	rendered: u64,
}

struct Voice {
	id: u64,
	clip: Arc<Clip>,
	head: Playhead,
	playing: bool,
	ended: bool,
	muted: bool,
}

impl Voice {
	fn status(&self) -> DeckStatus {
		DeckStatus {
			id: self.id,
			position: frames_to_duration(self.head.position(), self.clip.rate()).min(self.clip.duration()),
			duration: self.clip.duration(),
			playing: self.playing,
			ended: self.ended,
			speed: self.head.speed(),
			muted: self.muted,
		}
	}
}

impl Deck {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn set_notify(&self, notify: impl Fn() + Send + Sync + 'static) {
		*self.notify.lock() = Some(Arc::new(notify));
	}

	pub fn load(&self, id: u64, clip: Arc<Clip>, at: Duration, speed: f32) {
		let position = (at.as_secs_f64() * f64::from(clip.rate())).min(clip.frames() as f64);
		self.inner.lock().voice = Some(Voice {
			id,
			clip,
			head: Playhead::new(position, speed),
			playing: false,
			ended: false,
			muted: false,
		});
	}

	pub fn unload(&self) {
		self.inner.lock().voice = None;
	}

	pub fn loaded(&self) -> Option<u64> {
		self.inner.lock().voice.as_ref().map(|voice| voice.id)
	}

	pub fn play(&self) -> bool {
		let mut inner = self.inner.lock();
		let Some(voice) = inner.voice.as_mut() else {
			return false;
		};
		if voice.ended || voice.head.position() >= voice.clip.frames() as f64 {
			voice.head.seek(0.);
			voice.ended = false;
		}
		voice.playing = true;
		true
	}

	pub fn pause(&self) {
		if let Some(voice) = self.inner.lock().voice.as_mut() {
			voice.playing = false;
		}
	}

	pub fn seek(&self, at: Duration) {
		if let Some(voice) = self.inner.lock().voice.as_mut() {
			let frames = voice.clip.frames() as f64;
			voice
				.head
				.seek((at.as_secs_f64() * f64::from(voice.clip.rate())).min(frames));
			voice.ended = false;
		}
	}

	pub fn set_speed(&self, speed: f32) {
		if let Some(voice) = self.inner.lock().voice.as_mut() {
			voice.head.set_speed(speed);
		}
	}

	pub fn set_muted(&self, muted: bool) {
		if let Some(voice) = self.inner.lock().voice.as_mut() {
			voice.muted = muted;
		}
	}

	pub fn status(&self) -> Option<DeckStatus> {
		self.inner.lock().voice.as_ref().map(Voice::status)
	}

	pub fn is_playing(&self) -> bool {
		self.inner
			.lock()
			.voice
			.as_ref()
			.is_some_and(|voice| voice.playing)
	}

	pub fn rendered_frames(&self) -> u64 {
		self.inner.lock().rendered
	}

	pub fn set_feed(&self, feed: Option<Feed>) {
		*self.feed.lock() = feed;
	}

	pub fn render(&self, out: &mut [f32], channels: u16, rate: u32) {
		out.fill(0.);
		let ended = self.render_voice(out, channels, rate);
		let feed = self.feed.try_lock().and_then(|feed| feed.clone());
		if let Some(feed) = feed {
			feed.mix(out, channels, rate);
		}
		if ended {
			self.notify();
		}
	}

	fn render_voice(&self, out: &mut [f32], channels: u16, rate: u32) -> bool {
		let channels = usize::from(channels.max(1));
		{
			let Some(mut inner) = self.inner.try_lock() else {
				return false;
			};
			let frames = out.len() / channels;
			inner.rendered += frames as u64;
			let Some(voice) = inner.voice.as_mut().filter(|voice| voice.playing) else {
				return false;
			};
			let gain = if voice.muted { 0. } else { 1. };
			let mut ended = false;
			for frame in out.chunks_exact_mut(channels) {
				let Some((left, right)) = voice.head.next(&voice.clip, rate) else {
					ended = true;
					break;
				};
				match frame {
					[mono] => *mono = (left + right) * 0.5 * gain,
					[first, second, ..] => {
						*first = left * gain;
						*second = right * gain;
					}
					[] => {}
				}
			}
			if ended {
				voice.playing = false;
				voice.ended = true;
				let end = voice.clip.frames() as f64;
				voice.head.seek(end);
			}
			ended
		}
	}

	fn notify(&self) {
		let notify = self.notify.lock().clone();
		if let Some(notify) = notify {
			notify();
		}
	}
}
