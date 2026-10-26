use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, io, thread};

use protocol::Token;
use url::Url;

use crate::sign_in::{Failure, Flow, Screen, Status};

const PROFILE_PREFIX: &str = "kyoko-web-login-";
const REMOVE_ATTEMPTS: u32 = 150;
const REMOVE_DELAY: Duration = Duration::from_millis(100);

static PROFILES_MADE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct WebLoginConfig {
	pub login_url: Url,
	pub profiles: PathBuf,
}

impl WebLoginConfig {
	pub fn discord() -> Self {
		Self::at(Url::parse(login::DISCORD_LOGIN_URL).expect("discord's login page is a url"))
	}

	pub fn at(login_url: Url) -> Self {
		Self {
			login_url,
			profiles: std::env::temp_dir(),
		}
	}

	pub fn origin(&self) -> String {
		self.login_url.origin().ascii_serialization()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebEnd {
	Unavailable,
	Rejected,
	Network,
	Refused,
}

impl WebEnd {
	fn after(failure: Failure) -> WebEnd {
		match failure {
			Failure::Empty | Failure::Rejected => WebEnd::Rejected,
			Failure::Network => WebEnd::Network,
			Failure::Refused => WebEnd::Refused,
		}
	}

	pub fn message(self) -> &'static str {
		match self {
			WebEnd::Unavailable => {
				"Signing in on discord.com needs the Microsoft Edge WebView2 Runtime, which isn't available. Use a token or the QR code instead."
			}
			WebEnd::Rejected => "Discord rejected this sign-in. Try again.",
			WebEnd::Network => Failure::Network.message(),
			WebEnd::Refused => Failure::Refused.message(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebStage {
	Closed,
	Open,
	SigningIn,
	Ended(WebEnd),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPage {
	pub url: String,
	pub origin: String,
	pub profile: PathBuf,
	pub user_agent: String,
	pub script: String,
}

impl WebPage {
	pub fn allows(&self, url: &str) -> bool {
		same_origin(url, &self.origin)
	}
}

fn same_origin(url: &str, origin: &str) -> bool {
	Url::parse(url).is_ok_and(|url| url.origin().ascii_serialization() == origin)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCookie {
	pub name: String,
	pub value: String,
	pub max_age: Option<u64>,
	pub secure: bool,
	pub http_only: bool,
}

impl PageCookie {
	fn header(&self) -> String {
		let mut header = format!("{}={}; Path=/", self.name, self.value);
		if let Some(seconds) = self.max_age {
			header.push_str(&format!("; Max-Age={seconds}"));
		}
		if self.secure {
			header.push_str("; Secure");
		}
		if self.http_only {
			header.push_str("; HttpOnly");
		}
		header
	}
}

#[derive(Debug)]
pub struct Profile {
	path: PathBuf,
}

impl Profile {
	fn create(parent: &Path) -> io::Result<Profile> {
		let stamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		let count = PROFILES_MADE.fetch_add(1, Ordering::Relaxed);
		let path = parent.join(format!(
			"{PROFILE_PREFIX}{}-{stamp:x}-{count}",
			std::process::id()
		));
		fs::create_dir_all(&path)?;
		Ok(Profile { path })
	}

	pub fn path(&self) -> &Path {
		&self.path
	}

	pub fn remove(self) {
		if !remove_with_retries(&self.path) {
			tracing::warn!("the web login profile could not be removed yet; the next launch removes it");
		}
	}
}

fn remove_with_retries(path: &Path) -> bool {
	for _ in 0..REMOVE_ATTEMPTS {
		match fs::remove_dir_all(path) {
			Ok(()) => return true,
			Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
			Err(_) => thread::sleep(REMOVE_DELAY),
		}
	}
	false
}

pub fn sweep_profiles(parent: &Path) {
	let Ok(entries) = fs::read_dir(parent) else {
		return;
	};
	for entry in entries.flatten() {
		let leftover = entry.file_name().to_string_lossy().starts_with(PROFILE_PREFIX)
			&& entry.file_type().is_ok_and(|kind| kind.is_dir());
		if leftover && fs::remove_dir_all(entry.path()).is_err() {
			tracing::debug!("a web login profile is still in use");
		}
	}
}

pub struct WebSignIn {
	config: WebLoginConfig,
	stage: WebStage,
	profile: Option<Profile>,
	address: Option<Url>,
	loaded: bool,
}

fn idle(flow: &Flow) -> bool {
	matches!(flow.screen(), Screen::Login(status) if *status != Status::Checking)
		&& flow.gate().acknowledged()
}

impl WebSignIn {
	pub fn new(config: WebLoginConfig) -> Self {
		Self {
			config,
			stage: WebStage::Closed,
			profile: None,
			address: None,
			loaded: false,
		}
	}

	pub fn config(&self) -> &WebLoginConfig {
		&self.config
	}

	pub fn stage(&self) -> WebStage {
		self.stage
	}

	pub fn address(&self) -> Option<&Url> {
		self.address.as_ref()
	}

	pub fn loaded(&self) -> bool {
		self.loaded
	}

	pub fn wants_page(&self) -> bool {
		self.stage == WebStage::Open
	}

	pub fn open(&mut self, flow: &Flow) -> Option<WebPage> {
		if !idle(flow) || matches!(self.stage, WebStage::Open | WebStage::SigningIn) {
			return None;
		}
		let profile = match Profile::create(&self.config.profiles) {
			Ok(profile) => profile,
			Err(error) => {
				tracing::warn!(%error, "the web login profile could not be made");
				self.stage = WebStage::Ended(WebEnd::Unavailable);
				return None;
			}
		};
		let origin = self.config.origin();
		let page = WebPage {
			url: self.config.login_url.to_string(),
			script: login::token_script(&origin),
			origin,
			profile: profile.path().to_path_buf(),
			user_agent: flow.gate().identity().fingerprint().user_agent().to_owned(),
		};
		self.profile = Some(profile);
		self.address = Some(self.config.login_url.clone());
		self.loaded = false;
		self.stage = WebStage::Open;
		Some(page)
	}

	pub fn unavailable(&mut self) {
		if self.stage == WebStage::Open {
			self.stage = WebStage::Ended(WebEnd::Unavailable);
		}
	}

	pub fn allows(&self, url: &str) -> bool {
		same_origin(url, &self.config.origin())
	}

	pub fn navigated(&mut self, url: &str) {
		if self.stage != WebStage::Open {
			return;
		}
		if let Ok(parsed) = Url::parse(url)
			&& same_origin(url, &self.config.origin())
		{
			self.address = Some(parsed);
		}
	}

	pub fn page_loaded(&mut self) {
		if self.stage == WebStage::Open {
			self.loaded = true;
		}
	}

	pub fn message(&mut self, source: &str, body: &str) -> Option<Token> {
		if self.stage != WebStage::Open || !self.allows(source) {
			return None;
		}
		let token = login::token_from_page(body)?;
		self.stage = WebStage::SigningIn;
		Some(token)
	}

	pub fn carry_cookies(&self, flow: &Flow, cookies: &[PageCookie]) {
		let Ok(origin) = Url::parse(&self.config.origin()) else {
			return;
		};
		let headers: Vec<String> = cookies
			.iter()
			.map(PageCookie::header)
			.filter(|header| login::carried_cookie(header))
			.collect();
		if headers.is_empty() {
			return;
		}
		flow.gate()
			.identity()
			.cookies()
			.adopt(headers.iter().map(String::as_str), &origin);
	}

	pub fn signed_in(&mut self, failure: Option<Failure>) {
		if self.stage != WebStage::SigningIn {
			return;
		}
		self.stage = match failure {
			Some(failure) => WebStage::Ended(WebEnd::after(failure)),
			None => WebStage::Closed,
		};
	}

	pub fn cancel(&mut self) {
		if matches!(self.stage, WebStage::Open | WebStage::Ended(_)) {
			self.stage = WebStage::Closed;
		}
	}

	pub fn follow(&mut self, flow: &Flow) {
		let on_login = matches!(flow.screen(), Screen::Login(_));
		if !on_login && matches!(self.stage, WebStage::Open | WebStage::Ended(_)) {
			self.stage = WebStage::Closed;
		}
	}

	pub fn release(&mut self) -> Option<Profile> {
		if self.wants_page() {
			return None;
		}
		self.loaded = false;
		self.profile.take()
	}
}
