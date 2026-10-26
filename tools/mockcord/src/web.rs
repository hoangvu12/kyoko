use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::state::Shared;

pub const DEVICE_COOKIE: &str = "__dcfduid";
pub const SIGNED_DEVICE_COOKIE: &str = "__sdcfduid";
pub const VISITOR_COOKIE: &str = "_cfuvid";
pub const LOGIN_PAGE: &str = "/login";
const APP_SCRIPT: &str = "/assets/web.mock.js";
const SENTRY_SCRIPT: &str = "/assets/sentry.mock.js";
const COOKIE_MAX_AGE_SECS: u64 = 31_536_000;

pub fn installation_id(seed: u64) -> String {
	format!("mock-installation.{seed:016x}")
}

pub fn device_cookie_value(seed: u64) -> String {
	format!("{:016x}{:016x}", seed, seed.rotate_left(29) ^ 0x5bd1_e995)
}

pub async fn app() -> impl IntoResponse {
	let html = format!(
		"<!DOCTYPE html><html><head><script src=\"{APP_SCRIPT}\"></script><script src=\"{SENTRY_SCRIPT}\"></script></head><body></body></html>"
	);
	([(CONTENT_TYPE, "text/html")], html)
}

pub async fn asset(State(shared): State<Arc<Shared>>, Path(name): Path<String>) -> Response {
	let body = match format!("/assets/{name}").as_str() {
		APP_SCRIPT => "window.mock=true;".to_owned(),
		SENTRY_SCRIPT => format!(
			"Sentry.init({{release:\"discord_web-mock\",buildNumber:\"{}\"}});",
			shared.config.build_number
		),
		_ => return StatusCode::NOT_FOUND.into_response(),
	};
	([(CONTENT_TYPE, "text/javascript")], body).into_response()
}

pub async fn device_cookie(State(shared): State<Arc<Shared>>, request: Request, next: Next) -> Response {
	let has_cookie = request
		.headers()
		.get_all(COOKIE)
		.iter()
		.filter_map(|value| value.to_str().ok())
		.flat_map(|value| value.split(';'))
		.any(|pair| pair.trim().starts_with(&format!("{DEVICE_COOKIE}=")));
	let mut response = next.run(request).await;
	if !has_cookie {
		let cookie = format!(
			"{DEVICE_COOKIE}={}; Max-Age={COOKIE_MAX_AGE_SECS}; Path=/; HttpOnly; SameSite=Lax",
			device_cookie_value(shared.config.seed)
		);
		if let Ok(value) = HeaderValue::from_str(&cookie) {
			response.headers_mut().append(SET_COOKIE, value);
		}
	}
	response
}

pub fn login_cookies(seed: u64) -> Vec<String> {
	let device = device_cookie_value(seed);
	let signed = format!("{device}{:016x}", seed.rotate_right(17) ^ 0x2545_f491);
	let visitor = format!("{:016x}.{seed}", seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
	vec![
		format!("{DEVICE_COOKIE}={device}; Max-Age={COOKIE_MAX_AGE_SECS}; Path=/; HttpOnly; SameSite=Lax"),
		format!(
			"{SIGNED_DEVICE_COOKIE}={signed}; Max-Age={COOKIE_MAX_AGE_SECS}; Path=/; HttpOnly; SameSite=Lax"
		),
		format!("{VISITOR_COOKIE}={visitor}; Path=/; HttpOnly; SameSite=None"),
	]
}

#[derive(Deserialize)]
pub struct LoginQuery {
	auto: Option<String>,
}

const LOGIN_HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Discord</title>
<style>
body{margin:0;height:100vh;display:flex;align-items:center;justify-content:center;background:#313338;color:#dbdee1;font:14px sans-serif}
form{width:320px;display:flex;flex-direction:column;gap:12px}
h1{margin:0;font-size:22px;text-align:center;color:#f2f3f5}
label{font-size:12px;font-weight:700;text-transform:uppercase;color:#b5bac1}
input{height:40px;border:0;border-radius:4px;padding:0 10px;background:#1e1f22;color:#dbdee1}
button{height:44px;border:0;border-radius:4px;background:#5865f2;color:#fff;font-size:16px}
p{margin:0;min-height:18px;color:#fa777c}
</style></head>
<body><form id="login"><h1>Welcome back!</h1>
<label for="email">Email or phone number</label><input id="email" autocomplete="username">
<label for="password">Password</label><input id="password" type="password" autocomplete="current-password">
<button type="submit">Log In</button><p id="error"></p></form>
<script>
const form = document.getElementById('login');
const signIn = async (login, password) => {
	const response = await fetch('/api/v9/auth/login', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ login, password }),
	});
	const body = await response.json();
	if (!response.ok || !body.token) {
		document.getElementById('error').textContent = body.message || 'Login failed';
		return;
	}
	window.localStorage.setItem('token', JSON.stringify(body.token));
	window.location.replace('/app');
};
form.addEventListener('submit', (event) => {
	event.preventDefault();
	signIn(document.getElementById('email').value, document.getElementById('password').value);
});
if (AUTO) {
	signIn('mock@kyoko.invalid', 'mock-password');
}
</script></body></html>"#;

pub async fn login_page(State(shared): State<Arc<Shared>>, Query(query): Query<LoginQuery>) -> Response {
	let auto = query.auto.is_some_and(|auto| auto == "1");
	let html = LOGIN_HTML.replace("AUTO", if auto { "true" } else { "false" });
	let mut response = ([(CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response();
	for cookie in login_cookies(shared.config.seed) {
		if let Ok(value) = HeaderValue::from_str(&cookie) {
			response.headers_mut().append(SET_COOKIE, value);
		}
	}
	response
}
