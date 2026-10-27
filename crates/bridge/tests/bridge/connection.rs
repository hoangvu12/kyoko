use std::cell::RefCell;
use std::time::{Duration, Instant};

use bridge::{Banner, Connection, ConnectionWatch, Tone};
use mockcord::MockConfig;
use session::{Backoff, GatewayTuning};
use state::State;

use crate::harness::Running;

const RESUMABLE_CLOSE: u16 = 4000;
const DISALLOWED_INTENTS: u16 = 4014;
const REFUSALS: usize = 4;
const SHORT_BACKOFF: Backoff = Backoff {
	base: Duration::from_millis(150),
	cap: Duration::from_millis(600),
};

#[derive(Default)]
struct Seen {
	watch: ConnectionWatch,
	phases: Vec<&'static str>,
	banners: Vec<Banner>,
}

impl Seen {
	fn observe(&mut self, state: &State) -> &Connection {
		let now = Instant::now();
		if self.watch.observe(&state.session().connection, now) {
			let name = self.watch.current().name();
			if self.phases.last() != Some(&name) {
				self.phases.push(name);
			}
			if let Some(banner) = self.watch.current().banner(now) {
				self.banners.push(banner);
			}
		}
		self.watch.current()
	}
}

async fn started(name: &str) -> Running {
	Running::start_tuned(
		name,
		MockConfig::default(),
		GatewayTuning {
			backoff: SHORT_BACKOFF,
			..GatewayTuning::default()
		},
	)
	.await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_connection_status_follows_a_drop_an_offline_period_and_the_reconnect() {
	let mut running = started("connection-drop").await;
	let seen = RefCell::new(Seen::default());
	running
		.until_state("the session to be live", |state| {
			*seen.borrow_mut().observe(state) == Connection::Live
		})
		.await;
	assert!(
		seen.borrow().watch.current().banner(Instant::now()).is_none(),
		"a live session shows no banner"
	);

	let control = running.server.control();
	control.close_next_connections(RESUMABLE_CLOSE, REFUSALS);
	control.close_gateway(RESUMABLE_CLOSE);
	running
		.until_state("the session to come back after the offline period", |state| {
			let mut seen = seen.borrow_mut();
			let live = *seen.observe(state) == Connection::Live;
			live && seen.phases.contains(&"offline")
		})
		.await;

	let seen = seen.into_inner();
	assert_eq!(seen.phases, ["live", "reconnecting", "offline", "live"]);
	assert!(
		seen.banners.iter().any(|banner| {
			banner.tone == Tone::Warning && banner.retry && banner.text.starts_with("Reconnecting in ")
		}),
		"no reconnect countdown among {:?}",
		seen.banners
	);
	assert!(
		seen.banners.iter().any(|banner| {
			banner.tone == Tone::Danger && banner.retry && banner.text.starts_with("You're offline.")
		}),
		"no offline banner among {:?}",
		seen.banners
	);
	assert!(seen.watch.current().banner(Instant::now()).is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_gateway_shows_an_offline_banner_without_retry() {
	let mut running = started("connection-stopped").await;
	let seen = RefCell::new(Seen::default());
	running
		.until_state("the session to be live", |state| {
			*seen.borrow_mut().observe(state) == Connection::Live
		})
		.await;
	running.server.control().close_gateway(DISALLOWED_INTENTS);
	running
		.until_state("the gateway to stop", |state| {
			matches!(seen.borrow_mut().observe(state), Connection::Stopped(_))
		})
		.await;
	let banner = seen
		.borrow()
		.watch
		.current()
		.banner(Instant::now())
		.expect("a stopped gateway shows a banner");
	assert_eq!(banner.tone, Tone::Danger);
	assert!(!banner.retry);
	assert!(banner.text.starts_with("Disconnected from Discord"));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_identify_on_an_open_socket_shows_no_banner() {
	let mut running = started("connection-invalid-session").await;
	let seen = RefCell::new(Seen::default());
	running
		.until_state("the session to be live", |state| {
			*seen.borrow_mut().observe(state) == Connection::Live
		})
		.await;
	let first = running.session().state().read().session().session_id.clone();
	running.server.control().invalidate_session(false);
	running
		.until_state("a new session on the same socket", |state| {
			seen.borrow_mut().observe(state);
			state.session().connected && state.session().session_id != first
		})
		.await;
	assert_eq!(seen.borrow().phases, ["live"]);
	assert!(seen.borrow().banners.is_empty(), "{:?}", seen.borrow().banners);
	running.stop().await;
}
