use std::sync::Arc;

use bytes::Bytes;
use mockcord::hello::{
	Capture, Connection, HelloListener, Http2Request, Mode, NameCase, TlsSummary, chrome_fixture,
	differences, missing_names, order_difference,
};
use protocol::fingerprint::{ACCEPT_ENCODING, CHROME_MAJOR, WEB_ORIGIN};
use protocol::{ChannelType, Snowflake};
use rest::CookieJar;
use session::{Command, InviteSource, Trust, UploadFile};
use store::{MemoryKeyring, Paths, Store};
use url::Url;

use crate::harness::{Options, Running};

const NAVIGATION_ONLY: [&str; 2] = ["upgrade-insecure-requests", "sec-fetch-user"];
const NOT_SENT_BY_KYOKO: [&str; 1] = ["x-debug-options"];
const UNOFFERED_EXTENSIONS: &str = "Sec-WebSocket-Extensions";
const HTTP1_ONLY: [&str; 1] = ["host"];
const ORDER_INVITE: &str = "kyoko-order";
const AUDIT_REASON: &str = "header order";
const SEEDED_COOKIES: [&str; 2] = [
	"kyoko_a=1; Path=/; Secure; Max-Age=3600",
	"kyoko_b=2; Path=/; Secure; Max-Age=3600",
];

fn navigation_only(headers: &[(String, String)]) -> Vec<String> {
	headers
		.iter()
		.filter(|(name, value)| {
			let name = name.to_ascii_lowercase();
			NAVIGATION_ONLY.contains(&name.as_str())
				|| (name == "sec-fetch-mode" && value.eq_ignore_ascii_case("navigate"))
				|| (name == "accept" && value.starts_with("text/html"))
		})
		.map(|(name, value)| format!("{name}: {value}"))
		.collect()
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
	headers
		.iter()
		.find(|(key, _)| key.eq_ignore_ascii_case(name))
		.map(|(_, value)| value.as_str())
}

async fn listen(mode: Mode) -> HelloListener {
	HelloListener::start(mode)
		.await
		.expect("the hello listener starts")
}

fn origin(listener: &HelloListener) -> String {
	format!("https://localhost:{}", listener.port())
}

fn api_base(listener: &HelloListener) -> Url {
	Url::parse(&format!("{}/api/v9", origin(listener))).expect("the api base is a url")
}

fn seed_cookies(paths: &Paths, keyring: Arc<MemoryKeyring>, origin: &str) {
	let jar = CookieJar::default();
	jar.adopt(SEEDED_COOKIES, &Url::parse(origin).expect("the origin is a url"));
	let store = Store::open_with(paths.clone(), keyring).expect("the seed store opens");
	store.save_cookie_jar(&jar.save()).expect("the cookies are saved");
}

async fn request_history(running: &Running) -> Snowflake {
	running.connected().await;
	let (guild_id, channel_id) = running
		.session()
		.state()
		.read()
		.guilds()
		.find_map(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.kind == ChannelType::GuildText)
				.map(|channel| (guild.id, channel.id))
		})
		.expect("a text channel in the fixtures");
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	channel_id
}

async fn moderation_target(running: &Running) -> (Snowflake, Snowflake) {
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	let user_ids: Vec<Snowflake> = running.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.collect();
	running
		.until("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	running
		.session()
		.send(Command::RequestMembers { guild_id, user_ids });
	running
		.until("the first guild with its members", |state| {
			state
				.guild(guild_id)
				.is_some_and(|guild| guild.members.len() > 3 && guild.owner_id.is_some())
		})
		.await;
	let state = running.session().state().read();
	let me = state.current_user_id().expect("signed in");
	let target = state
		.guild(guild_id)
		.expect("the guild is loaded")
		.members
		.iter()
		.map(|(id, _)| *id)
		.find(|id| state.outranks(guild_id, me, *id))
		.expect("a member below the current user");
	(guild_id, target)
}

fn api_kind(request: &Http2Request) -> (String, bool, bool) {
	(
		header(&request.headers, ":method").unwrap_or_default().to_owned(),
		header(&request.headers, "x-context-properties").is_some(),
		header(&request.headers, "x-audit-log-reason").is_some(),
	)
}

fn http2_requests(listener: &HelloListener) -> Vec<Http2Request> {
	listener
		.connections()
		.into_iter()
		.filter_map(|connection| connection.http2)
		.flat_map(|summary| summary.requests)
		.collect()
}

async fn connection_where(
	running: &Running,
	listener: &HelloListener,
	what: &str,
	found: impl Fn(&Connection) -> bool,
) -> Connection {
	running
		.eventually(what, || listener.connections().iter().any(&found))
		.await;
	listener
		.connections()
		.into_iter()
		.find(|connection| found(connection))
		.expect(what)
}

fn assert_hello_is_chromes(connection: &Connection, kind: &str) {
	let expected = chrome_fixture(CHROME_MAJOR, kind).expect("a recorded fixture for CHROME_MAJOR");
	let actual = Capture {
		client: "kyoko".to_owned(),
		kind: kind.to_owned(),
		negotiated_alpn: None,
		tls: TlsSummary::of(connection.hello.as_ref().expect("a client hello")),
		http2: None,
		http1: None,
	};
	let found = differences(&expected, &actual);
	assert!(
		found.is_empty(),
		"the {kind} client hello differs from chrome {CHROME_MAJOR}: {found:?}"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_requests_present_chromes_client_hello() {
	let listener = listen(Mode::HelloOnly).await;
	let running = Running::start_with(Options::new("tls-rest-hello").rest_base(api_base(&listener))).await;
	request_history(&running).await;
	let connection = connection_where(&running, &listener, "a rest client hello", |connection| {
		connection.hello.is_some()
	})
	.await;
	assert_hello_is_chromes(&connection, "https");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rest_requests_speak_chromes_http2_with_split_cookies() {
	let listener = listen(Mode::Complete).await;
	let seeded_origin = origin(&listener);
	let keyring = Arc::new(MemoryKeyring::new());
	let seed_keyring = keyring.clone();
	let running = Running::start_with(
		Options::new("tls-rest-h2")
			.rest_base(api_base(&listener))
			.trust(Trust::only(listener.root_pem()))
			.keyring(keyring)
			.seed(move |paths| seed_cookies(paths, seed_keyring, &seeded_origin)),
	)
	.await;
	request_history(&running).await;
	let connection = connection_where(&running, &listener, "an http2 GET", |connection| {
		connection.http2.as_ref().is_some_and(|summary| {
			summary
				.requests
				.iter()
				.any(|request| header(&request.headers, ":method") == Some("GET"))
		})
	})
	.await;
	assert_eq!(connection.alpn.as_deref(), Some("h2"));
	let expected = chrome_fixture(CHROME_MAJOR, "https")
		.expect("a recorded fixture for CHROME_MAJOR")
		.http2
		.expect("the fixture recorded http2");
	let fetch = expected
		.requests
		.iter()
		.find(|request| request.path() == Some("/fetch"))
		.expect("the fixture recorded chrome's fetch()");
	let actual = connection.http2.expect("an http2 summary");
	assert_eq!(actual.akamai, expected.akamai);
	assert_eq!(actual.settings, expected.settings);
	assert_eq!(actual.window_update, expected.window_update);
	assert_eq!(actual.priority_frames, expected.priority_frames);
	assert_eq!(actual.frames[..3], expected.frames[..3]);
	let request = actual
		.requests
		.iter()
		.find(|request| header(&request.headers, ":method") == Some("GET"))
		.expect("a GET request");
	assert_eq!(request.flags, fetch.flags);
	assert_eq!(request.priority, fetch.priority);
	assert_eq!(request.pseudo_order, fetch.pseudo_order);
	assert_eq!(request.cookie_fields, SEEDED_COOKIES.len());
	assert_eq!(request.cookie_fields, fetch.cookie_fields);
	assert_eq!(navigation_only(&request.headers), Vec::<String>::new());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_navigation_header_reaches_mockcord_on_any_rest_request() {
	let running = Running::start("tls-no-navigation-headers").await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: "with a file".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: vec![UploadFile::new(
			"notes.txt",
			Bytes::from_static(b"notes"),
			"text/plain",
		)],
	});
	running
		.until("the message with a file to post", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty()
					&& cache
						.ordered
						.iter()
						.any(|message| message.content == "with a file")
			})
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(recording.requests_to("PUT", "/upload/**").count(), 1);
	assert!(recording.requests.len() > 3);
	for request in &recording.requests {
		assert_eq!(
			navigation_only(&request.headers),
			Vec::<String>::new(),
			"{} {}",
			request.method,
			request.path
		);
	}
	for connection in &recording.connections {
		assert_eq!(navigation_only(&connection.headers), Vec::<String>::new());
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_gateway_presents_chromes_websocket_client_hello() {
	let listener = listen(Mode::HelloOnly).await;
	let running = Running::start_with(
		Options::new("tls-gateway-hello").gateway_url(format!("wss://localhost:{}", listener.port())),
	)
	.await;
	let connection = connection_where(&running, &listener, "a gateway client hello", |connection| {
		connection.hello.is_some()
	})
	.await;
	assert_hello_is_chromes(&connection, "wss");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_gateway_upgrade_runs_over_tls_with_the_web_client_headers() {
	let listener = listen(Mode::Complete).await;
	let running = Running::start_with(
		Options::new("tls-gateway-upgrade")
			.gateway_url(format!("wss://localhost:{}", listener.port()))
			.trust(Trust::only(listener.root_pem())),
	)
	.await;
	let connection = connection_where(&running, &listener, "a gateway upgrade", |connection| {
		connection.http1.is_some()
	})
	.await;
	assert_eq!(connection.alpn.as_deref(), Some("http/1.1"));
	let upgrade = connection.http1.expect("an upgrade request");
	let chrome = chrome_fixture(CHROME_MAJOR, "wss")
		.expect("a recorded fixture for CHROME_MAJOR")
		.http1
		.expect("the fixture recorded the upgrade");
	assert!(upgrade.request_line.starts_with("GET /?v="));
	assert_eq!(
		header(&upgrade.headers, "user-agent"),
		header(&chrome.headers, "user-agent")
	);
	assert_eq!(header(&upgrade.headers, "origin"), Some(WEB_ORIGIN));
	assert!(header(&upgrade.headers, "accept-language").is_some());
	assert_eq!(header(&upgrade.headers, "upgrade"), Some("websocket"));
	assert_eq!(navigation_only(&upgrade.headers), Vec::<String>::new());
	assert_upgrade_is_chromes(&upgrade.headers);
	running.stop().await;
}

fn assert_upgrade_is_chromes(headers: &[(String, String)]) {
	let chrome = chrome_fixture(CHROME_MAJOR, "wss-cross-site")
		.expect("a recorded cross-site upgrade for CHROME_MAJOR")
		.http1
		.expect("the fixture recorded the upgrade");
	let difference = order_difference(&chrome.headers, headers, NameCase::Exact);
	assert!(
		difference.is_empty(),
		"the upgrade differs from chrome's: {difference:?}"
	);
	assert_eq!(headers.first().map(|(name, _)| name.as_str()), Some("Host"));
	assert_eq!(
		missing_names(&chrome.headers, headers, NameCase::Exact),
		[UNOFFERED_EXTENSIONS]
	);
	for name in [
		"Pragma",
		"Cache-Control",
		"Accept-Encoding",
		"Upgrade",
		"Sec-WebSocket-Version",
		"User-Agent",
	] {
		assert_eq!(header(headers, name), header(&chrome.headers, name), "{name}");
	}
	assert_eq!(header(headers, "Accept-Encoding"), Some(ACCEPT_ENCODING));
	assert_eq!(header(headers, "Connection"), Some("upgrade"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_requests_send_their_headers_in_chromes_order() {
	let listener = listen(Mode::Complete).await;
	let seeded_origin = origin(&listener);
	let keyring = Arc::new(MemoryKeyring::new());
	let seed_keyring = keyring.clone();
	let running = Running::start_with(
		Options::new("tls-rest-header-order")
			.rest_base(api_base(&listener))
			.trust(Trust::only(listener.root_pem()))
			.keyring(keyring)
			.seed(move |paths| seed_cookies(paths, seed_keyring, &seeded_origin)),
	)
	.await;
	let channel_id = request_history(&running).await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: "in chrome's order".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	running.session().send(Command::AcceptInvite {
		code: ORDER_INVITE.to_owned(),
		from: InviteSource::AddServer,
	});
	let (guild_id, user_id) = moderation_target(&running).await;
	running.session().send(Command::TimeoutMember {
		guild_id,
		user_id,
		until: Some(chrono::Utc::now() + chrono::TimeDelta::minutes(10)),
		reason: Some(AUDIT_REASON.to_owned()),
	});
	let chrome: Vec<Http2Request> = chrome_fixture(CHROME_MAJOR, "https")
		.expect("a recorded fixture for CHROME_MAJOR")
		.http2
		.expect("the fixture recorded http2")
		.requests
		.into_iter()
		.filter(|request| request.path().is_some_and(|path| path.starts_with("/api/")))
		.collect();
	assert_eq!(chrome.len(), 4, "the fixture's GET, POST, join POST and PATCH");
	running
		.eventually("a request of every recorded api kind", || {
			let sent = http2_requests(&listener);
			chrome
				.iter()
				.all(|recorded| sent.iter().any(|request| api_kind(request) == api_kind(recorded)))
		})
		.await;
	let sent = http2_requests(&listener);
	for recorded in &chrome {
		let kind = api_kind(recorded);
		let matching: Vec<&Http2Request> = sent.iter().filter(|request| api_kind(request) == kind).collect();
		for request in &matching {
			let difference = order_difference(&recorded.headers, &request.headers, NameCase::Exact);
			assert!(
				difference.is_empty(),
				"{kind:?} {:?} differs from chrome's order: {difference:?}",
				request.path()
			);
		}
		let triggered = matching
			.iter()
			.rev()
			.find(|request| {
				request
					.path()
					.is_some_and(|path| path.contains("/messages") || kind.0 != "GET")
			})
			.expect("the request this test triggered");
		assert_eq!(
			missing_names(&recorded.headers, &triggered.headers, NameCase::Exact),
			NOT_SENT_BY_KYOKO,
			"{kind:?}"
		);
		assert_eq!(triggered.cookie_fields, SEEDED_COOKIES.len());
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uploads_send_their_headers_in_chromes_api_order() {
	let running = Running::start("tls-upload-header-order").await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: "with a file".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: vec![UploadFile::new(
			"notes.txt",
			Bytes::from_static(b"notes"),
			"text/plain",
		)],
	});
	running
		.until_wire("the upload", |recording| {
			recording.requests_to("PUT", "/upload/**").next().cloned()
		})
		.await;
	let recording = running.mock().recording();
	let upload = recording
		.requests_to("PUT", "/upload/**")
		.next()
		.expect("the upload");
	let order: Vec<(String, String)> = net::API_ORDER
		.iter()
		.map(|name| ((*name).to_owned(), String::new()))
		.collect();
	let headers: Vec<(String, String)> = upload
		.headers
		.iter()
		.filter(|(name, _)| !HTTP1_ONLY.contains(&name.as_str()))
		.cloned()
		.collect();
	let difference = order_difference(&order, &headers, NameCase::Ignored);
	assert!(difference.is_empty(), "{difference:?}");
	running.stop().await;
}
