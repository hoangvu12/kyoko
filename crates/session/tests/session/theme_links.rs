use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
use session::{Command, ThemeLinkCopy, ThemeLinkTuning};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::aws_lc_rs;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::harness::{Options, Running};

const THEME: &str = "/**\n * @name Ocean\n */\n:root { --background-primary: #0b1d2a; }\n";
const THEME_PATH: &str = "/ocean.css";
const LIMIT: usize = 4 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(3);
const SLOW_ANSWER: Duration = Duration::from_secs(10);
const FORBIDDEN_HEADERS: [&str; 6] = [
	"authorization",
	"cookie",
	"x-super-properties",
	"x-discord-locale",
	"x-discord-timezone",
	"x-debug-options",
];

type Request = (String, Vec<(String, String)>);

#[derive(Clone, Default)]
struct Served {
	requests: Arc<Mutex<Vec<Request>>>,
	broken: Arc<AtomicBool>,
}

impl Served {
	fn asked_for(&self, path: &str) -> bool {
		self.requests.lock().iter().any(|(seen, _)| seen == path)
	}
}

struct TestCertificates {
	root_pem: String,
	server: CertificateDer<'static>,
	key: PrivateKeyDer<'static>,
}

fn certificates() -> TestCertificates {
	let root_key = KeyPair::generate().expect("root key");
	let mut root_params = CertificateParams::new(Vec::<String>::new()).expect("root params");
	root_params
		.distinguished_name
		.push(DnType::CommonName, "kyoko test root");
	root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
	let root = root_params.self_signed(&root_key).expect("root certificate");
	let issuer = Issuer::new(root_params, root_key);
	let server_key = KeyPair::generate().expect("server key");
	let mut server_params =
		CertificateParams::new(vec!["localhost".to_owned(), "127.0.0.1".to_owned()]).expect("server params");
	server_params
		.distinguished_name
		.push(DnType::CommonName, "localhost");
	let server = server_params
		.signed_by(&server_key, &issuer)
		.expect("server certificate");
	TestCertificates {
		root_pem: root.pem(),
		server: server.der().clone(),
		key: PrivateKeyDer::try_from(server_key.serialize_der()).expect("server key der"),
	}
}

fn tls(certificates: &TestCertificates) -> TlsAcceptor {
	let certs = vec![certificates.server.clone()];
	let key = certificates.key.clone_key();
	let config = ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
		.with_safe_default_protocol_versions()
		.expect("protocol versions")
		.with_no_client_auth()
		.with_single_cert(certs, key)
		.expect("server config");
	TlsAcceptor::from(Arc::new(config))
}

async fn serve(secure: bool, certificates: &TestCertificates) -> (SocketAddr, Served) {
	let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
	let address = listener.local_addr().expect("address");
	let served = Served::default();
	let state = served.clone();
	let acceptor = tls(certificates);
	tokio::spawn(async move {
		while let Ok((stream, _)) = listener.accept().await {
			let state = state.clone();
			let acceptor = acceptor.clone();
			tokio::spawn(async move {
				if secure {
					if let Ok(stream) = acceptor.accept(stream).await {
						answer(stream, state).await;
					}
				} else {
					answer(stream, state).await;
				}
			});
		}
	});
	(address, served)
}

async fn answer<S: AsyncReadExt + AsyncWriteExt + Unpin>(mut stream: S, served: Served) {
	let mut request = Vec::new();
	let mut buffer = [0u8; 1024];
	while !request.windows(4).any(|window| window == b"\r\n\r\n") {
		match stream.read(&mut buffer).await {
			Ok(0) | Err(_) => return,
			Ok(read) => request.extend_from_slice(&buffer[..read]),
		}
	}
	let text = String::from_utf8_lossy(&request).into_owned();
	let mut lines = text.split("\r\n");
	let path = lines
		.next()
		.and_then(|line| line.split(' ').nth(1))
		.unwrap_or_default()
		.to_owned();
	let headers: Vec<(String, String)> = lines
		.filter_map(|line| line.split_once(':'))
		.map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
		.collect();
	served.requests.lock().push((path.clone(), headers));
	let (status, body, length) = match path.as_str() {
		"/ocean.css" if served.broken.load(Ordering::SeqCst) => {
			("500 Internal Server Error", String::new(), true)
		}
		"/ocean.css" => ("200 OK", THEME.to_owned(), true),
		"/huge.css" => ("200 OK", ":root{}".repeat(LIMIT), true),
		"/endless.css" => ("200 OK", ":root{}".repeat(LIMIT), false),
		"/slow.css" => {
			tokio::time::sleep(SLOW_ANSWER).await;
			("200 OK", THEME.to_owned(), true)
		}
		"/page" => ("200 OK", "<!DOCTYPE html><html></html>".to_owned(), true),
		_ => ("404 Not Found", String::new(), true),
	};
	let mut head = format!("HTTP/1.1 {status}\r\nContent-Type: text/css\r\nConnection: close\r\n");
	if length {
		head.push_str(&format!("Content-Length: {}\r\n", body.len()));
	}
	head.push_str("\r\n");
	let _ = stream.write_all(head.as_bytes()).await;
	let _ = stream.write_all(body.as_bytes()).await;
	let _ = stream.shutdown().await;
}

fn tuning(certificates: &TestCertificates) -> ThemeLinkTuning {
	ThemeLinkTuning {
		max_bytes: LIMIT,
		timeout: FETCH_TIMEOUT,
		trusted_roots: vec![certificates.root_pem.clone().into_bytes()],
	}
}

async fn refreshed(running: &Running, urls: &[String]) -> Vec<ThemeLinkCopy> {
	let before = running.session().theme_links();
	running
		.session()
		.send(Command::RefreshThemeLinks { urls: urls.to_vec() });
	running
		.eventually("the theme links to be fetched", || {
			let now = running.session().theme_links();
			!Arc::ptr_eq(&now, &before) && now.len() == urls.len()
		})
		.await;
	running.session().theme_links().as_ref().clone()
}

fn copy<'a>(copies: &'a [ThemeLinkCopy], url: &str) -> &'a ThemeLinkCopy {
	copies
		.iter()
		.find(|copy| copy.url == url)
		.unwrap_or_else(|| panic!("no copy for {url}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn theme_links_are_fetched_over_https_only_without_any_discord_credentials() {
	let certificates = certificates();
	let (secure, served) = serve(true, &certificates).await;
	let (plain, plain_served) = serve(false, &certificates).await;
	let running =
		Running::start_with(Options::new("theme-links-https").theme_links(tuning(&certificates))).await;
	running.connected().await;
	let good = format!("https://localhost:{}{THEME_PATH}", secure.port());
	let insecure = format!("http://127.0.0.1:{}{THEME_PATH}", plain.port());
	let page = format!("https://localhost:{}/page", secure.port());
	let copies = refreshed(&running, &[good.clone(), insecure.clone(), page.clone()]).await;

	assert_eq!(
		copy(&copies, &good).text.as_deref(),
		Some(THEME),
		"{:?}",
		copy(&copies, &good).error
	);
	assert_eq!(copy(&copies, &good).error, None);
	assert_eq!(
		copy(&copies, &insecure).error.as_deref(),
		Some("only https links are allowed")
	);
	assert!(
		!plain_served.asked_for(THEME_PATH),
		"a plain http link was requested"
	);
	assert!(
		copy(&copies, &page)
			.error
			.as_deref()
			.is_some_and(|error| error.contains("web page"))
	);

	assert!(served.asked_for(THEME_PATH));
	let token = running.token().to_owned();
	for (path, headers) in served.requests.lock().iter() {
		for (name, value) in headers {
			assert!(
				!FORBIDDEN_HEADERS.contains(&name.as_str()),
				"{path} was sent the {name} header"
			);
			assert!(!value.contains(&token), "{path} was sent the token in {name}");
		}
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_or_slow_theme_link_fails_alone() {
	let certificates = certificates();
	let (secure, _served) = serve(true, &certificates).await;
	let running =
		Running::start_with(Options::new("theme-links-limits").theme_links(tuning(&certificates))).await;
	running.connected().await;
	let good = format!("https://localhost:{}/ocean.css", secure.port());
	let huge = format!("https://localhost:{}/huge.css", secure.port());
	let endless = format!("https://localhost:{}/endless.css", secure.port());
	let slow = format!("https://localhost:{}/slow.css", secure.port());
	let started = std::time::Instant::now();
	let copies = refreshed(
		&running,
		&[huge.clone(), slow.clone(), good.clone(), endless.clone()],
	)
	.await;

	assert!(
		started.elapsed() < SLOW_ANSWER,
		"the slow link held up the others"
	);
	assert_eq!(
		copy(&copies, &good).text.as_deref(),
		Some(THEME),
		"{:?}",
		copy(&copies, &good).error
	);
	for oversized in [&huge, &endless] {
		let failed = copy(&copies, oversized);
		assert_eq!(failed.text, None);
		assert!(
			failed
				.error
				.as_deref()
				.is_some_and(|error| error.contains("larger than 4 KB")),
			"{oversized}: {:?}",
			failed.error
		);
	}
	assert_eq!(
		copy(&copies, &slow).error.as_deref(),
		Some("the server took too long to answer")
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_last_good_copy_of_a_theme_link_is_used_when_it_fails() {
	let certificates = certificates();
	let (secure, served) = serve(true, &certificates).await;
	let running =
		Running::start_with(Options::new("theme-links-cache").theme_links(tuning(&certificates))).await;
	running.connected().await;
	let good = format!("https://localhost:{}/ocean.css", secure.port());
	let fresh = refreshed(&running, std::slice::from_ref(&good)).await;
	assert_eq!(
		copy(&fresh, &good).text.as_deref(),
		Some(THEME),
		"{:?}",
		copy(&fresh, &good).error
	);
	assert!(!copy(&fresh, &good).cached);

	served.broken.store(true, Ordering::SeqCst);
	let fallback = refreshed(&running, std::slice::from_ref(&good)).await;
	let kept = copy(&fallback, &good);
	assert_eq!(kept.text.as_deref(), Some(THEME));
	assert!(kept.cached);
	assert_eq!(
		kept.error.as_deref(),
		Some("the server answered 500 Internal Server Error")
	);

	let offline = session::cached_copies(
		&running.store().paths.theme_links_dir(),
		std::slice::from_ref(&good),
	);
	assert_eq!(offline[0].text.as_deref(), Some(THEME));
	assert!(offline[0].cached);
	running.stop().await;
}
