use protocol::fingerprint::CHROME_MAJOR;
use wreq::header::OrigHeaderMap;
use wreq::http2::{StreamDependency, StreamId};
use wreq::{ClientBuilder, Emulation, IntoEmulation};
use wreq_util::Profile;

use crate::order::{API_ORDER, IMAGE_ORDER, UPGRADE_ORDER, orig_headers};
use crate::trust::Trust;

pub const CHROME_PROFILE: Profile = match profile(CHROME_MAJOR) {
	Some(profile) => profile,
	None => panic!("CHROME_MAJOR has no captured handshake profile; see docs/fingerprint.md"),
};

const DOCUMENT_WEIGHT: u8 = 255;
const FETCH_WEIGHT: u8 = 219;
const IMAGE_WEIGHT: u8 = 146;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
	Document,
	Fetch,
	Image,
}

pub const fn profile(major: u32) -> Option<Profile> {
	match major {
		154 | 156 => Some(Profile::Chrome154),
		_ => None,
	}
}

pub fn chrome_emulation(destination: Destination) -> Emulation {
	let mut emulation = CHROME_PROFILE.into_emulation();
	emulation.headers.clear();
	emulation.orig_headers = match destination {
		Destination::Document => OrigHeaderMap::new(),
		Destination::Fetch => orig_headers(&API_ORDER),
		Destination::Image => orig_headers(&IMAGE_ORDER),
	};
	if let Some(options) = emulation.http2_options.as_mut() {
		let weight = match destination {
			Destination::Document => DOCUMENT_WEIGHT,
			Destination::Fetch => FETCH_WEIGHT,
			Destination::Image => IMAGE_WEIGHT,
		};
		options.headers_stream_dependency = Some(StreamDependency::new(StreamId::zero(), weight, true));
	}
	emulation
}

pub fn websocket_emulation() -> Emulation {
	let mut emulation = chrome_emulation(Destination::Fetch);
	emulation.orig_headers = orig_headers(&UPGRADE_ORDER);
	if let Some(options) = emulation.tls_options.as_mut() {
		options.alps_protocols = None;
	}
	emulation
}

pub fn client_builder(destination: Destination, trust: &Trust) -> wreq::Result<ClientBuilder> {
	let builder = wreq::Client::builder()
		.emulation(chrome_emulation(destination))
		.no_proxy();
	trust.apply(builder)
}
