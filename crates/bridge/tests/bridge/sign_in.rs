use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bridge::{Failure, Flow, Gate, Handover, Remember, Screen, Start, StartSession, Status};
use mockcord::{ALT_USER_ID, MockConfig, Reply, Server};
use protocol::{Opcode, Snowflake};
use session::{Command, Session, SessionConfig};
use state::{RiskyReason, RiskyTarget};
use store::{Keyring, MemoryKeyring, Paths, Store};
use tokio::time::Instant;
use url::Url;

const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(10);
const CACHE_KEY: &str = "cache-key";

struct Desk {
	server: Server,
	keyring: Arc<MemoryKeyring>,
	store: Arc<Store>,
	scratch: PathBuf,
	flow: Flow,
}

impl Desk {
	async fn open(name: &str) -> Desk {
		Self::open_with(name, Arc::new(MemoryKeyring::new())).await
	}

	async fn open_with(name: &str, keyring: Arc<MemoryKeyring>) -> Desk {
		let server = Server::start(MockConfig {
			record: true,
			..MockConfig::default()
		})
		.await
		.expect("mock server starts");
		let scratch = std::env::temp_dir().join(format!("kyoko-sign-in-{name}-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&scratch);
		let store = open_store(&scratch, &keyring);
		let rest_base = Url::parse(server.rest_base_url()).expect("rest url");
		let flow = flow_for(&server, store.clone(), rest_base);
		Desk {
			server,
			keyring,
			store,
			scratch,
			flow,
		}
	}

	fn relaunch(&mut self) {
		let rest_base = Url::parse(self.server.rest_base_url()).expect("rest url");
		self.flow = flow_for(&self.server, self.store.clone(), rest_base);
		self.flow.launch(None);
	}

	fn token(&self) -> String {
		self.server.token().to_owned()
	}

	fn acknowledge(&self) {
		self.flow
			.gate()
			.acknowledge()
			.expect("the notice is acknowledged");
	}

	async fn submit(&mut self, pasted: &str) {
		let check = self
			.flow
			.submit(pasted.to_owned())
			.expect("the login screen takes the token");
		assert_eq!(self.flow.screen(), &Screen::Login(Status::Checking));
		let checked = check.await;
		self.flow.checked(checked);
	}

	fn session(&self) -> &Session {
		self.flow.session().expect("a session is running")
	}

	fn user_id(&self) -> Snowflake {
		self.session()
			.state()
			.read()
			.current_user_id()
			.expect("a signed-in user")
	}

	async fn hand_over(&mut self, handover: Handover) {
		assert_eq!(self.flow.screen(), &Screen::Switching);
		assert!(self.flow.session().is_none());
		let arrival = tokio::task::spawn_blocking(move || handover.finish())
			.await
			.expect("the old session stops");
		self.flow.arrived(arrival);
	}

	async fn add_alt_account(&mut self) {
		let handover = self.flow.add_account().expect("an account to add from");
		self.hand_over(handover).await;
		assert_eq!(self.flow.screen(), &Screen::Login(Status::Ready));
		assert!(self.flow.adding());
		let alt = self.server.alt_token().to_owned();
		self.submit(&alt).await;
		assert_eq!(self.flow.screen(), &Screen::Main);
		assert!(!self.flow.adding());
		self.connected().await;
	}

	async fn connected(&self) {
		let state = self.session().state().clone();
		eventually("the session to connect", || {
			let state = state.read();
			state.current_user().is_some() && state.session().connected
		})
		.await;
	}

	fn secrets_without_cache_key(&self) -> Vec<String> {
		self.keyring
			.users()
			.into_iter()
			.filter(|user| user != CACHE_KEY)
			.collect()
	}

	async fn stop(mut self) {
		if let Some(session) = self.flow.shutdown() {
			shut_down(session).await;
		}
		self.server.shutdown().await;
		let _ = std::fs::remove_dir_all(&self.scratch);
	}
}

fn cached_ready(store: &Store, user_id: Snowflake) -> bool {
	if !store.paths.account_database_path(user_id).exists() {
		return false;
	}
	let account = store.for_account(user_id).expect("the account cache opens");
	account.db.kv_get("session.ready").ok().flatten().is_some()
}

fn guild_count(session: &Session) -> usize {
	session.state().read().guilds().count()
}

fn identify_connections(server: &Server) -> Vec<(u64, serde_json::Value)> {
	server
		.control()
		.recording()
		.frames_with(Opcode::Identify)
		.map(|frame| (frame.connection, frame.data["properties"].clone()))
		.collect()
}

fn without_installation(properties: &serde_json::Value) -> serde_json::Value {
	let mut properties = properties.clone();
	if let Some(fields) = properties.as_object_mut() {
		fields.remove("installation_id");
	}
	properties
}

fn closed_normally(server: &Server, connection: u64) -> bool {
	server
		.control()
		.recording()
		.closes
		.iter()
		.any(|close| close.connection == connection && close.code == Some(1000))
}

fn open_store(scratch: &Path, keyring: &Arc<MemoryKeyring>) -> Arc<Store> {
	let paths = Paths::at(scratch).expect("scratch paths");
	Arc::new(Store::open_with(paths, keyring.clone()).expect("store opens"))
}

fn flow_for(server: &Server, store: Arc<Store>, rest_base: Url) -> Flow {
	let identity = session::client_identity(&store);
	let gate = Gate::new(store.clone(), identity.clone(), rest_base.clone());
	let gateway_url = server.gateway_url().to_owned();
	let start: StartSession = Box::new(
		move |Start {
		          token,
		          remember,
		          store,
		      }| {
			Session::start(SessionConfig {
				identity: identity.clone(),
				build_source: None,
				store,
				rest_base: rest_base.clone(),
				gateway_url: gateway_url.clone(),
				token,
				runtime: None,
				notify: Arc::new(|_| {}),
				hooks: None,
				gateway: Default::default(),
				rest: Default::default(),
				caches: Default::default(),
				alerts: Default::default(),
				retention: Default::default(),
				remember_account: remember == Remember::Account,
				theme_links: Default::default(),
			})
		},
	);
	Flow::new(gate, start)
}

async fn eventually(what: &str, done: impl Fn() -> bool) {
	let deadline = Instant::now() + STEP_TIMEOUT;
	while !done() {
		if Instant::now() >= deadline {
			panic!("timed out waiting for {what}");
		}
		tokio::time::sleep(POLL).await;
	}
}

async fn shut_down(session: Session) {
	tokio::task::spawn_blocking(move || session.shutdown())
		.await
		.expect("the session shuts down");
}

fn files_containing(root: &Path, secret: &str) -> Vec<PathBuf> {
	let mut found = Vec::new();
	let mut pending = vec![root.to_path_buf()];
	while let Some(dir) = pending.pop() {
		let Ok(entries) = std::fs::read_dir(&dir) else {
			continue;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			if path.is_dir() {
				pending.push(path);
			} else if std::fs::read(&path).is_ok_and(|bytes| {
				bytes
					.windows(secret.len())
					.any(|window| window == secret.as_bytes())
			}) {
				found.push(path);
			}
		}
	}
	found
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_saved_account_the_app_starts_at_the_login_screen() {
	let mut desk = Desk::open("fresh").await;
	desk.flow.launch(None);
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(!desk.flow.gate().acknowledged());
	assert!(desk.flow.submit(desk.token()).is_none());
	desk.acknowledge();
	assert!(desk.flow.gate().acknowledged());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_valid_token_is_saved_to_the_keyring_and_the_session_connects() {
	let mut desk = Desk::open("valid").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&format!("  \"{token}\"\n")).await;
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	let user_id = desk
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("a signed-in user");
	assert_eq!(desk.store.accounts.current(), Some(user_id));
	assert!(
		desk.keyring
			.secret(&format!("account-{user_id}"))
			.is_some_and(|stored| !stored.is_empty())
	);
	assert!(files_containing(&desk.scratch, &token).is_empty());

	let session = desk.flow.shutdown().expect("the session was running");
	shut_down(session).await;
	desk.relaunch();
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_token_shows_an_error_and_nothing_is_stored() {
	let mut desk = Desk::open("rejected").await;
	desk.acknowledge();
	desk.submit("not-the-mock-token").await;
	assert_eq!(
		desk.flow.screen(),
		&Screen::Login(Status::Failed(Failure::Rejected))
	);
	assert!(desk.flow.session().is_none());
	assert!(desk.secrets_without_cache_key().is_empty());
	assert!(desk.store.accounts.list().is_empty());
	assert!(desk.flow.gate().stored_token().is_none());

	desk.submit("   ").await;
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Failed(Failure::Empty)));
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_or_rate_limit_is_reported_once_without_retrying() {
	let mut desk = Desk::open("refused").await;
	desk.acknowledge();
	let mock = desk.server.control();
	let token = desk.token();
	for reply in [Reply::captcha_required(), Reply::rate_limited(0.5, false)] {
		mock.clear_recording();
		mock.respond_once("GET", "/users/@me", reply);
		desk.submit(&token).await;
		assert_eq!(
			desk.flow.screen(),
			&Screen::Login(Status::Failed(Failure::Refused))
		);
		tokio::time::sleep(Duration::from_millis(700)).await;
		let asked = mock.recording().requests_to("GET", "/users/@me").count();
		assert_eq!(asked, 1);
	}
	assert!(desk.secrets_without_cache_key().is_empty());
	assert!(desk.flow.session().is_none());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_discord_is_a_network_error() {
	let mut desk = Desk::open("network").await;
	let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
	let port = closed.local_addr().expect("the port").port();
	drop(closed);
	let rest_base = Url::parse(&format!("http://127.0.0.1:{port}/api/v9")).expect("rest url");
	desk.flow = flow_for(&desk.server, desk.store.clone(), rest_base);
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	assert_eq!(
		desk.flow.screen(),
		&Screen::Login(Status::Failed(Failure::Network))
	);
	assert!(desk.secrets_without_cache_key().is_empty());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_keyring_error_offers_this_session_only_and_writes_no_plaintext() {
	let keyring = Arc::new(MemoryKeyring::new());
	keyring
		.set(CACHE_KEY, &"07".repeat(32))
		.expect("the cache key is seeded");
	keyring.set_failing_writes(true);
	let mut desk = Desk::open_with("keyring", keyring).await;
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	let Screen::Login(status @ Status::Unsaved(_)) = desk.flow.screen().clone() else {
		panic!("expected the keyring error, got {:?}", desk.flow.screen());
	};
	assert!(status.is_error());
	assert!(desk.flow.session().is_none());
	desk.flow.continue_unsaved();
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	assert!(desk.secrets_without_cache_key().is_empty());
	assert!(!desk.store.paths.token_path().exists());
	assert!(files_containing(&desk.scratch, &token).is_empty());
	assert!(desk.flow.gate().stored_token().is_none());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_token_mid_session_returns_to_the_login_screen() {
	let mut desk = Desk::open("expired").await;
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	desk.connected().await;
	let state = desk.session().state().clone();
	assert!(desk.flow.observe(&state.read()).is_none());
	desk.server.control().close_gateway(4004);
	eventually("the session to report the rejected token", || {
		state.read().session().connection
			== state::ConnectionStatus::Disconnected(state::DisconnectReason::AuthenticationFailed)
	})
	.await;
	let session = desk
		.flow
		.observe(&state.read())
		.expect("the rejected session is handed back");
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Expired));
	shut_down(session).await;
	assert!(desk.flow.gate().stored_token().is_none());
	assert!(desk.secrets_without_cache_key().is_empty());
	desk.relaunch();
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signing_out_wipes_the_account_and_returns_to_the_login_screen() {
	let mut desk = Desk::open("sign-out").await;
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	desk.connected().await;
	let user_id = desk
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("a signed-in user");
	let store = desk.store.clone();
	eventually("the session to cache READY", || cached_ready(&store, user_id)).await;
	let sign_out = desk.flow.sign_out().expect("a session to sign out of");
	assert_eq!(desk.flow.screen(), &Screen::SigningOut);
	tokio::task::spawn_blocking(move || sign_out.finish())
		.await
		.expect("sign-out finishes");
	desk.flow.signed_out();
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::SignedOut));
	assert!(desk.flow.session().is_none());
	assert!(desk.store.accounts.list().is_empty());
	assert!(desk.store.accounts.token_for(user_id).ok().flatten().is_none());
	assert!(!cached_ready(&desk.store, user_id));
	assert!(desk.secrets_without_cache_key().is_empty());
	assert!(desk.flow.gate().acknowledged());
	desk.relaunch();
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_accounts_reconnects_as_the_other_user_with_nothing_shared() {
	let mut desk = Desk::open("switch").await;
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	desk.connected().await;
	let main = desk.user_id();
	let store = desk.store.clone();
	eventually("the first account to cache READY", || cached_ready(&store, main)).await;
	assert!(guild_count(desk.session()) > 0);
	let confirmed = (RiskyTarget::User(Snowflake(42)), RiskyReason::StrangerDm);
	desk.session().send(Command::ConfirmRisky {
		target: confirmed.0,
		reason: confirmed.1,
	});
	let state = desk.session().state().clone();
	eventually("the confirmation to apply", || {
		state.read().is_risky_confirmed(confirmed.0, confirmed.1)
	})
	.await;

	desk.add_alt_account().await;
	let alt = desk.user_id();
	assert_eq!(alt, Snowflake(ALT_USER_ID));
	assert_eq!(guild_count(desk.session()), 0);
	assert!(desk.session().state().read().dm_rows().is_empty());
	assert!(
		!desk
			.session()
			.state()
			.read()
			.is_risky_confirmed(confirmed.0, confirmed.1)
	);
	eventually("the second account to cache READY", || cached_ready(&store, alt)).await;
	assert_eq!(store.accounts.current(), Some(alt));
	let identifies = identify_connections(&desk.server);
	assert_eq!(identifies.len(), 2);
	assert_eq!(
		without_installation(&identifies[0].1),
		without_installation(&identifies[1].1)
	);
	assert!(closed_normally(&desk.server, identifies[0].0));
	for user_id in [main, alt] {
		assert!(
			desk.keyring
				.secret(&format!("account-{user_id}"))
				.is_some_and(|stored| !stored.is_empty())
		);
	}
	assert!(files_containing(&desk.scratch, &token).is_empty());
	assert!(files_containing(&desk.scratch, desk.server.alt_token()).is_empty());

	let entries = desk.flow.accounts();
	assert_eq!(entries.len(), 2);
	assert!(
		entries
			.iter()
			.any(|entry| entry.current && entry.account.user_id == alt)
	);
	assert!(desk.flow.switch(alt).is_none());
	desk.store
		.update_settings(|settings| settings.safety_locks = false)
		.expect("the setting saves");
	let handover = desk.flow.switch(main).expect("the first account to switch to");
	desk.hand_over(handover).await;
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	assert_eq!(desk.user_id(), main);
	assert!(guild_count(desk.session()) > 0);
	let (still_confirmed, locks) = {
		let state = desk.session().state().read();
		(
			state.is_risky_confirmed(confirmed.0, confirmed.1),
			state.safety_locks(),
		)
	};
	assert!(!still_confirmed);
	assert!(!locks);
	assert_eq!(store.accounts.current(), Some(main));
	let identifies = identify_connections(&desk.server);
	assert_eq!(identifies.len(), 3);
	assert_eq!(identifies[1].1, identifies[2].1);
	assert!(closed_normally(&desk.server, identifies[1].0));
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_removed_account_loses_its_token_and_cache_and_adding_can_be_cancelled() {
	let mut desk = Desk::open("remove").await;
	desk.acknowledge();
	let token = desk.token();
	desk.submit(&token).await;
	desk.connected().await;
	let main = desk.user_id();
	desk.add_alt_account().await;
	let alt = desk.user_id();
	let store = desk.store.clone();
	eventually("the second account to cache READY", || cached_ready(&store, alt)).await;
	let handover = desk.flow.switch(main).expect("the first account to switch to");
	desk.hand_over(handover).await;
	desk.connected().await;
	assert!(desk.flow.remove_account(main).is_none());
	let removal = desk
		.flow
		.remove_account(alt)
		.expect("the other account to remove");
	tokio::task::spawn_blocking(move || removal.finish())
		.await
		.expect("the removal finishes");
	assert!(desk.keyring.secret(&format!("account-{alt}")).is_none());
	assert!(!store.paths.account_database_path(alt).exists());
	let entries = desk.flow.accounts();
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0].account.user_id, main);
	assert!(entries[0].current);

	let handover = desk.flow.add_account().expect("an account to add from");
	desk.hand_over(handover).await;
	assert!(desk.flow.adding());
	desk.flow.cancel_add();
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	assert_eq!(desk.user_id(), main);
	desk.stop().await;
}
