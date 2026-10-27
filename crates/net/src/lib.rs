use wreq::header::OrigHeaderMap;
use wreq::{ClientBuilder, Emulation, IntoEmulation};
use wreq_util::Profile;

pub const CHROME_PROFILE: Profile = Profile::Chrome154;

pub fn chrome_emulation() -> Emulation {
	let mut emulation = CHROME_PROFILE.into_emulation();
	emulation.headers.clear();
	emulation.orig_headers = OrigHeaderMap::new();
	emulation
}

pub fn websocket_emulation() -> Emulation {
	let mut emulation = chrome_emulation();
	if let Some(options) = emulation.tls_options.as_mut() {
		options.alps_protocols = None;
	}
	emulation
}

pub fn client_builder() -> ClientBuilder {
	wreq::Client::builder().emulation(chrome_emulation()).no_proxy()
}
