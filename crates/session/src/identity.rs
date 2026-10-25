use std::sync::Arc;
use std::time::Duration;

use protocol::Fingerprint;
use protocol::fingerprint::Clock;
use rest::api::build;
use rest::{ClientIdentity, CookieJar};
use store::Store;
use url::Url;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

pub fn client_identity(store: &Store) -> ClientIdentity {
	client_identity_with_clock(store, Clock::system())
}

pub fn client_identity_with_clock(store: &Store, clock: Clock) -> ClientIdentity {
	let (locale, build_number) = {
		let settings = store.settings.read();
		(settings.locale.clone(), settings.client.build_number)
	};
	let cookies = store
		.cookie_jar()
		.map_or_else(CookieJar::default, |saved| CookieJar::load(&saved));
	ClientIdentity::with_clock(
		Fingerprint::web_client(&locale, build_number),
		store.installation_id(),
		cookies,
		clock,
	)
}

pub fn discord_build_source() -> Url {
	Url::parse(protocol::fingerprint::APP_URL).expect("the app url is valid")
}

pub(crate) async fn discover_build_number(identity: &ClientIdentity, store: &Arc<Store>, source: &Url) {
	let Ok(client) = build::discovery_client(identity) else {
		return;
	};
	let discovered =
		tokio::time::timeout(DISCOVERY_TIMEOUT, build::discover_build_number(&client, source)).await;
	let Ok(Ok(build_number)) = discovered else {
		tracing::debug!("build number discovery failed, keeping the last known build");
		return;
	};
	tracing::info!(build_number, "discovered the web client build number");
	identity.set_build_number(build_number);
	if store.settings.read().client.build_number == build_number {
		return;
	}
	let store = store.clone();
	tokio::task::spawn_blocking(move || {
		if let Err(error) = store.update_settings(|settings| settings.client.build_number = build_number) {
			tracing::warn!(%error, "discovered build number was not saved");
		}
	});
}

pub(crate) fn save_cookies(identity: &ClientIdentity, store: &Store) {
	if let Err(error) = store.save_cookie_jar(&identity.cookies().save()) {
		tracing::warn!(%error, "cookies were not saved");
	}
}

pub(crate) async fn keep_cookies_saved(identity: ClientIdentity, store: Arc<Store>) {
	loop {
		identity.cookies().changed().await;
		let identity = identity.clone();
		let store = store.clone();
		let _ = tokio::task::spawn_blocking(move || save_cookies(&identity, &store)).await;
	}
}

pub(crate) fn save_installation_id(
	identity: &ClientIdentity,
	store: &Arc<Store>,
	runtime: &tokio::runtime::Handle,
	id: &str,
) {
	if !identity.adopt_installation_id(id) {
		return;
	}
	let store = store.clone();
	let id = id.to_owned();
	runtime.spawn_blocking(move || {
		if let Err(error) = store.save_installation_id(&id) {
			tracing::warn!(%error, "installation id was not saved");
		}
	});
}
