use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SIGNATURE_PARAMS: [&str; 3] = ["ex", "is", "hm"];

fn param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
	let query = url.split_once('?')?.1.split('#').next()?;
	query.split('&').find_map(|pair| {
		let (key, value) = pair.split_once('=')?;
		(key == name).then_some(value)
	})
}

pub fn is_signed(url: &str) -> bool {
	param(url, "ex").is_some() && param(url, "hm").is_some()
}

pub fn expires_at(url: &str) -> Option<SystemTime> {
	let seconds = u64::from_str_radix(param(url, "ex")?, 16).ok()?;
	UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}

pub fn is_expired(url: &str, now: SystemTime) -> bool {
	is_signed(url) && expires_at(url).is_none_or(|expiry| expiry <= now)
}

pub fn unsigned(url: &str) -> Cow<'_, str> {
	let Some((base, rest)) = url.split_once('?') else {
		return Cow::Borrowed(url);
	};
	let query = rest.split('#').next().unwrap_or_default();
	let kept: Vec<&str> = query
		.split('&')
		.filter(|pair| {
			let name = pair.split('=').next().unwrap_or_default();
			!name.is_empty() && !SIGNATURE_PARAMS.contains(&name)
		})
		.collect();
	if kept.is_empty() {
		Cow::Borrowed(base)
	} else {
		Cow::Owned(format!("{base}?{}", kept.join("&")))
	}
}
