use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::state::Shared;

pub const DEVICE_COOKIE: &str = "__dcfduid";
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
