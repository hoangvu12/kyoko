use protocol::Token;
use zeroize::Zeroizing;

use crate::token_login::{clean_token, plausible};

pub const DISCORD_LOGIN_URL: &str = "https://discord.com/login";
pub const CARRIED_COOKIES: [&str; 3] = ["__dcfduid", "__sdcfduid", "_cfuvid"];

const POLL_MS: u32 = 500;

pub fn token_script(origin: &str) -> String {
	let origin = serde_json::Value::from(origin).to_string();
	format!(
		r#"(() => {{
	if (window.top !== window || window.location.origin !== {origin}) {{
		return;
	}}
	const view = window.chrome && window.chrome.webview;
	const storage = window.localStorage;
	if (!view || !storage) {{
		return;
	}}
	const post = view.postMessage.bind(view);
	const timer = setInterval(() => {{
		let stored = null;
		try {{
			stored = storage.getItem('token');
		}} catch (error) {{
			return;
		}}
		if (stored) {{
			clearInterval(timer);
			post(stored);
		}}
	}}, {POLL_MS});
}})();"#
	)
}

pub fn token_from_page(stored: &str) -> Option<Token> {
	let stored = Zeroizing::new(stored.to_owned());
	let cleaned = clean_token(&stored);
	plausible(cleaned).then(|| Token::new(cleaned))
}

pub fn carried_cookie(set_cookie: &str) -> bool {
	let name = set_cookie.split_once('=').map_or("", |(name, _)| name.trim());
	CARRIED_COOKIES.contains(&name)
}
