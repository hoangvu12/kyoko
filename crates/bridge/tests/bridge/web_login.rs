use std::path::{Path, PathBuf};

use bridge::{Failure, PageCookie, Screen, Status, WebEnd, WebLoginConfig, WebPage, WebSignIn, WebStage};
use mockcord::{DEVICE_COOKIE, SIGNED_DEVICE_COOKIE, VISITOR_COOKIE};
use protocol::Snowflake;
use protocol::fingerprint::CHROME_MAJOR;
use url::Url;

use super::sign_in::{Desk, eventually};

const STRANGERS: [&str; 3] = [
	"https://evil.example/login",
	"https://discord.com.evil.example/login",
	"http://localhost/login",
];

struct Browser {
	client: reqwest::Client,
	origin: String,
}

struct Visit {
	cookies: Vec<PageCookie>,
	stored: String,
}

impl Browser {
	fn new(desk: &Desk) -> Browser {
		Browser {
			client: reqwest::Client::new(),
			origin: format!("http://{}", desk.server.address()),
		}
	}

	async fn sign_in(&self, page: &WebPage) -> Visit {
		let shown = self
			.client
			.get(&page.url)
			.send()
			.await
			.expect("the login page loads");
		assert!(shown.status().is_success());
		let cookies = shown
			.headers()
			.get_all(reqwest::header::SET_COOKIE)
			.iter()
			.filter_map(|value| value.to_str().ok())
			.map(page_cookie)
			.collect();
		let answer: serde_json::Value = self
			.client
			.post(format!("{}/api/v9/auth/login", self.origin))
			.json(&serde_json::json!({ "login": "mock@kyoko.invalid", "password": "mock-password" }))
			.send()
			.await
			.expect("the login form posts")
			.json()
			.await
			.expect("the login answer is json");
		let token = answer["token"].as_str().expect("the login answer has a token");
		Visit {
			cookies,
			stored: serde_json::Value::from(token).to_string(),
		}
	}
}

fn page_cookie(header: &str) -> PageCookie {
	let mut parts = header.split(';').map(str::trim);
	let (name, value) = parts
		.next()
		.and_then(|pair| pair.split_once('='))
		.expect("a cookie pair");
	let attributes: Vec<&str> = parts.collect();
	PageCookie {
		name: name.to_owned(),
		value: value.to_owned(),
		max_age: attributes
			.iter()
			.find_map(|attribute| attribute.strip_prefix("Max-Age="))
			.and_then(|seconds| seconds.parse().ok()),
		secure: attributes.contains(&"Secure"),
		http_only: attributes.contains(&"HttpOnly"),
	}
}

fn profiles(name: &str) -> PathBuf {
	let path = std::env::temp_dir().join(format!("kyoko-web-profiles-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&path);
	std::fs::create_dir_all(&path).expect("a profiles folder");
	path
}

fn web_for(desk: &Desk, profiles: &Path) -> WebSignIn {
	WebSignIn::new(WebLoginConfig {
		login_url: Url::parse(&desk.server.login_page_url()).expect("login page url"),
		profiles: profiles.to_path_buf(),
	})
}

fn released(web: &mut WebSignIn, page: &WebPage) {
	assert!(!web.wants_page());
	let profile = web.release().expect("the page's profile is handed back once");
	assert_eq!(profile.path(), page.profile.as_path());
	profile.remove();
	assert!(!page.profile.exists(), "the profile folder is removed");
	assert!(web.release().is_none());
}

fn cookie_header(desk: &Desk, path: &str) -> Option<String> {
	desk.server
		.control()
		.recording()
		.requests
		.into_iter()
		.filter(|request| request.is("GET", path))
		.find_map(|request| request.header("cookie").map(str::to_owned))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_token_from_the_login_page_signs_in_and_carries_its_cookies() {
	let mut desk = Desk::open("web-full").await;
	desk.flow.launch(None);
	let folder = profiles("full");
	let mut web = web_for(&desk, &folder);
	assert!(
		web.open(&desk.flow).is_none(),
		"no page before the notice is accepted"
	);
	desk.acknowledge();
	let page = web.open(&desk.flow).expect("the login screen opens the page");
	assert_eq!(web.stage(), WebStage::Open);
	assert!(web.open(&desk.flow).is_none(), "one page at a time");
	assert_eq!(page.url, desk.server.login_page_url());
	assert_eq!(page.origin, format!("http://{}", desk.server.address()));
	assert!(
		page.script
			.contains(&serde_json::Value::from(page.origin.as_str()).to_string())
	);
	assert!(page.profile.is_dir());
	assert!(page.profile.starts_with(&folder));
	let identity = desk.flow.gate().identity().fingerprint();
	assert_eq!(page.user_agent, identity.user_agent());
	assert!(page.user_agent.contains(&format!("Chrome/{CHROME_MAJOR}.0.0.0")));

	let visit = Browser::new(&desk).sign_in(&page).await;
	let names: Vec<&str> = visit.cookies.iter().map(|cookie| cookie.name.as_str()).collect();
	assert_eq!(names, [DEVICE_COOKIE, SIGNED_DEVICE_COOKIE, VISITOR_COOKIE]);
	assert!(visit.cookies.iter().all(|cookie| cookie.http_only));
	web.navigated(&format!("{}/app", page.origin));
	assert_eq!(web.address().map(Url::path), Some("/app"));
	let token = web
		.message(&format!("{}/app", page.origin), &visit.stored)
		.expect("the page hands over its token");
	assert_eq!(token.as_str(), desk.server.token());
	assert_eq!(web.stage(), WebStage::SigningIn);
	assert!(
		web.message(&page.origin, &visit.stored).is_none(),
		"the token is taken once"
	);
	web.carry_cookies(&desk.flow, &visit.cookies);
	released(&mut web, &page);

	let check = desk.flow.redeem(token).expect("the login screen takes the token");
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Checking));
	let failure = desk.flow.redeemed(check.await);
	assert_eq!(failure, None);
	web.signed_in(failure);
	assert_eq!(web.stage(), WebStage::Closed);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	assert_eq!(desk.user_id(), Snowflake(100));
	assert_eq!(desk.store.accounts.current(), Some(Snowflake(100)));

	eventually("a request from the session", || {
		cookie_header(&desk, "/users/@me").is_some()
	})
	.await;
	let sent = cookie_header(&desk, "/users/@me").expect("the validation carries cookies");
	for cookie in &visit.cookies {
		assert!(
			sent.contains(&format!("{}={}", cookie.name, cookie.value)),
			"{} reaches REST",
			cookie.name
		);
	}
	desk.stop().await;
	let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_token_from_another_origin_is_ignored_and_cancel_removes_the_profile() {
	let mut desk = Desk::open("web-strangers").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let folder = profiles("strangers");
	let mut web = web_for(&desk, &folder);
	let page = web.open(&desk.flow).expect("the page opens");
	let stored = serde_json::Value::from(desk.server.token()).to_string();
	for stranger in STRANGERS {
		assert!(web.message(stranger, &stored).is_none(), "{stranger} is ignored");
		assert!(!web.allows(stranger));
		web.navigated(stranger);
		assert_eq!(web.address().map(Url::as_str), Some(page.url.as_str()));
	}
	assert!(web.message(&page.origin, "").is_none());
	assert!(web.message(&page.origin, "\"\"").is_none());
	assert!(web.message(&page.origin, "not a token").is_none());
	assert_eq!(web.stage(), WebStage::Open);
	assert!(web.release().is_none(), "an open page keeps its profile");
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(desk.server.control().recording().requests.is_empty());

	web.cancel();
	assert_eq!(web.stage(), WebStage::Closed);
	assert!(
		web.message(&page.origin, &stored).is_none(),
		"a closed page signs nothing in"
	);
	released(&mut web, &page);

	let page = web.open(&desk.flow).expect("the page opens again");
	web.unavailable();
	assert_eq!(web.stage(), WebStage::Ended(WebEnd::Unavailable));
	assert!(WebEnd::Unavailable.message().contains("WebView2"));
	released(&mut web, &page);
	assert!(desk.store.accounts.list().is_empty());
	desk.stop().await;
	let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_hand_off_ends_with_a_message_and_removes_the_profile() {
	let mut desk = Desk::open("web-rejected").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let folder = profiles("rejected");
	let mut web = web_for(&desk, &folder);
	let page = web.open(&desk.flow).expect("the page opens");
	let token = web
		.message(&page.origin, "\"mock-but-wrong-token\"")
		.expect("a plausible token is handed over");
	released(&mut web, &page);
	let check = desk.flow.redeem(token).expect("the token is checked");
	let failure = desk.flow.redeemed(check.await);
	assert_eq!(failure, Some(Failure::Rejected));
	web.signed_in(failure);
	assert_eq!(web.stage(), WebStage::Ended(WebEnd::Rejected));
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(desk.store.accounts.list().is_empty());

	let page = web.open(&desk.flow).expect("a failed sign-in can start over");
	let pasted = desk.server.token().to_owned();
	let check = desk.flow.submit(pasted).expect("a pasted token is still taken");
	desk.flow.checked(check.await);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	web.follow(&desk.flow);
	assert_eq!(
		web.stage(),
		WebStage::Closed,
		"leaving the login screen closes the page"
	);
	released(&mut web, &page);
	desk.stop().await;
	let _ = std::fs::remove_dir_all(&folder);
}
