use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bridge::WebPage;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
	CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
	WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use windows::core::w;

use super::{Bounds, PageEvent, PageHost};

const WAIT: Duration = Duration::from_secs(60);
const PUMP_PAUSE: Duration = Duration::from_millis(5);
const PAGE: &str = "<!doctype html><title>agent</title><p>agent</p>";
const REPORT: &str = "window.chrome.webview.postMessage(JSON.stringify({ agent: navigator.userAgent, brands: navigator.userAgentData ? navigator.userAgentData.brands : [] }));";

type Headers = HashMap<String, String>;

struct LoginServer {
	origin: String,
	seen: Arc<Mutex<Option<Headers>>>,
}

impl LoginServer {
	fn start() -> LoginServer {
		let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
		let origin = format!("http://{}", listener.local_addr().expect("an address"));
		let seen = Arc::new(Mutex::new(None));
		let recorded = Arc::clone(&seen);
		std::thread::spawn(move || {
			for stream in listener.incoming().flatten() {
				answer(stream, &recorded);
			}
		});
		LoginServer { origin, seen }
	}

	fn login_headers(&self) -> Headers {
		self.seen
			.lock()
			.expect("the recording")
			.clone()
			.expect("the page was requested")
	}
}

fn answer(mut stream: TcpStream, seen: &Mutex<Option<Headers>>) {
	let Ok(reader) = stream.try_clone() else {
		return;
	};
	let mut lines = BufReader::new(reader).lines().map_while(Result::ok);
	let Some(request) = lines.next() else {
		return;
	};
	let headers: Headers = lines
		.take_while(|line| !line.is_empty())
		.filter_map(|line| {
			let (name, value) = line.split_once(':')?;
			Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
		})
		.collect();
	let reply = if request.starts_with("GET /login ") {
		seen.lock().expect("the recording").get_or_insert(headers);
		format!(
			"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
			PAGE.len()
		)
	} else {
		"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
	};
	let _ = stream.write_all(reply.as_bytes());
}

struct Parent(HWND);

impl Parent {
	fn hidden() -> Parent {
		let window = unsafe {
			CreateWindowExW(
				WINDOW_EX_STYLE::default(),
				w!("STATIC"),
				w!("kyoko web login agent"),
				WS_OVERLAPPEDWINDOW,
				0,
				0,
				640,
				480,
				None,
				None,
				None,
				None,
			)
		}
		.expect("a hidden parent window");
		Parent(window)
	}

	fn handle(&self) -> usize {
		self.0.0 as usize
	}
}

impl Drop for Parent {
	fn drop(&mut self) {
		let _ = unsafe { DestroyWindow(self.0) };
	}
}

fn pump_until(what: &str, mut done: impl FnMut() -> bool) {
	let started = Instant::now();
	let mut message = MSG::default();
	while !done() {
		assert!(started.elapsed() < WAIT, "timed out waiting for {what}");
		while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
			unsafe {
				let _ = TranslateMessage(&message);
				DispatchMessageW(&message);
			}
		}
		std::thread::sleep(PUMP_PAUSE);
	}
}

struct ProfileFolder(PathBuf);

impl ProfileFolder {
	fn fresh() -> ProfileFolder {
		let path = std::env::temp_dir().join(format!("kyoko-web-login-agent-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&path);
		std::fs::create_dir_all(&path).expect("a profile folder");
		ProfileFolder(path)
	}

	fn path(&self) -> &Path {
		&self.0
	}
}

impl Drop for ProfileFolder {
	fn drop(&mut self) {
		let started = Instant::now();
		while std::fs::remove_dir_all(&self.0).is_err() && self.0.exists() && started.elapsed() < WAIT {
			std::thread::sleep(PUMP_PAUSE);
		}
	}
}

fn brand_list(brands: &serde_json::Value) -> Vec<(String, String)> {
	brands
		.as_array()
		.expect("the page reports its brands")
		.iter()
		.map(|brand| {
			(
				brand["brand"].as_str().expect("a brand name").to_owned(),
				brand["version"].as_str().expect("a brand version").to_owned(),
			)
		})
		.collect()
}

fn chrome_major(agent: &str) -> &str {
	agent
		.split_once("Chrome/")
		.and_then(|(_, rest)| rest.split('.').next())
		.expect("the agent names Chrome's version")
}

#[test]
fn the_login_page_sees_webview2s_own_agent_and_matching_client_hints() {
	let server = LoginServer::start();
	let profile = ProfileFolder::fresh();
	let page = WebPage {
		url: format!("{}/login", server.origin),
		origin: server.origin.clone(),
		profile: profile.path().to_path_buf(),
		script: REPORT.to_owned(),
	};
	let parent = Parent::hidden();
	let reports = Rc::new(RefCell::new(Vec::new()));
	let sink = Rc::clone(&reports);
	let bounds = Bounds {
		x: 0,
		y: 0,
		width: 640,
		height: 480,
	};
	let host = PageHost::open(parent.handle(), &page, bounds, move |event| {
		if let PageEvent::Message { source, body } = event {
			sink.borrow_mut().push((source, body));
		}
	})
	.expect("WebView2 opens the page");
	pump_until("the page's report", || !reports.borrow().is_empty());
	host.close();
	drop(parent);

	let (source, body) = reports.borrow()[0].clone();
	assert!(page.allows(&source), "the report comes from the page");
	let report: serde_json::Value = serde_json::from_str(&body).expect("the report is json");
	let agent = report["agent"].as_str().expect("the page reports its agent");
	let brands = brand_list(&report["brands"]);
	let sent = server.login_headers();

	assert_eq!(sent.get("user-agent").map(String::as_str), Some(agent));
	assert_ne!(agent, protocol::fingerprint::user_agent());
	assert!(agent.contains(" Edg/"), "{agent} is WebView2's Edge agent");
	assert!(
		brands.iter().any(|(brand, _)| brand == "Microsoft Edge WebView2"),
		"{brands:?} are WebView2's brands"
	);
	let chromium = brands
		.iter()
		.find(|(brand, _)| brand == "Chromium")
		.map(|(_, version)| version.as_str());
	assert_eq!(chromium, Some(chrome_major(agent)));
	let hints = brands
		.iter()
		.map(|(brand, version)| format!("\"{brand}\";v=\"{version}\""))
		.collect::<Vec<_>>()
		.join(", ");
	assert_eq!(sent.get("sec-ch-ua"), Some(&hints));
}
