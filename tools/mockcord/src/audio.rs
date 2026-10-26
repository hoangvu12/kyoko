use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};

pub const VOICE_MESSAGE: &[u8] = include_bytes!("../fixtures/audio/voice-message.ogg");
pub const VOICE_REPLY: &[u8] = include_bytes!("../fixtures/audio/voice-reply.ogg");
pub const AUDIO_FILE: &[u8] = include_bytes!("../fixtures/audio/rainy-loop.mp3");
pub const VOICE_MESSAGE_FILENAME: &str = "voice-message.ogg";
pub const VOICE_REPLY_FILENAME: &str = "voice-reply.ogg";
pub const AUDIO_FILENAME: &str = "rainy-loop.mp3";
pub const BROKEN_AUDIO_FILENAME: &str = "broken-voice.ogg";
pub const VOICE_MESSAGE_SECS: f64 = 8.;
pub const VOICE_REPLY_SECS: f64 = 7.;
pub const AUDIO_FILE_SECS: f64 = 84.;
pub const AUDIO_FILE_SIZE: i64 = 2_411_725;
pub const VOICE_WAVEFORM: &str = "TIDMmeb/smaZ2YBMc7LyzIxZmb/mpmZMgLLZ/7+AWYzMmWZMc1lAMw==";
pub const OGG: &str = "audio/ogg";
pub const MP3: &str = "audio/mpeg";
const BROKEN: &[u8] = b"OggS this is not really a voice message";
const BROKEN_MARK: &str = "broken";

pub fn is_audio(file: &str) -> bool {
	kind_of(file).is_some()
}

pub fn content_type(file: &str) -> Option<&'static str> {
	kind_of(file)
}

pub fn respond(file: &str) -> Option<Response> {
	let kind = kind_of(file)?;
	let bytes = match fixture(file) {
		Fixture::Voice => VOICE_MESSAGE,
		Fixture::Reply => VOICE_REPLY,
		Fixture::File => AUDIO_FILE,
		Fixture::Broken => BROKEN,
	};
	Some(([(CONTENT_TYPE, kind)], bytes).into_response())
}

pub fn seconds_of(file: &str) -> f64 {
	match fixture(file) {
		Fixture::Reply => VOICE_REPLY_SECS,
		Fixture::File => AUDIO_FILE_SECS,
		Fixture::Voice | Fixture::Broken => VOICE_MESSAGE_SECS,
	}
}

pub fn size_of(file: &str) -> usize {
	match fixture(file) {
		Fixture::Voice => VOICE_MESSAGE.len(),
		Fixture::Reply => VOICE_REPLY.len(),
		Fixture::File => AUDIO_FILE.len(),
		Fixture::Broken => BROKEN.len(),
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fixture {
	Voice,
	Reply,
	File,
	Broken,
}

fn fixture(file: &str) -> Fixture {
	if file.contains(BROKEN_MARK) {
		Fixture::Broken
	} else if kind_of(file) == Some(MP3) {
		Fixture::File
	} else if file.contains("reply") {
		Fixture::Reply
	} else {
		Fixture::Voice
	}
}

fn kind_of(file: &str) -> Option<&'static str> {
	let (_, extension) = file.rsplit_once('.')?;
	match extension.to_ascii_lowercase().as_str() {
		"ogg" | "oga" | "opus" => Some(OGG),
		"mp3" => Some(MP3),
		_ => None,
	}
}
