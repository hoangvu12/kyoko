use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue};
use reqwest::redirect::Policy;
use url::{Host, Url};

use crate::error::{RestError, Result, network_error};
use crate::identity::ClientIdentity;

const MAX_SCRIPTS: usize = 6;
const MAX_TOTAL_BYTES: usize = 30 * 1024 * 1024;
const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
const KEYS: [&[u8]; 3] = [b"build_number:\"", b"buildNumber:\"", b"buildNumber\",\""];
const SENTRY_MARKER: &str = "sentry";
const PLAUSIBLE_BUILDS: std::ops::Range<u64> = 1..100_000_000;

pub fn discovery_client(identity: &ClientIdentity) -> Result<Client> {
	let fingerprint = identity.fingerprint();
	let mut headers = HeaderMap::new();
	if let Ok(language) = HeaderValue::from_str(&fingerprint.accept_language()) {
		headers.insert(ACCEPT_LANGUAGE, language);
	}
	Client::builder()
		.redirect(Policy::none())
		.user_agent(fingerprint.user_agent())
		.default_headers(headers)
		.cookie_provider(identity.cookies().clone())
		.build()
		.map_err(network_error)
}

pub fn allowed_source(url: &Url) -> bool {
	match (url.scheme(), url.host()) {
		("https", Some(Host::Domain(domain))) => domain == "discord.com",
		("http", Some(Host::Ipv4(address))) => address.is_loopback(),
		("http", Some(Host::Ipv6(address))) => address.is_loopback(),
		("http", Some(Host::Domain(domain))) => domain.eq_ignore_ascii_case("localhost"),
		_ => false,
	}
}

pub async fn discover_build_number(client: &Client, app_url: &Url) -> Result<u64> {
	if !allowed_source(app_url) {
		return Err(RestError::InvalidRequest);
	}
	let (page, _) = fetch_capped(client, app_url, MAX_PAGE_BYTES).await?;
	let mut scripts = script_urls(app_url, &String::from_utf8_lossy(&page));
	scripts.reverse();
	scripts.sort_by_key(|url| !url.path().contains(SENTRY_MARKER));
	let mut remaining = MAX_TOTAL_BYTES;
	for url in scripts.iter().take(MAX_SCRIPTS) {
		let (body, complete) = fetch_capped(client, url, remaining).await?;
		remaining -= body.len();
		if let Some(number) = find_build_number(&body).filter(|number| PLAUSIBLE_BUILDS.contains(number)) {
			return Ok(number);
		}
		if !complete || remaining == 0 {
			break;
		}
	}
	Err(RestError::NotFound)
}

async fn fetch_capped(client: &Client, url: &Url, cap: usize) -> Result<(Vec<u8>, bool)> {
	let response = client
		.get(url.clone())
		.send()
		.await
		.map_err(network_error)?
		.error_for_status()
		.map_err(network_error)?;
	let mut body = Vec::new();
	let mut stream = response.bytes_stream();
	while let Some(chunk) = stream.next().await {
		let chunk = chunk.map_err(network_error)?;
		let room = cap - body.len();
		if chunk.len() > room {
			body.extend_from_slice(&chunk[..room]);
			return Ok((body, false));
		}
		body.extend_from_slice(&chunk);
	}
	Ok((body, true))
}

fn script_urls(base: &Url, html: &str) -> Vec<Url> {
	html.split("src=\"")
		.skip(1)
		.filter_map(|rest| rest.split('"').next())
		.filter(|source| source.starts_with("/assets/") && source.ends_with(".js"))
		.filter_map(|source| base.join(source).ok())
		.collect()
}

fn find_build_number(body: &[u8]) -> Option<u64> {
	KEYS.iter().find_map(|key| {
		body.windows(key.len())
			.enumerate()
			.filter(|(_, window)| window == key)
			.find_map(|(index, _)| {
				let digits = &body[index + key.len()..];
				let length = digits.iter().take_while(|byte| byte.is_ascii_digit()).count();
				let closed = digits.get(length) == Some(&b'"');
				let text = std::str::from_utf8(&digits[..length]).ok()?;
				closed.then(|| text.parse().ok()).flatten()
			})
	})
}
