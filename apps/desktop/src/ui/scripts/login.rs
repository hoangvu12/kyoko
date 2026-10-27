use serde_json::json;

use super::{Calls, WIDE, last_call, recording_app, shows};

const WEB_NOTE: &str = "You'll sign in on Discord's own page, in a Kyoko window.";
const EMAIL_LABEL: &str = "EMAIL OR PHONE NUMBER";

fn called(calls: &Calls, name: &str) -> usize {
	calls.borrow().iter().filter(|(called, _)| called == name).count()
}

#[test]
fn the_login_card_leads_with_discord_com_and_keeps_email_one_click_away() {
	let calls = Calls::default();
	let mut app = recording_app(&calls, "mentions");
	app.ui.scene.borrow_mut().resize(WIDE.0, WIDE.1);
	shows(&mut app, "the locked discord.com column", WEB_NOTE, &[]);
	app.click("login_email_link");
	app.click("login_discord");
	app.frames(2);
	assert!(
		last_call(&calls, "login.mode").is_none() && called(&calls, "login.web_open") == 0,
		"the column ignores the pointer until the notice is accepted"
	);

	shows(
		&mut app,
		"the discord.com column",
		"Sign in on discord.com",
		&[("login.acknowledged", json!(true))],
	);
	assert!(
		!app.texts().iter().any(|text| text == EMAIL_LABEL),
		"the email form is not the default"
	);
	app.click("login_discord");
	app.frames(2);
	assert_eq!(
		called(&calls, "login.web_open"),
		1,
		"the main button opens web login"
	);

	app.click("login_email_link");
	app.frames(2);
	assert_eq!(last_call(&calls, "login.mode"), Some(json!("email")));
	shows(
		&mut app,
		"the email form",
		EMAIL_LABEL,
		&[("login.mode", json!("email"))],
	);
	for marker in ["Forgot your password?", "Log In", "Sign in with a token"] {
		assert!(
			app.texts().iter().any(|text| text == marker),
			"the email form keeps \"{marker}\""
		);
	}
	app.click("login_email_back");
	app.frames(2);
	assert_eq!(last_call(&calls, "login.mode"), Some(json!("discord")));
	shows(
		&mut app,
		"the way back",
		WEB_NOTE,
		&[("login.mode", json!("discord"))],
	);

	app.click("login_token_link");
	app.frames(2);
	assert_eq!(last_call(&calls, "login.mode"), Some(json!("token")));
	shows(
		&mut app,
		"the token form",
		"Sign in with your account token.",
		&[("login.mode", json!("token"))],
	);
	app.click("login_token_back");
	app.frames(2);
	assert_eq!(last_call(&calls, "login.mode"), Some(json!("discord")));

	shows(
		&mut app,
		"a session that expired",
		"Your session expired. Sign in again to continue.",
		&[
			("login.mode", json!("discord")),
			(
				"login.message",
				json!("Your session expired. Sign in again to continue."),
			),
			("login.error", json!(true)),
		],
	);
	shows(
		&mut app,
		"adding an account",
		"Back to your account",
		&[("login.adding", json!(true))],
	);
}
