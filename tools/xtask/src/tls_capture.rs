use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use mockcord::hello::{
	API_CALLS, Capture, Connection, FETCH_PATH, GATEWAY_HOST, GATEWAY_PATH, HelloListener, Http2Request,
	IMAGE_HOST, IMAGE_PATH, Mode, TlsSummary, WEBSOCKET_PATH, differences,
};

use crate::parity::browser;

const POLL: Duration = Duration::from_millis(200);
const PROFILE_CLEANUP_TRIES: usize = 20;
const FIXTURES: &str = "tools/mockcord/fixtures/tls";
const HTTPS: &str = "https";
const WSS: &str = "wss";
const IMAGE: &str = "image";
const CROSS_SITE_WSS: &str = "wss-cross-site";
const KINDS: [&str; 4] = [HTTPS, WSS, IMAGE, CROSS_SITE_WSS];
const PAGE_HOST: &str = "localhost";

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Client {
	Chrome,
	Wreq,
}

#[derive(clap::Args)]
pub struct Args {
	#[arg(long, value_enum, default_value = "chrome")]
	client: Client,
	#[arg(long)]
	h2: bool,
	#[arg(long)]
	browser: Option<PathBuf>,
	#[arg(long)]
	trust_root: bool,
	#[arg(long)]
	record: Option<String>,
	#[arg(long)]
	compare: Option<String>,
	#[arg(long, default_value_t = 40)]
	wait: u64,
}

pub fn run(args: Args) -> Result<()> {
	let root = workspace_root();
	if args.client == Client::Wreq {
		build_probe(&root)?;
	}
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.context("could not start the async runtime")?;
	let mode = if args.h2 { Mode::Complete } else { Mode::HelloOnly };
	let listener = runtime
		.block_on(HelloListener::start(mode))
		.context("could not listen on localhost")?;
	let port = listener.port();
	println!("tls-capture: listening on https://localhost:{port}/ only");
	let (label, mut client) = match args.client {
		Client::Chrome => {
			let exe = browser::find(args.browser.as_deref())?;
			(chrome_label(&exe), Running::chrome(&exe, port)?)
		}
		Client::Wreq => {
			let root_pem = args.trust_root.then(|| listener.root_pem().to_owned());
			let label = if args.trust_root {
				"wreq Chrome154 via kyoko-net, test root trusted"
			} else {
				"wreq Chrome154 via kyoko-net, certificate checks off"
			};
			(
				label.to_owned(),
				Running::probe(&root, port, root_pem.as_deref())?,
			)
		}
	};
	let deadline = Instant::now() + Duration::from_secs(args.wait);
	while Instant::now() < deadline
		&& !complete(&listener.connections(), mode, args.client)
		&& client.running()
	{
		std::thread::sleep(POLL);
	}
	if args.client == Client::Wreq {
		client.wait_until(deadline);
	}
	client.stop();
	let connections = listener.connections();
	drop(listener);
	runtime.shutdown_timeout(Duration::from_secs(1));
	print_connections(&connections);
	let captures = KINDS
		.into_iter()
		.filter_map(|kind| capture(&connections, mode, kind, &label))
		.collect::<Vec<_>>();
	if captures.is_empty() {
		bail!("no ClientHello arrived from {label}");
	}
	for captured in &captures {
		print_capture(captured)?;
	}
	if let Some(name) = &args.record {
		record(&root, name, &captures)?;
	}
	if let Some(name) = &args.compare {
		compare(&root, name, &captures)?;
	}
	Ok(())
}

fn complete(connections: &[Connection], mode: Mode, client: Client) -> bool {
	match mode {
		Mode::HelloOnly => [HTTPS, WSS].into_iter().all(|kind| {
			connections
				.iter()
				.any(|connection| offered_kind(connection) == Some(kind))
		}),
		Mode::Complete => {
			let fetched = |path| connections.iter().any(|connection| requested(connection, path));
			let upgraded = |path| {
				connections
					.iter()
					.any(|connection| upgrade_path(connection) == Some(path))
			};
			let probed = fetched(FETCH_PATH) && upgraded(WEBSOCKET_PATH);
			match client {
				Client::Wreq => probed,
				Client::Chrome => {
					probed
						&& API_CALLS.iter().all(|call| fetched(call.path))
						&& fetched(IMAGE_PATH)
						&& upgraded(GATEWAY_PATH)
				}
			}
		}
	}
}

fn offered_kind(connection: &Connection) -> Option<&'static str> {
	let summary = TlsSummary::of(connection.hello.as_ref()?);
	Some(if summary.alpn.iter().any(|protocol| protocol == "h2") {
		HTTPS
	} else {
		WSS
	})
}

fn requested(connection: &Connection, path: &str) -> bool {
	connection.http2.as_ref().is_some_and(|summary| {
		summary
			.requests
			.iter()
			.any(|request| request.path() == Some(path))
	})
}

fn on_page_host(request: &Http2Request) -> bool {
	request
		.headers
		.iter()
		.find(|(name, _)| name == ":authority")
		.is_some_and(|(_, authority)| authority.split(':').next() == Some(PAGE_HOST))
}

fn upgrade_path(connection: &Connection) -> Option<&str> {
	let request = connection.http1.as_ref()?;
	let upgrade = request
		.headers
		.iter()
		.any(|(name, value)| name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket"));
	upgrade.then(|| request.request_line.split(' ').nth(1))?
}

fn capture(connections: &[Connection], mode: Mode, kind: &str, label: &str) -> Option<Capture> {
	let chosen = connections.iter().find(|connection| match (mode, kind) {
		(Mode::HelloOnly, HTTPS | WSS) => offered_kind(connection) == Some(kind),
		(Mode::HelloOnly, _) => false,
		(Mode::Complete, HTTPS) => connection
			.http2
			.as_ref()
			.is_some_and(|summary| summary.requests.iter().any(on_page_host)),
		(Mode::Complete, IMAGE) => requested(connection, IMAGE_PATH),
		(Mode::Complete, WSS) => upgrade_path(connection) == Some(WEBSOCKET_PATH),
		(Mode::Complete, _) => upgrade_path(connection) == Some(GATEWAY_PATH),
	})?;
	let http2 = chosen.http2.clone().map(|mut summary| {
		if kind == HTTPS {
			summary.requests = connections
				.iter()
				.filter_map(|connection| connection.http2.as_ref())
				.flat_map(|summary| {
					summary
						.requests
						.iter()
						.filter(|request| on_page_host(request))
						.cloned()
				})
				.collect();
		}
		summary
	});
	Some(Capture {
		client: label.to_owned(),
		kind: kind.to_owned(),
		negotiated_alpn: chosen.alpn.clone(),
		tls: TlsSummary::of(chosen.hello.as_ref()?),
		http2,
		http1: chosen.http1.clone(),
	})
}

fn print_connections(connections: &[Connection]) {
	for (index, connection) in connections.iter().enumerate() {
		let offered = connection
			.hello
			.as_ref()
			.map(|hello| TlsSummary::of(hello).alpn.join(","))
			.unwrap_or_else(|| "no hello".to_owned());
		let mut paths = connection
			.http2
			.iter()
			.flat_map(|summary| summary.requests.iter())
			.filter_map(|request| request.path().map(str::to_owned))
			.collect::<Vec<_>>();
		paths.extend(
			connection
				.http1
				.iter()
				.map(|request| request.request_line.clone()),
		);
		println!(
			"tls-capture: connection {index}: offered [{offered}], negotiated {}, requests [{}]",
			connection.alpn.as_deref().unwrap_or("none"),
			paths.join(", ")
		);
	}
}

fn print_capture(captured: &Capture) -> Result<()> {
	println!();
	println!("== {} ({})", captured.kind, captured.client);
	println!("JA4    {}", captured.tls.ja4);
	if let Some(http2) = &captured.http2 {
		println!("Akamai {}", http2.akamai);
	}
	println!("{}", serde_json::to_string_pretty(captured)?);
	Ok(())
}

fn record(root: &Path, name: &str, captures: &[Capture]) -> Result<()> {
	let folder = root.join(FIXTURES);
	fs::create_dir_all(&folder).with_context(|| format!("create {}", folder.display()))?;
	for captured in captures {
		let path = folder.join(format!("{name}-{}.json", captured.kind));
		let mut text = serde_json::to_string_pretty(captured)?;
		text.push('\n');
		fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
		println!("tls-capture: wrote {}", path.display());
	}
	Ok(())
}

fn compare(root: &Path, name: &str, captures: &[Capture]) -> Result<()> {
	for captured in captures {
		let path = root.join(FIXTURES).join(format!("{name}-{}.json", captured.kind));
		if !path.exists() {
			println!();
			println!("== {} has no {} to compare with", captured.kind, path.display());
			continue;
		}
		let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
		let expected: Capture =
			serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
		let found = differences(&expected, captured);
		println!();
		println!(
			"== {} against {} ({} differences)",
			captured.kind,
			path.display(),
			found.len()
		);
		for difference in found {
			println!("{}", difference.field);
			println!("  expected {}", difference.expected);
			println!("  actual   {}", difference.actual);
		}
	}
	Ok(())
}

fn chrome_label(exe: &Path) -> String {
	let version = exe.parent().and_then(|folder| {
		fs::read_dir(folder)
			.ok()?
			.filter_map(|entry| entry.ok())
			.filter_map(|entry| entry.file_name().into_string().ok())
			.filter(|name| {
				name.split('.').count() == 4 && name.split('.').all(|part| part.parse::<u32>().is_ok())
			})
			.max()
	});
	format!("chrome {}", version.unwrap_or_else(|| "unknown".to_owned()))
}

struct Running {
	child: Child,
	profile: Option<PathBuf>,
	root_file: Option<PathBuf>,
}

impl Running {
	fn chrome(exe: &Path, port: u16) -> Result<Running> {
		let profile = std::env::temp_dir().join(format!("kyoko-tls-capture-{}", std::process::id()));
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
				"--ignore-certificate-errors",
				&format!("--host-resolver-rules=MAP {IMAGE_HOST} 127.0.0.1, MAP {GATEWAY_HOST} 127.0.0.1"),
				"--new-window",
				&format!("https://localhost:{port}/"),
			])
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.with_context(|| format!("start {}", exe.display()))?;
		Ok(Running {
			child,
			profile: Some(profile),
			root_file: None,
		})
	}

	fn probe(root: &Path, port: u16, root_pem: Option<&str>) -> Result<Running> {
		let mut command = cargo(root);
		command.args([
			"run",
			"--quiet",
			"-p",
			"kyoko-net",
			"--example",
			"tls_probe",
			"--",
		]);
		command.arg(port.to_string());
		let mut root_file = None;
		if let Some(pem) = root_pem {
			let path =
				std::env::temp_dir().join(format!("kyoko-tls-capture-root-{}.pem", std::process::id()));
			fs::write(&path, pem).with_context(|| format!("write {}", path.display()))?;
			command.arg(&path);
			root_file = Some(path);
		}
		let child = command
			.stdin(Stdio::null())
			.spawn()
			.context("start the wreq probe")?;
		Ok(Running {
			child,
			profile: None,
			root_file,
		})
	}

	fn running(&mut self) -> bool {
		matches!(self.child.try_wait(), Ok(None))
	}

	fn wait_until(&mut self, deadline: Instant) {
		while Instant::now() < deadline && self.running() {
			std::thread::sleep(POLL);
		}
	}

	fn stop(mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
		if let Some(path) = self.root_file.take() {
			let _ = fs::remove_file(path);
		}
		if let Some(profile) = self.profile.take() {
			for _ in 0..PROFILE_CLEANUP_TRIES {
				if fs::remove_dir_all(&profile).is_ok() || !profile.exists() {
					return;
				}
				std::thread::sleep(POLL);
			}
		}
	}
}

fn cargo(root: &Path) -> Command {
	let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
	command.current_dir(root);
	command
}

fn build_probe(root: &Path) -> Result<()> {
	let status = cargo(root)
		.args(["build", "--quiet", "-p", "kyoko-net", "--example", "tls_probe"])
		.status()
		.context("build the wreq probe")?;
	if !status.success() {
		bail!("building the wreq probe failed");
	}
	Ok(())
}

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.map(Path::to_path_buf)
		.unwrap_or_else(|| PathBuf::from("."))
}
