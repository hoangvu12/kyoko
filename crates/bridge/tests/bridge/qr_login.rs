use bridge::{QrEnd, QrEvents, QrSignIn, QrStage, Screen, Status};
use login::{LoginError, QrConfig, QrEvent, QrLogin};
use mockcord::hello::{
	Capture, Connection, HelloListener, Mode, NameCase, TlsSummary, chrome_fixture, differences,
	missing_names, order_difference,
};
use mockcord::{Reply, RestRequest};
use protocol::fingerprint::{CHROME_MAJOR, WEB_ORIGIN};
use protocol::{Snowflake, Token};
use session::Trust;
use tokio::time::timeout;
use url::Url;

use super::sign_in::{Desk, STEP_TIMEOUT, eventually};

const EXCHANGE: &str = "/users/@me/remote-auth/login";

fn qr_for(desk: &Desk) -> QrSignIn {
	let gateway = Url::parse(&desk.server.remote_auth_url()).expect("remote auth url");
	QrSignIn::new(desk.flow.gate().qr_config(gateway))
}

async fn step(qr: &mut QrSignIn, events: &mut QrEvents) -> Option<Token> {
	let event = timeout(STEP_TIMEOUT, events.events.recv())
		.await
		.expect("the QR login moves on in time")
		.expect("the QR login sends another event");
	qr.apply(events.generation, event)
}

async fn until_code(qr: &mut QrSignIn, events: &mut QrEvents) -> String {
	assert_eq!(qr.stage(), &QrStage::Waiting);
	assert!(step(qr, events).await.is_none());
	match qr.stage() {
		QrStage::Code { url } => url.clone(),
		other => panic!("expected a code, got {other:?}"),
	}
}

async fn until_ended(qr: &mut QrSignIn, events: &mut QrEvents) -> QrEnd {
	while !matches!(qr.stage(), QrStage::Ended(_)) {
		assert!(step(qr, events).await.is_none());
	}
	match qr.stage() {
		QrStage::Ended(end) => *end,
		_ => unreachable!(),
	}
}

async fn until_shown(desk: &Desk, codes: usize) {
	let control = desk.server.control();
	eventually("the phone to see the code", || control.qr_codes_shown() >= codes).await;
}

fn exchanges(desk: &Desk) -> Vec<RestRequest> {
	desk.server
		.control()
		.recording()
		.requests
		.into_iter()
		.filter(|request| request.is("POST", EXCHANGE))
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scanning_shows_the_user_before_the_phone_confirms_and_then_signs_in() {
	let mut desk = Desk::open("qr-full").await;
	desk.flow.launch(None);
	let mut qr = qr_for(&desk);
	assert!(
		qr.follow(&desk.flow).is_none(),
		"no code before the notice is accepted"
	);
	desk.acknowledge();
	let mut events = qr.follow(&desk.flow).expect("the login screen asks for a code");
	let url = until_code(&mut qr, &mut events).await;
	let fingerprint = url
		.strip_prefix("https://discord.com/ra/")
		.expect("the code is a discord.com remote auth link");
	assert_eq!(fingerprint.len(), 43);

	until_shown(&desk, 1).await;
	desk.server.control().scan_qr_code();
	assert!(step(&mut qr, &mut events).await.is_none());
	let QrStage::Scanned(scanner) = qr.stage().clone() else {
		panic!("expected the scanning user, got {:?}", qr.stage());
	};
	let fixture = &desk.server.fixtures().current_user;
	assert_eq!(scanner.id, fixture.id);
	assert_eq!(scanner.username, fixture.username);
	assert_eq!(scanner.avatar, fixture.avatar);
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(exchanges(&desk).is_empty());

	desk.server.control().confirm_qr_login();
	let token = step(&mut qr, &mut events)
		.await
		.expect("the confirmed login yields a token");
	assert_eq!(qr.stage(), &QrStage::SigningIn);
	assert_eq!(token.as_str(), desk.server.token());
	let check = desk.flow.redeem(token).expect("the login screen takes the token");
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Checking));
	assert!(qr.follow(&desk.flow).is_none());
	let failure = desk.flow.redeemed(check.await);
	assert_eq!(failure, None);
	qr.signed_in(failure);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	assert!(qr.follow(&desk.flow).is_none());
	assert_eq!(qr.stage(), &QrStage::Idle);
	desk.connected().await;
	assert_eq!(desk.user_id(), Snowflake(100));
	assert_eq!(desk.store.accounts.current(), Some(Snowflake(100)));

	let exchange = exchanges(&desk);
	assert_eq!(exchange.len(), 1);
	let request = &exchange[0];
	assert_eq!(request.header("authorization"), None);
	assert_eq!(request.header("referer"), Some("https://discord.com/login"));
	assert_eq!(request.header("origin"), Some("https://discord.com"));
	assert!(request.header("x-super-properties").is_some());
	let body = request.json().expect("a json body");
	assert!(body["ticket"].as_str().is_some_and(|ticket| !ticket.is_empty()));
	assert_eq!(body.as_object().map(|fields| fields.len()), Some(1));
	let socket = &desk.server.control().qr_connections()[0];
	let header = |name: &str| {
		socket
			.iter()
			.find(|(key, _)| key.eq_ignore_ascii_case(name))
			.map(|(_, value)| value.clone())
	};
	assert_eq!(header("origin").as_deref(), Some("https://discord.com"));
	assert_eq!(
		header("user-agent"),
		request.header("user-agent").map(str::to_owned)
	);
	assert_eq!(header("authorization"), None);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_on_the_phone_ends_the_code_with_a_message_and_a_refresh_starts_over() {
	let mut desk = Desk::open("qr-cancel").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut qr = qr_for(&desk);
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	until_code(&mut qr, &mut events).await;
	until_shown(&desk, 1).await;
	desk.server.control().scan_qr_code();
	assert!(step(&mut qr, &mut events).await.is_none());
	desk.server.control().cancel_qr_login();
	let end = until_ended(&mut qr, &mut events).await;
	assert_eq!(end, QrEnd::Cancelled);
	assert_eq!(end.message(), Some("The login was cancelled on your phone."));
	assert!(
		qr.follow(&desk.flow).is_none(),
		"an ended code waits for a refresh"
	);
	assert!(desk.flow.session().is_none());
	assert!(exchanges(&desk).is_empty());

	let mut events = qr.refresh(&desk.flow).expect("a refresh asks for a new code");
	until_code(&mut qr, &mut events).await;
	assert_eq!(desk.server.control().qr_connections().len(), 2);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_code_says_so_and_a_refresh_brings_a_new_one() {
	let mut desk = Desk::open("qr-expired").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut qr = qr_for(&desk);
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	let first = until_code(&mut qr, &mut events).await;
	until_shown(&desk, 1).await;
	desk.server.control().expire_qr_code();
	let end = until_ended(&mut qr, &mut events).await;
	assert_eq!(end, QrEnd::Expired);
	assert!(!end.is_error());

	let mut events = qr.refresh(&desk.flow).expect("a refresh asks for a new code");
	let second = until_code(&mut qr, &mut events).await;
	assert_ne!(first, second, "a new code has a new key");
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_on_the_ticket_exchange_is_reported_once_and_nothing_is_saved() {
	let mut desk = Desk::open("qr-captcha").await;
	desk.flow.launch(None);
	desk.acknowledge();
	desk.server
		.control()
		.respond_once("POST", EXCHANGE, Reply::captcha_required());
	let mut qr = qr_for(&desk);
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	until_code(&mut qr, &mut events).await;
	until_shown(&desk, 1).await;
	desk.server.control().scan_qr_code();
	desk.server.control().confirm_qr_login();
	let end = until_ended(&mut qr, &mut events).await;
	assert_eq!(end, QrEnd::Captcha);
	assert!(end.is_error());
	assert!(end.message().is_some_and(|message| message.contains("captcha")));
	assert!(
		end.message()
			.is_some_and(|message| message.contains("discord.com"))
	);
	assert!(end.offers_web_login());
	assert!(!QrEnd::Expired.offers_web_login());
	let exchange = exchanges(&desk);
	assert_eq!(exchange.len(), 1, "a captcha is never retried");
	assert!(exchange[0].injected);
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(desk.store.accounts.list().is_empty());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_nonce_proof_ends_the_handshake_before_any_code() {
	let mut desk = Desk::open("qr-nonce").await;
	desk.flow.launch(None);
	desk.acknowledge();
	desk.server.control().garble_next_qr_nonce();
	let mut qr = qr_for(&desk);
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	assert!(step(&mut qr, &mut events).await.is_none());
	assert_eq!(qr.stage(), &QrStage::Ended(QrEnd::Broken));
	assert_eq!(desk.server.control().qr_codes_shown(), 0);
	assert!(exchanges(&desk).is_empty());

	let mut events = qr.refresh(&desk.flow).expect("a refresh tries again");
	until_code(&mut qr, &mut events).await;
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_the_login_screen_stops_the_code() {
	let mut desk = Desk::open("qr-leave").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut qr = qr_for(&desk);
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	until_code(&mut qr, &mut events).await;
	until_shown(&desk, 1).await;
	let token = desk.server.token().to_owned();
	let check = desk.flow.submit(token).expect("the token form still works");
	let checked = check.await;
	desk.flow.checked(checked);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	assert!(qr.follow(&desk.flow).is_none());
	assert_eq!(qr.stage(), &QrStage::Idle);
	let control = desk.server.control();
	eventually("the remote auth socket to close", || {
		control.qr_codes_shown() == 0
	})
	.await;
	desk.stop().await;
}

async fn listen(mode: Mode) -> HelloListener {
	HelloListener::start(mode)
		.await
		.expect("the hello listener starts")
}

fn tls_config(desk: &Desk, listener: &HelloListener, trust: Trust) -> QrConfig {
	let gateway = Url::parse(&format!("wss://localhost:{}/?v=2", listener.port())).expect("a wss url");
	QrConfig {
		trust,
		..desk.flow.gate().qr_config(gateway)
	}
}

async fn first_connection(listener: &HelloListener, found: impl Fn(&Connection) -> bool) -> Connection {
	eventually("the remote auth socket to reach the listener", || {
		listener.connections().iter().any(&found)
	})
	.await;
	listener
		.connections()
		.into_iter()
		.find(|connection| found(connection))
		.expect("a recorded connection")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
	headers
		.iter()
		.find(|(key, _)| key.eq_ignore_ascii_case(name))
		.map(|(_, value)| value.as_str())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_remote_auth_socket_presents_chromes_websocket_client_hello() {
	let mut desk = Desk::open("qr-tls-hello").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let listener = listen(Mode::HelloOnly).await;
	let mut qr = QrSignIn::new(tls_config(&desk, &listener, Trust::default()));
	let mut events = qr.follow(&desk.flow).expect("a code is asked for");
	let connection = first_connection(&listener, |connection| connection.hello.is_some()).await;
	let expected = chrome_fixture(CHROME_MAJOR, "wss").expect("a recorded fixture for CHROME_MAJOR");
	let actual = Capture {
		client: "kyoko".to_owned(),
		kind: "wss".to_owned(),
		negotiated_alpn: None,
		tls: TlsSummary::of(connection.hello.as_ref().expect("a client hello")),
		http2: None,
		http1: None,
	};
	let found = differences(&expected, &actual);
	assert!(
		found.is_empty(),
		"the remote auth client hello differs from chrome {CHROME_MAJOR}: {found:?}"
	);
	assert_eq!(until_ended(&mut qr, &mut events).await, QrEnd::Network);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_remote_auth_upgrade_over_tls_names_the_http_status() {
	let mut desk = Desk::open("qr-tls-refused").await;
	desk.flow.launch(None);
	let listener = listen(Mode::Complete).await;
	let mut login = QrLogin::start(tls_config(&desk, &listener, Trust::only(listener.root_pem())));
	let mut events = login.events().expect("the login hands out its events");
	let event = timeout(STEP_TIMEOUT, events.recv())
		.await
		.expect("the QR login ends in time")
		.expect("the QR login reports how it ended");
	let QrEvent::Failed(LoginError::Network(reason)) = event else {
		panic!("expected a network failure, got {event:?}");
	};
	assert!(reason.contains("404"), "the reason names the status: {reason}");
	let connection = first_connection(&listener, |connection| connection.http1.is_some()).await;
	assert_eq!(connection.alpn.as_deref(), Some("http/1.1"));
	let upgrade = connection.http1.expect("an upgrade request");
	let chrome = chrome_fixture(CHROME_MAJOR, "wss")
		.expect("a recorded fixture for CHROME_MAJOR")
		.http1
		.expect("the fixture recorded the upgrade");
	assert!(upgrade.request_line.starts_with("GET /?v=2 "));
	assert_eq!(
		header(&upgrade.headers, "user-agent"),
		header(&chrome.headers, "user-agent")
	);
	assert_eq!(header(&upgrade.headers, "origin"), Some(WEB_ORIGIN));
	assert!(header(&upgrade.headers, "accept-language").is_some());
	assert_eq!(header(&upgrade.headers, "upgrade"), Some("websocket"));
	assert_eq!(header(&upgrade.headers, "authorization"), None);
	let cross_site = chrome_fixture(CHROME_MAJOR, "wss-cross-site")
		.expect("a recorded cross-site upgrade for CHROME_MAJOR")
		.http1
		.expect("the fixture recorded the upgrade");
	let difference = order_difference(&cross_site.headers, &upgrade.headers, NameCase::Exact);
	assert!(
		difference.is_empty(),
		"the upgrade differs from chrome's: {difference:?}"
	);
	assert_eq!(
		upgrade.headers.first().map(|(name, _)| name.as_str()),
		Some("Host")
	);
	assert_eq!(
		missing_names(&cross_site.headers, &upgrade.headers, NameCase::Exact),
		["Sec-WebSocket-Extensions"]
	);
	for name in [
		"Pragma",
		"Cache-Control",
		"Accept-Encoding",
		"Upgrade",
		"Sec-WebSocket-Version",
	] {
		assert_eq!(
			header(&upgrade.headers, name),
			header(&cross_site.headers, name),
			"{name}"
		);
	}
	assert_eq!(header(&upgrade.headers, "Connection"), Some("upgrade"));
	desk.stop().await;
}
