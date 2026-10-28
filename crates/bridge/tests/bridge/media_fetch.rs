use std::sync::Arc;

use media::{MediaError, MediaFetcher};
use mockcord::hello::{
	Capture, Connection, HelloListener, Mode, NameCase, TlsSummary, chrome_fixture, differences,
	missing_names, order_difference,
};
use parking_lot::Mutex;
use protocol::fingerprint::{CHROME_MAJOR, Fingerprint};
use session::Trust;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const IMAGE: &[u8] = b"image";
const CAP: usize = 16;
const OVER_CAP: usize = 64;
const MAX_REDIRECTS: usize = 5;

#[derive(Clone)]
struct Origin {
	port: u16,
	paths: Arc<Mutex<Vec<String>>>,
}

impl Origin {
	async fn start() -> Origin {
		let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
		let origin = Origin {
			port: listener.local_addr().expect("address").port(),
			paths: Arc::default(),
		};
		let serving = origin.clone();
		tokio::spawn(async move {
			while let Ok((stream, _)) = listener.accept().await {
				tokio::spawn(serving.clone().answer(stream));
			}
		});
		origin
	}

	fn url(&self, path: &str) -> String {
		format!("http://127.0.0.1:{}{path}", self.port)
	}

	fn times(&self, path: &str) -> usize {
		self.paths.lock().iter().filter(|seen| *seen == path).count()
	}

	async fn answer(self, mut stream: TcpStream) {
		let mut request = Vec::new();
		let mut buffer = [0u8; 1024];
		while !request.windows(4).any(|window| window == b"\r\n\r\n") {
			match stream.read(&mut buffer).await {
				Ok(0) | Err(_) => return,
				Ok(read) => request.extend_from_slice(&buffer[..read]),
			}
		}
		let text = String::from_utf8_lossy(&request).into_owned();
		let path = text.split(' ').nth(1).unwrap_or_default().to_owned();
		self.paths.lock().push(path.clone());
		let _ = stream.write_all(&self.response(&path)).await;
		let _ = stream.shutdown().await;
	}

	fn response(&self, path: &str) -> Vec<u8> {
		if let Some(left) = path
			.strip_prefix("/hop/")
			.and_then(|left| left.parse::<usize>().ok())
		{
			return match left {
				0 => found(IMAGE),
				_ => redirect(&format!("/hop/{}", left - 1)),
			};
		}
		match path {
			"/away" => redirect(&format!("http://localhost:{}/landed", self.port)),
			"/big" => found(&[b'x'; OVER_CAP]),
			"/big-unannounced" => chunked(&[b'x'; OVER_CAP]),
			_ => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
		}
	}
}

fn found(body: &[u8]) -> Vec<u8> {
	let mut response = format!(
		"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
		body.len()
	)
	.into_bytes();
	response.extend_from_slice(body);
	response
}

fn chunked(body: &[u8]) -> Vec<u8> {
	let mut response =
		b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
			.to_vec();
	for piece in body.chunks(8) {
		response.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
		response.extend_from_slice(piece);
		response.extend_from_slice(b"\r\n");
	}
	response.extend_from_slice(b"0\r\n\r\n");
	response
}

fn redirect(location: &str) -> Vec<u8> {
	format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
		.into_bytes()
}

fn fingerprint() -> Fingerprint {
	Fingerprint::web_client("en-US", 1)
}

fn loopback_fetcher(origin: &Origin) -> MediaFetcher {
	bridge::media_fetcher_builder(&fingerprint())
		.only_loopback(origin.port)
		.build()
		.expect("the fetcher builds")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
	headers
		.iter()
		.find(|(key, _)| key.eq_ignore_ascii_case(name))
		.map(|(_, value)| value.as_str())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redirects_are_followed_up_to_five_times_and_only_to_allowed_hosts() {
	let origin = Origin::start().await;
	let fetcher = loopback_fetcher(&origin);
	let followed = fetcher
		.fetch(&origin.url(&format!("/hop/{MAX_REDIRECTS}")))
		.await
		.expect("five redirects are followed");
	assert_eq!(&followed[..], IMAGE);
	let too_many = fetcher
		.fetch(&origin.url(&format!("/hop/{}", MAX_REDIRECTS + 1)))
		.await;
	assert!(matches!(too_many, Err(MediaError::Fetch(_))), "{too_many:?}");
	assert_eq!(origin.times("/hop/0"), 1, "the sixth redirect is never followed");
	let away = fetcher.fetch(&origin.url("/away")).await;
	assert!(matches!(away, Err(MediaError::Fetch(_))), "{away:?}");
	assert_eq!(
		origin.times("/landed"),
		0,
		"the disallowed redirect is never followed"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn outside_loopback_only_https_to_the_allowed_hosts_is_fetched() {
	let fetcher = bridge::media_fetcher(&fingerprint(), None).expect("the fetcher builds");
	for url in [
		"http://cdn.discordapp.com/avatars/1/a.webp",
		"https://cdn.discordapp.com:8443/avatars/1/a.webp",
		"https://user:pass@cdn.discordapp.com/avatars/1/a.webp",
		"https://example.invalid/a.webp",
		"http://127.0.0.1/a.webp",
	] {
		let refused = fetcher.fetch(url).await;
		assert!(
			matches!(refused, Err(MediaError::DisallowedHost(_))),
			"{url}: {refused:?}"
		);
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_body_over_the_cap_is_refused_with_or_without_a_declared_length() {
	let origin = Origin::start().await;
	let fetcher = bridge::media_fetcher_builder(&fingerprint())
		.only_loopback(origin.port)
		.max_bytes(CAP)
		.build()
		.expect("the fetcher builds");
	for path in ["/big", "/big-unannounced"] {
		let refused = fetcher.fetch(&origin.url(path)).await;
		assert!(
			matches!(refused, Err(MediaError::Limit(_))),
			"{path}: {refused:?}"
		);
	}
	let small = fetcher.fetch(&origin.url("/hop/0")).await.expect("a small image");
	assert_eq!(&small[..], IMAGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn media_fetches_present_chromes_client_hello_and_image_request() {
	let listener = HelloListener::start(Mode::Complete)
		.await
		.expect("the hello listener starts");
	let fetcher = bridge::media_fetcher_builder(&fingerprint())
		.allow_host(format!("localhost:{}", listener.port()))
		.trust(Trust::only(listener.root_pem()))
		.build()
		.expect("the fetcher builds");
	let body = fetcher
		.fetch(&format!("https://localhost:{}/fetch", listener.port()))
		.await
		.expect("the fetch completes over tls");
	assert_eq!(&body[..], b"ok");
	let connection = listener
		.connections()
		.into_iter()
		.find(|connection| connection.http2.is_some())
		.expect("an http2 connection");
	assert_hello_is_chromes(&connection);
	assert_eq!(connection.alpn.as_deref(), Some("h2"));
	let expected = chrome_fixture(CHROME_MAJOR, "https")
		.expect("a recorded fixture for CHROME_MAJOR")
		.http2
		.expect("the fixture recorded http2");
	let chrome_fetch = expected
		.requests
		.iter()
		.find(|request| request.path() == Some("/fetch"))
		.expect("the fixture recorded chrome's fetch()");
	let actual = connection.http2.expect("an http2 summary");
	assert_eq!(actual.akamai, expected.akamai);
	let request = actual
		.requests
		.iter()
		.find(|request| request.path() == Some("/fetch"))
		.expect("the image request");
	assert_eq!(request.pseudo_order, chrome_fetch.pseudo_order);
	let chrome_image = chrome_fixture(CHROME_MAJOR, "image")
		.expect("a recorded image fixture for CHROME_MAJOR")
		.http2
		.and_then(|summary| summary.requests.into_iter().next())
		.expect("the fixture recorded chrome's <img> request");
	assert_eq!(request.priority, chrome_image.priority);
	assert_eq!(request.flags, chrome_image.flags);
	let difference = order_difference(&chrome_image.headers, &request.headers, NameCase::Exact);
	assert!(
		difference.is_empty(),
		"the image request differs from chrome's: {difference:?}"
	);
	assert_eq!(
		missing_names(&chrome_image.headers, &request.headers, NameCase::Exact),
		Vec::<String>::new()
	);
	for name in [
		"accept",
		"accept-encoding",
		"sec-fetch-dest",
		"sec-fetch-mode",
		"sec-fetch-site",
		"sec-fetch-storage-access",
		"priority",
		"user-agent",
		"sec-ch-ua",
		"sec-ch-ua-mobile",
		"sec-ch-ua-platform",
	] {
		assert_eq!(
			header(&request.headers, name),
			header(&chrome_image.headers, name),
			"{name}"
		);
	}
	assert_eq!(header(&request.headers, "authorization"), None);
	assert_eq!(request.cookie_fields, 0);
}

fn assert_hello_is_chromes(connection: &Connection) {
	let expected = chrome_fixture(CHROME_MAJOR, "https").expect("a recorded fixture for CHROME_MAJOR");
	let actual = Capture {
		client: "kyoko".to_owned(),
		kind: "https".to_owned(),
		negotiated_alpn: None,
		tls: TlsSummary::of(connection.hello.as_ref().expect("a client hello")),
		http2: None,
		http1: None,
	};
	let found = differences(&expected, &actual);
	assert!(
		found.is_empty(),
		"the media client hello differs from chrome {CHROME_MAJOR}: {found:?}"
	);
}
