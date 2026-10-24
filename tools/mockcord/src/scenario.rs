use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Json;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use parking_lot::Mutex;
use serde_json::{Value, json};

const CAPTCHA_SITEKEY: &str = "mock-captcha-sitekey";
const CAPTCHA_RQDATA: &str = "mock-captcha-rqdata";
const CAPTCHA_RQTOKEN: &str = "mock-captcha-rqtoken";
const CAPTCHA_SESSION: &str = "mock-captcha-session";

#[derive(Debug, Clone)]
pub struct Reply {
	pub status: u16,
	pub headers: Vec<(String, String)>,
	pub body: Value,
}

impl Reply {
	pub fn json(status: u16, body: Value) -> Self {
		Self {
			status,
			headers: Vec::new(),
			body,
		}
	}

	pub fn error(status: u16, code: i64, message: &str) -> Self {
		Self::json(status, json!({ "message": message, "code": code }))
	}

	pub fn with_header(mut self, name: &str, value: &str) -> Self {
		self.headers.push((name.to_owned(), value.to_owned()));
		self
	}

	pub fn unauthorized() -> Self {
		Self::error(401, 0, "401: Unauthorized")
	}

	pub fn forbidden() -> Self {
		Self::error(403, 50001, "Missing Access")
	}

	pub fn missing_permissions() -> Self {
		Self::error(403, 50013, "Missing Permissions")
	}

	pub fn rate_limited(retry_after: f64, global: bool) -> Self {
		let scope = if global { "global" } else { "user" };
		let reply = Self::json(
			429,
			json!({
				"message": "You are being rate limited.",
				"retry_after": retry_after,
				"global": global,
				"code": 0,
			}),
		)
		.with_header("retry-after", &retry_after.ceil().max(0.0).to_string())
		.with_header("x-ratelimit-scope", scope);
		if global {
			reply.with_header("x-ratelimit-global", "true")
		} else {
			reply
		}
	}

	pub fn captcha_required() -> Self {
		Self::captcha(&["captcha-required"])
	}

	pub fn captcha(keys: &[&str]) -> Self {
		Self::json(
			400,
			json!({
				"captcha_key": keys,
				"captcha_sitekey": CAPTCHA_SITEKEY,
				"captcha_service": "hcaptcha",
				"captcha_rqdata": CAPTCHA_RQDATA,
				"captcha_rqtoken": CAPTCHA_RQTOKEN,
				"captcha_session_id": CAPTCHA_SESSION,
			}),
		)
	}
}

impl IntoResponse for Reply {
	fn into_response(self) -> Response {
		let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
		let mut response = (status, Json(self.body)).into_response();
		for (name, value) in &self.headers {
			if let (Ok(name), Ok(value)) = (
				HeaderName::from_bytes(name.as_bytes()),
				HeaderValue::from_str(value),
			) {
				response.headers_mut().append(name, value);
			}
		}
		response
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartbeatAcks {
	#[default]
	Prompt,
	Delayed(Duration),
	Withheld,
}

pub type ReadyTransform = Arc<dyn Fn(&Value, &mut Value) + Send + Sync>;

struct Rule {
	method: String,
	pattern: String,
	reply: Reply,
	remaining: Option<usize>,
}

#[derive(Default)]
pub struct Scenario {
	rules: Mutex<Vec<Rule>>,
	failing_resumes: AtomicUsize,
	closing_identifies: Mutex<Vec<u16>>,
	closing_connections: Mutex<Vec<u16>>,
	acks: Mutex<HeartbeatAcks>,
	ready: Mutex<Option<ReadyTransform>>,
}

impl Scenario {
	pub fn respond(&self, method: &str, pattern: &str, reply: Reply, times: Option<usize>) {
		if times == Some(0) {
			return;
		}
		self.rules.lock().push(Rule {
			method: method.to_owned(),
			pattern: pattern.to_owned(),
			reply,
			remaining: times,
		});
	}

	pub fn clear_replies(&self) {
		self.rules.lock().clear();
	}

	pub fn take_reply(&self, method: &str, path: &str) -> Option<Reply> {
		let mut rules = self.rules.lock();
		let index = rules.iter().position(|rule| {
			(rule.method == "*" || rule.method.eq_ignore_ascii_case(method))
				&& path_matches(&rule.pattern, path)
		})?;
		let rule = &mut rules[index];
		let reply = rule.reply.clone();
		if let Some(remaining) = &mut rule.remaining {
			*remaining -= 1;
			if *remaining == 0 {
				rules.remove(index);
			}
		}
		Some(reply)
	}

	pub fn fail_resumes(&self, count: usize) {
		self.failing_resumes.fetch_add(count, Ordering::Relaxed);
	}

	pub fn take_resume_failure(&self) -> bool {
		self.failing_resumes
			.try_update(Ordering::Relaxed, Ordering::Relaxed, |left| left.checked_sub(1))
			.is_ok()
	}

	pub fn close_next_identify(&self, code: u16) {
		self.closing_identifies.lock().push(code);
	}

	pub fn take_identify_close(&self) -> Option<u16> {
		let mut codes = self.closing_identifies.lock();
		(!codes.is_empty()).then(|| codes.remove(0))
	}

	pub fn close_next_connections(&self, code: u16, count: usize) {
		self.closing_connections
			.lock()
			.extend(std::iter::repeat_n(code, count));
	}

	pub fn take_connection_close(&self) -> Option<u16> {
		let mut codes = self.closing_connections.lock();
		(!codes.is_empty()).then(|| codes.remove(0))
	}

	pub fn set_acks(&self, acks: HeartbeatAcks) {
		*self.acks.lock() = acks;
	}

	pub fn acks(&self) -> HeartbeatAcks {
		*self.acks.lock()
	}

	pub fn set_ready(&self, transform: Option<ReadyTransform>) {
		*self.ready.lock() = transform;
	}

	pub fn ready(&self) -> Option<ReadyTransform> {
		self.ready.lock().clone()
	}
}

pub fn path_matches(pattern: &str, path: &str) -> bool {
	let pattern: Vec<&str> = pattern.trim_matches('/').split('/').collect();
	let path: Vec<&str> = path.trim_matches('/').split('/').collect();
	segments_match(&pattern, &path)
}

fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
	match (pattern.split_first(), path.split_first()) {
		(None, None) => true,
		(Some((&"**", rest)), _) => {
			rest.is_empty() || (0..=path.len()).any(|skip| segments_match(rest, &path[skip..]))
		}
		(Some((expected, rest)), Some((actual, remaining))) => {
			segment_matches(expected, actual) && segments_match(rest, remaining)
		}
		_ => false,
	}
}

fn segment_matches(expected: &str, actual: &str) -> bool {
	expected == "*" || (expected.starts_with('{') && expected.ends_with('}')) || expected == actual
}
