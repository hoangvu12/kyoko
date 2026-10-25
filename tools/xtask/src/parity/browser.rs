use std::collections::VecDeque;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use image::RgbaImage;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{self, Message, WebSocket};
use windows::Win32::System::Registry::{
	HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::core::{HSTRING, PCWSTR};

use super::compose::{HEIGHT, WIDTH};

pub const BROWSER_VARIABLE: &str = "KYOKO_PARITY_BROWSER";
const START_LIMIT: Duration = Duration::from_secs(30);
const PAGE_LIMIT: Duration = Duration::from_secs(60);
const CLOSE_GRACE: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(100);
const EXECUTABLES: [&str; 2] = ["chrome.exe", "msedge.exe"];
const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\";

const SETTLED: &str = r#"(async () => {
	await document.fonts.ready;
	await Promise.all([...document.images].map((image) => image.decode().catch(() => null)));
	await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
	const wanted = [...document.querySelectorAll('link[rel="stylesheet"]')].map((link) => link.href);
	const loaded = [...document.styleSheets].map((sheet) => sheet.href).filter(Boolean);
	return {
		stylesheets: wanted.filter((href) => !loaded.includes(href)),
		images: [...new Set([...document.images].filter((image) => !image.naturalWidth).map((image) => image.src))],
		fonts: wanted.length > 0 && ![...document.fonts].some((face) => face.status === "loaded"),
	};
})()"#;

pub fn find(chosen: Option<&Path>) -> Result<PathBuf> {
	if let Some(path) = chosen {
		return existing(path, "--browser");
	}
	if let Some(path) = std::env::var_os(BROWSER_VARIABLE) {
		return existing(Path::new(&path), BROWSER_VARIABLE);
	}
	installed().with_context(|| {
		format!(
			"no Chrome or Edge found; install one, or pass --browser <path to chrome.exe or msedge.exe>, or set {BROWSER_VARIABLE}"
		)
	})
}

fn existing(path: &Path, source: &str) -> Result<PathBuf> {
	if path.is_file() {
		Ok(path.to_path_buf())
	} else {
		bail!("the browser {} from {source} does not exist", path.display())
	}
}

fn installed() -> Option<PathBuf> {
	let program_dirs = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
		.into_iter()
		.filter_map(std::env::var_os)
		.map(PathBuf::from)
		.collect::<Vec<_>>();
	let folders = [
		Path::new("Google").join("Chrome").join("Application"),
		Path::new("Microsoft").join("Edge").join("Application"),
	];
	let in_program_dirs = folders
		.iter()
		.zip(EXECUTABLES)
		.flat_map(|(folder, exe)| program_dirs.iter().map(move |dir| dir.join(folder).join(exe)));
	let registered = EXECUTABLES.into_iter().flat_map(|exe| {
		[HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
			.into_iter()
			.filter_map(move |root| app_path(root, exe))
	});
	in_program_dirs.chain(registered).find(|path| path.is_file())
}

fn app_path(root: HKEY, exe: &str) -> Option<PathBuf> {
	let key = HSTRING::from(format!("{APP_PATHS}{exe}"));
	let mut size = 0u32;
	unsafe {
		RegGetValueW(
			root,
			&key,
			PCWSTR::null(),
			RRF_RT_REG_SZ,
			None,
			None,
			Some(&mut size),
		)
	}
	.ok()
	.ok()?;
	let mut buffer = vec![0u16; size as usize / 2];
	unsafe {
		RegGetValueW(
			root,
			&key,
			PCWSTR::null(),
			RRF_RT_REG_SZ,
			None,
			Some(buffer.as_mut_ptr().cast()),
			Some(&mut size),
		)
	}
	.ok()
	.ok()?;
	let text = String::from_utf16_lossy(&buffer);
	let path = text.trim_end_matches('\0').trim_matches('"');
	(!path.is_empty()).then(|| PathBuf::from(path))
}

pub struct Browser {
	child: Child,
	profile: PathBuf,
	socket: WebSocket<TcpStream>,
	next_id: u64,
	events: VecDeque<Value>,
}

impl Browser {
	pub fn start(exe: &Path, log: &Path) -> Result<Browser> {
		let profile = std::env::temp_dir().join(format!("kyoko-parity-browser-{}", std::process::id()));
		let _ = fs::remove_dir_all(&profile);
		fs::create_dir_all(&profile).with_context(|| format!("create {}", profile.display()))?;
		let log = fs::File::create(log).context("create the browser log")?;
		let child = Command::new(exe)
			.args([
				"--headless=new",
				"--remote-debugging-port=0",
				&format!("--user-data-dir={}", profile.display()),
				"--no-first-run",
				"--no-default-browser-check",
				"--disable-extensions",
				"--disable-sync",
				"--hide-scrollbars",
				"--mute-audio",
				"--force-color-profile=srgb",
				"--font-render-hinting=none",
				"about:blank",
			])
			.stdin(Stdio::null())
			.stdout(log.try_clone().context("share the browser log")?)
			.stderr(log)
			.spawn()
			.with_context(|| format!("start {}", exe.display()))?;
		let mut starting = Starting {
			child: Some(child),
			profile: profile.clone(),
		};
		let (port, path) = starting.endpoint()?;
		let stream = TcpStream::connect(("127.0.0.1", port)).context("connect to the browser")?;
		stream
			.set_read_timeout(Some(PAGE_LIMIT))
			.context("set the browser read timeout")?;
		let (socket, _) = tungstenite::client(format!("ws://127.0.0.1:{port}{path}"), stream)
			.map_err(|error| anyhow::anyhow!("open the DevTools connection: {error}"))?;
		let child = starting
			.child
			.take()
			.context("the browser was lost while starting")?;
		Ok(Browser {
			child,
			profile,
			socket,
			next_id: 0,
			events: VecDeque::new(),
		})
	}

	pub fn render(&mut self, page: &Path, scale: f64) -> Result<RgbaImage> {
		let url = file_url(page)?;
		let target = self.call(None, "Target.createTarget", json!({ "url": "about:blank" }))?;
		let target = target["targetId"]
			.as_str()
			.context("the browser opened no tab")?
			.to_owned();
		let rendered = self.render_in(&target, &url, scale);
		let _ = self.call(None, "Target.closeTarget", json!({ "targetId": target }));
		rendered
	}

	fn render_in(&mut self, target: &str, url: &str, scale: f64) -> Result<RgbaImage> {
		let attached = self.call(
			None,
			"Target.attachToTarget",
			json!({ "targetId": target, "flatten": true }),
		)?;
		let session = attached["sessionId"]
			.as_str()
			.context("the browser did not attach to the tab")?
			.to_owned();
		let session = Some(session.as_str());
		self.call(
			session,
			"Emulation.setDeviceMetricsOverride",
			json!({ "width": WIDTH, "height": HEIGHT, "deviceScaleFactor": scale, "mobile": false }),
		)?;
		self.call(session, "Page.enable", json!({}))?;
		self.events.clear();
		let navigated = self.call(session, "Page.navigate", json!({ "url": url }))?;
		if let Some(error) = navigated["errorText"].as_str() {
			bail!("the browser could not open {url}: {error}");
		}
		self.wait_event(session, "Page.loadEventFired")?;
		let settled = self.call(
			session,
			"Runtime.evaluate",
			json!({ "expression": SETTLED, "awaitPromise": true, "returnByValue": true }),
		)?;
		if let Some(error) = settled["exceptionDetails"].as_object() {
			bail!("the mockup did not settle: {}", Value::Object(error.clone()));
		}
		missing_resources(&settled["result"]["value"])?;
		let shot = self.call(
			session,
			"Page.captureScreenshot",
			json!({
				"format": "png",
				"clip": { "x": 0, "y": 0, "width": WIDTH, "height": HEIGHT, "scale": 1 },
			}),
		)?;
		let data = shot["data"]
			.as_str()
			.context("the browser returned no screenshot")?;
		let bytes = base64::engine::general_purpose::STANDARD
			.decode(data)
			.context("decode the screenshot")?;
		Ok(image::load_from_memory(&bytes)
			.context("read the screenshot")?
			.to_rgba8())
	}

	fn call(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<Value> {
		self.next_id += 1;
		let id = self.next_id;
		let mut request = json!({ "id": id, "method": method, "params": params });
		if let Some(session) = session {
			request["sessionId"] = json!(session);
		}
		self.socket
			.send(Message::text(request.to_string()))
			.with_context(|| format!("send {method} to the browser"))?;
		loop {
			let reply = self.receive(method)?;
			if reply["id"].as_u64() != Some(id) {
				if reply.get("method").is_some() {
					self.events.push_back(reply);
				}
				continue;
			}
			if let Some(error) = reply.get("error") {
				bail!("the browser refused {method}: {error}");
			}
			return Ok(reply["result"].clone());
		}
	}

	fn wait_event(&mut self, session: Option<&str>, event: &str) -> Result<()> {
		let deadline = Instant::now() + PAGE_LIMIT;
		while Instant::now() < deadline {
			let message = match self.events.pop_front() {
				Some(message) => message,
				None => self.receive(event)?,
			};
			if message["method"].as_str() == Some(event) && message["sessionId"].as_str() == session {
				return Ok(());
			}
		}
		bail!("the mockup did not finish loading in {}s", PAGE_LIMIT.as_secs())
	}

	fn receive(&mut self, waiting_for: &str) -> Result<Value> {
		loop {
			let message = self
				.socket
				.read()
				.with_context(|| format!("wait for {waiting_for} from the browser"))?;
			if let Message::Text(text) = message {
				return serde_json::from_str(text.as_str()).context("read a DevTools message");
			}
		}
	}

	pub fn stop(mut self) {
		let _ = self.call(None, "Browser.close", json!({}));
		let _ = self.socket.close(None);
		stop_child(&mut self.child, &self.profile);
	}
}

struct Starting {
	child: Option<Child>,
	profile: PathBuf,
}

impl Starting {
	fn endpoint(&mut self) -> Result<(u16, String)> {
		let file = self.profile.join("DevToolsActivePort");
		let deadline = Instant::now() + START_LIMIT;
		loop {
			if let Some(endpoint) = fs::read_to_string(&file).ok().as_deref().and_then(parse_endpoint) {
				return Ok(endpoint);
			}
			if let Some(child) = self.child.as_mut()
				&& let Ok(Some(status)) = child.try_wait()
			{
				bail!("the browser exited with {status} before it opened DevTools; see browser.log");
			}
			if Instant::now() >= deadline {
				bail!(
					"the browser did not open DevTools in {}s; see browser.log",
					START_LIMIT.as_secs()
				);
			}
			std::thread::sleep(POLL);
		}
	}
}

impl Drop for Starting {
	fn drop(&mut self) {
		if let Some(mut child) = self.child.take() {
			let _ = child.kill();
			let _ = child.wait();
			let _ = fs::remove_dir_all(&self.profile);
		}
	}
}

fn parse_endpoint(text: &str) -> Option<(u16, String)> {
	let mut lines = text.lines();
	let port = lines.next()?.trim().parse().ok()?;
	let path = lines.next()?.trim();
	path.starts_with('/').then(|| (port, path.to_owned()))
}

fn stop_child(child: &mut Child, profile: &Path) {
	let deadline = Instant::now() + CLOSE_GRACE;
	while Instant::now() < deadline && !matches!(child.try_wait(), Ok(Some(_))) {
		std::thread::sleep(POLL);
	}
	let _ = child.kill();
	let _ = child.wait();
	for _ in 0..20 {
		if fs::remove_dir_all(profile).is_ok() || !profile.exists() {
			return;
		}
		std::thread::sleep(POLL);
	}
}

fn missing_resources(report: &Value) -> Result<()> {
	let listed = |key: &str| -> Vec<String> {
		report[key]
			.as_array()
			.map(|items| {
				items
					.iter()
					.filter_map(Value::as_str)
					.map(str::to_owned)
					.collect()
			})
			.unwrap_or_default()
	};
	let mut missing = listed("stylesheets");
	missing.extend(listed("images"));
	if report["fonts"].as_bool() == Some(true) {
		missing.push("the Inter font".to_owned());
	}
	if missing.is_empty() {
		return Ok(());
	}
	bail!(
		"the mockup could not load {}; rendering a mockup needs the network on the first run",
		missing.join(", ")
	)
}

fn file_url(page: &Path) -> Result<String> {
	let page = page
		.canonicalize()
		.with_context(|| format!("find {}", page.display()))?;
	let text = page.to_string_lossy();
	let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
	let url = url::Url::from_file_path(text).map_err(|()| anyhow::anyhow!("{text} is not a file path"))?;
	Ok(url.to_string())
}
