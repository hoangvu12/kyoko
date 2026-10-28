use std::sync::Arc;
use std::time::{Duration, Instant};

use media::{
	Gallery, ImageName, ImageRequest, Limits, MediaFetcher, MediaService, Protocol, Shape, SizeHint, Status,
	Variant,
};
use mockcord::hello::{Connection, HelloListener, IMAGE_PATH, Mode, chrome_fixture};
use parking_lot::Mutex;
use protocol::fingerprint::{CHROME_MAJOR, Fingerprint};
use session::Trust;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

const WAIT: Duration = Duration::from_secs(10);
const BURST: usize = 10;
const PIXEL: [u8; 43] = [
	0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
	0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
	0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

#[derive(Clone)]
struct HeldOrigin {
	port: u16,
	arrived: Arc<Mutex<Vec<String>>>,
	release: Arc<Semaphore>,
}

impl HeldOrigin {
	async fn start() -> HeldOrigin {
		let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
		let origin = HeldOrigin {
			port: listener.local_addr().expect("address").port(),
			arrived: Arc::default(),
			release: Arc::new(Semaphore::new(0)),
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

	fn arrived(&self) -> Vec<String> {
		self.arrived.lock().clone()
	}

	fn release_all(&self) {
		self.release.add_permits(Semaphore::MAX_PERMITS / 2);
	}

	async fn answer(self, mut stream: TcpStream) {
		loop {
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
			self.arrived.lock().push(path);
			let Ok(permit) = self.release.acquire().await else {
				return;
			};
			permit.forget();
			let mut response = format!(
				"HTTP/1.1 200 OK\r\nContent-Type: image/gif\r\nContent-Length: {}\r\n\r\n",
				PIXEL.len()
			)
			.into_bytes();
			response.extend_from_slice(&PIXEL);
			if stream.write_all(&response).await.is_err() {
				return;
			}
		}
	}
}

fn fingerprint() -> Fingerprint {
	Fingerprint::web_client("en-US", 1)
}

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn image(url: String) -> ImageRequest {
	ImageRequest::new(url, Variant::new(SizeHint::new(32, 32), Shape::Square))
}

async fn until(what: &str, ready: impl Fn() -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready() {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		tokio::time::sleep(Duration::from_millis(5)).await;
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_images_start_visible_first_in_request_order_and_drop_when_they_scroll_away() {
	let origin = HeldOrigin::start().await;
	let fetcher = bridge::media_fetcher_builder(&fingerprint())
		.only_loopback(origin.port)
		.per_host(1, 1)
		.build()
		.expect("the fetcher builds");
	let service = MediaService::new(fetcher.clone(), scratch("media-queue"), Limits::default());
	let cached = origin.url("/cached.gif");
	service.disk().put(&cached, &PIXEL).expect("a cached image");
	let gallery = Gallery::new(service, tokio::runtime::Handle::current());

	let plug = gallery.request(image(origin.url("/plug.gif")));
	until("the first request to hold the only slot", || {
		origin.arrived() == ["/plug.gif"]
	})
	.await;
	let cached = gallery.request(image(cached));
	until("the cached image to load while the slot is held", || {
		matches!(gallery.status(&cached), Some(Status::Ready { .. }))
	})
	.await;

	let names: Vec<ImageName> = (0..BURST)
		.map(|index| gallery.request(image(origin.url(&format!("/{index}.gif")))))
		.collect();
	let away = gallery.request(image(origin.url("/away.gif")));
	gallery.set_visible([plug.as_str(), names[6].as_str(), names[7].as_str(), away.as_str()]);
	gallery.set_visible([plug.as_str(), names[6].as_str(), names[7].as_str()]);
	until("the burst to queue behind the held request", || {
		fetcher.waiting() == BURST && gallery.status(&away) == Some(Status::Dropped)
	})
	.await;

	origin.release_all();
	until("every queued image to load", || {
		names
			.iter()
			.all(|name| matches!(gallery.status(name), Some(Status::Ready { .. })))
	})
	.await;
	let mut expected = vec!["/plug.gif".to_owned(), "/6.gif".to_owned(), "/7.gif".to_owned()];
	expected.extend(
		(0..BURST)
			.filter(|index| ![6, 7].contains(index))
			.map(|index| format!("/{index}.gif")),
	);
	assert_eq!(
		origin.arrived(),
		expected,
		"visible images go first, then the rest in the order they were asked for, and the one that scrolled away is never fetched"
	);

	gallery.set_visible([away.as_str()]);
	until("the dropped image to load once it is back on screen", || {
		matches!(gallery.status(&away), Some(Status::Ready { .. }))
	})
	.await;
	assert_eq!(origin.arrived().last().map(String::as_str), Some("/away.gif"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_burst_of_images_to_one_host_shares_one_http2_connection() {
	let listener = HelloListener::start(Mode::Complete)
		.await
		.expect("the hello listener starts");
	let fetcher: MediaFetcher = bridge::media_fetcher_builder(&fingerprint())
		.allow_host(format!("localhost:{}", listener.port()))
		.trust(Trust::only(listener.root_pem()))
		.build()
		.expect("the fetcher builds");
	let url = |index: usize| format!("https://localhost:{}{IMAGE_PATH}?n={index}", listener.port());
	let fetches = (0..BURST).map(|index| {
		let fetcher = fetcher.clone();
		let url = url(index);
		tokio::spawn(async move { fetcher.fetch(&url).await })
	});
	for fetch in fetches.collect::<Vec<_>>() {
		let _ = fetch.await.expect("the fetch task finishes");
	}
	assert_eq!(fetcher.protocol(&url(0)), Protocol::Http2);
	let connections: Vec<Connection> = listener
		.connections()
		.into_iter()
		.filter(|connection| connection.hello.is_some())
		.collect();
	assert_eq!(
		connections.len(),
		1,
		"one TLS connection for the whole burst, as Chrome opens one per origin"
	);
	let requests = connections[0]
		.http2
		.as_ref()
		.expect("an http2 connection")
		.requests
		.clone();
	assert_eq!(requests.len(), BURST, "every image went over that connection");
	let chrome_image = chrome_fixture(CHROME_MAJOR, "image")
		.expect("a recorded image fixture for CHROME_MAJOR")
		.http2
		.and_then(|summary| summary.requests.into_iter().next())
		.expect("the fixture recorded chrome's <img> request");
	for request in &requests {
		assert_eq!(
			request.priority.as_ref().map(|priority| priority.weight),
			chrome_image.priority.as_ref().map(|priority| priority.weight),
			"every image keeps Chrome's weight"
		);
	}
}
