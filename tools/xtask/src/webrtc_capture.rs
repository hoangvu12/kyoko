mod peer;
mod sdp;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mockcord::hello::{Capture, ClientHello, TlsSummary, differences};
use parking_lot::Mutex;

use crate::parity::browser;
use crate::tls_capture::chrome_label;
use peer::{Answerer, Backend, Dtls};
use sdp::OfferSummary;

const FIXTURES: &str = "tools/mockcord/fixtures/webrtc";
const POLL: Duration = Duration::from_millis(200);
const SETTLE: Duration = Duration::from_secs(3);
const PROFILE_CLEANUP_TRIES: usize = 20;
const MAX_BODY: usize = 64 * 1024;
const PAGE: &str = include_str!("webrtc_capture/page.html");

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Client {
	Chrome,
	Str0m,
}

#[derive(clap::Args)]
pub struct Args {
	#[arg(long, value_enum, default_value = "chrome")]
	client: Client,
	#[arg(long, value_enum, default_value = "wincrypto")]
	backend: Backend,
	#[arg(long, value_enum, default_value = "1.2")]
	dtls: Dtls,
	#[arg(long)]
	browser: Option<PathBuf>,
	#[arg(long)]
	record: Option<String>,
	#[arg(long)]
	compare: Option<String>,
	#[arg(long, default_value_t = 40)]
	wait: u64,
}

struct Captured {
	offer: String,
	summary: OfferSummary,
	dtls: Capture,
	hello_bytes: usize,
	hello_datagrams: usize,
	connected: bool,
}

pub fn run(args: Args) -> Result<()> {
	let root = workspace_root();
	let wait = Duration::from_secs(args.wait);
	let captured = match args.client {
		Client::Chrome => chrome(args.browser.as_deref(), args.backend, args.dtls, wait)?,
		Client::Str0m => str0m(args.backend, args.dtls, wait)?,
	};
	print(&captured)?;
	if let Some(name) = &args.record {
		record(&root, name, &captured)?;
	}
	if let Some(name) = &args.compare {
		compare(&root, name, &captured)?;
	}
	Ok(())
}

fn str0m(backend: Backend, dtls: Dtls, wait: Duration) -> Result<Captured> {
	let label = format!(
		"{} {} (str0m 0.24.1 offerer, in-process loopback)",
		backend.label(),
		dtls.label()
	);
	let run = peer::loopback(backend, dtls, wait)?;
	Ok(Captured {
		summary: sdp::summarize(&label, &run.offer),
		offer: run.offer,
		dtls: dtls_capture(&label, &run.hello),
		hello_bytes: run.hello_bytes,
		hello_datagrams: run.hello_datagrams,
		connected: run.connected,
	})
}

fn chrome(chosen: Option<&Path>, backend: Backend, dtls: Dtls, wait: Duration) -> Result<Captured> {
	let exe = browser::find(chosen)?;
	let label = chrome_label(&exe);
	let server = TcpListener::bind("127.0.0.1:0").context("listen on localhost")?;
	let port = server.local_addr()?.port();
	println!(
		"webrtc-capture: page on http://127.0.0.1:{port}/ only, str0m answerer {} {}",
		backend.label(),
		dtls.label()
	);
	let shared = Arc::new(Mutex::new(Page::default()));
	server.set_nonblocking(true)?;
	let mut browser = Browser::start(&exe, port)?;
	let deadline = Instant::now() + wait;
	let mut hello_at = None;
	while Instant::now() < deadline && browser.running() {
		accept(&server, &shared, backend, dtls);
		let page = shared.lock();
		if let Some(answerer) = &page.answerer {
			let observed = answerer.observed.lock();
			if observed.hello.is_some() && hello_at.is_none() {
				hello_at = Some(Instant::now());
			}
			let settled = hello_at.is_some_and(|at| at.elapsed() >= SETTLE);
			if observed.connected && page.states.iter().any(|state| state == "connected") || settled {
				break;
			}
		}
		drop(page);
		std::thread::sleep(POLL);
	}
	browser.stop();
	let mut page = Arc::try_unwrap(shared)
		.map_err(|_| anyhow::anyhow!("the page state is still shared"))?
		.into_inner();
	let Some(offer) = page.offer.take() else {
		bail!("{label} posted no offer");
	};
	let Some(answerer) = page.answerer.take() else {
		bail!("no str0m answerer started");
	};
	let observed = std::mem::take(&mut *answerer.observed.lock());
	drop(answerer);
	println!("webrtc-capture: page states {:?}", page.states);
	println!("webrtc-capture: str0m states {:?}", observed.states);
	println!("webrtc-capture: {} datagrams from Chrome", observed.datagrams);
	if let Some(error) = &observed.error {
		println!("webrtc-capture: str0m stopped with {error}");
	}
	let Some(hello) = observed.hello else {
		bail!("no DTLS ClientHello arrived from {label}");
	};
	Ok(Captured {
		summary: sdp::summarize(&label, &offer),
		offer,
		dtls: dtls_capture(&label, &hello),
		hello_bytes: observed.reader.bytes,
		hello_datagrams: observed.reader.datagrams,
		connected: observed.connected,
	})
}

#[derive(Default)]
struct Page {
	offer: Option<String>,
	answerer: Option<Answerer>,
	states: Vec<String>,
}

fn accept(server: &TcpListener, shared: &Mutex<Page>, backend: Backend, dtls: Dtls) {
	while let Ok((stream, _)) = server.accept() {
		if let Err(error) = respond(stream, shared, backend, dtls) {
			println!("webrtc-capture: request failed: {error:#}");
		}
	}
}

fn respond(stream: TcpStream, shared: &Mutex<Page>, backend: Backend, dtls: Dtls) -> Result<()> {
	stream.set_nonblocking(false)?;
	stream.set_read_timeout(Some(Duration::from_secs(5)))?;
	let mut reader = BufReader::new(stream.try_clone()?);
	let mut request_line = String::new();
	if reader.read_line(&mut request_line)? == 0 {
		return Ok(());
	}
	let mut length = 0;
	loop {
		let mut line = String::new();
		reader.read_line(&mut line)?;
		let line = line.trim_end();
		if line.is_empty() {
			break;
		}
		if let Some((name, value)) = line.split_once(':')
			&& name.eq_ignore_ascii_case("content-length")
		{
			length = value.trim().parse::<usize>()?.min(MAX_BODY);
		}
	}
	let mut body = vec![0u8; length];
	reader.read_exact(&mut body)?;
	let body = String::from_utf8_lossy(&body).into_owned();
	let mut parts = request_line.split(' ');
	let (method, path) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
	let (status, kind, reply) = match (method, path) {
		("GET", "/") => ("200 OK", "text/html; charset=utf-8", PAGE.to_owned()),
		("POST", "/offer") => {
			let answerer = Answerer::start(backend, dtls, &body)?;
			let answer = answerer.answer.clone();
			let mut page = shared.lock();
			page.offer = Some(body);
			page.answerer = Some(answerer);
			("200 OK", "application/sdp", answer)
		}
		("POST", "/state") => {
			shared.lock().states.push(body);
			("200 OK", "text/plain", String::new())
		}
		_ => ("404 Not Found", "text/plain", String::new()),
	};
	let mut stream = stream;
	write!(
		stream,
		"HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{reply}",
		reply.len()
	)?;
	stream.flush()?;
	Ok(())
}

fn dtls_capture(label: &str, hello: &ClientHello) -> Capture {
	Capture {
		client: label.to_owned(),
		kind: "dtls".to_owned(),
		negotiated_alpn: None,
		tls: TlsSummary::of(hello),
		http2: None,
		http1: None,
	}
}

fn print(captured: &Captured) -> Result<()> {
	println!();
	println!("== offer ({})", captured.summary.client);
	println!("{}", captured.offer.trim_end());
	println!();
	println!("{}", serde_json::to_string_pretty(&captured.summary)?);
	println!();
	println!(
		"== dtls ({}), {} bytes in {} datagrams, handshake completed: {}",
		captured.dtls.client, captured.hello_bytes, captured.hello_datagrams, captured.connected
	);
	println!("JA4    {}", captured.dtls.tls.ja4);
	println!("{}", serde_json::to_string_pretty(&captured.dtls)?);
	Ok(())
}

fn record(root: &Path, name: &str, captured: &Captured) -> Result<()> {
	let folder = root.join(FIXTURES);
	fs::create_dir_all(&folder).with_context(|| format!("create {}", folder.display()))?;
	let files = [
		(format!("{name}-offer.sdp"), captured.offer.replace("\r\n", "\n")),
		(
			format!("{name}-offer.json"),
			serde_json::to_string_pretty(&captured.summary)? + "\n",
		),
		(
			format!("{name}-dtls.json"),
			serde_json::to_string_pretty(&captured.dtls)? + "\n",
		),
	];
	for (file, text) in files {
		let path = folder.join(file);
		fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
		println!("webrtc-capture: wrote {}", path.display());
	}
	Ok(())
}

fn compare(root: &Path, name: &str, captured: &Captured) -> Result<()> {
	let folder = root.join(FIXTURES);
	let read = |file: String| {
		let path = folder.join(file);
		fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))
	};
	let offer: OfferSummary = serde_json::from_str(&read(format!("{name}-offer.json"))?)?;
	let found = sdp::differences(&offer, &captured.summary);
	println!();
	println!("== offer against {name}-offer.json ({} differences)", found.len());
	for difference in found {
		println!("{difference}");
	}
	let dtls: Capture = serde_json::from_str(&read(format!("{name}-dtls.json"))?)?;
	let found = differences(&dtls, &captured.dtls);
	println!();
	println!("== dtls against {name}-dtls.json ({} differences)", found.len());
	for difference in found {
		println!("{}", difference.field);
		println!("  expected {}", difference.expected);
		println!("  actual   {}", difference.actual);
	}
	Ok(())
}

struct Browser {
	child: Child,
	profile: PathBuf,
}

impl Browser {
	fn start(exe: &Path, port: u16) -> Result<Browser> {
		let profile = std::env::temp_dir().join(format!("kyoko-webrtc-capture-{}", std::process::id()));
		let _ = fs::remove_dir_all(&profile);
		fs::create_dir_all(&profile).with_context(|| format!("create {}", profile.display()))?;
		let child = Command::new(exe)
			.args([
				&format!("--user-data-dir={}", profile.display()),
				"--no-first-run",
				"--no-default-browser-check",
				"--disable-extensions",
				"--disable-sync",
				"--disable-background-networking",
				"--disable-component-update",
				"--allow-loopback-in-peer-connection",
				"--disable-features=WebRtcHideLocalIpsWithMdns",
				"--new-window",
				&format!("http://127.0.0.1:{port}/"),
			])
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.with_context(|| format!("start {}", exe.display()))?;
		Ok(Browser { child, profile })
	}

	fn running(&mut self) -> bool {
		matches!(self.child.try_wait(), Ok(None))
	}

	fn stop(mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
		for _ in 0..PROFILE_CLEANUP_TRIES {
			if fs::remove_dir_all(&self.profile).is_ok() || !self.profile.exists() {
				return;
			}
			std::thread::sleep(POLL);
		}
	}
}

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.map(Path::to_path_buf)
		.unwrap_or_else(|| PathBuf::from("."))
}
