use bridge::{
	MfaMethod, MfaView, PasswordHandover, PasswordProblem, PasswordSignIn, PasswordStage, PasswordWork,
	Screen, Status, WebLoginConfig, WebSignIn,
};
use mockcord::{
	BACKUP_CODE, CAPTCHA_LOGIN, MFA_LOGIN, MOCK_PASSWORD, PASSWORD_LOGIN, Reply, RestRequest,
	SECURITY_KEY_LOGIN, SMS_CODE, SMS_PHONE, TOTP_CODE,
};
use protocol::{Snowflake, Token};
use tokio::time::timeout;
use url::Url;

use super::sign_in::{Desk, STEP_TIMEOUT, files_containing};

const LOGIN: &str = "/auth/login";
const TEXT: &str = "/auth/mfa/sms/send";

async fn answer(password: &mut PasswordSignIn, work: PasswordWork) -> Option<Token> {
	let reply = timeout(STEP_TIMEOUT, work)
		.await
		.expect("discord answers in time");
	password.apply(reply)
}

async fn log_in(desk: &mut Desk, password: &mut PasswordSignIn, login: &str, secret: &str) -> Option<Token> {
	let work = password
		.submit(&desk.flow, login.to_owned(), secret.to_owned())
		.expect("the form sends the sign-in");
	assert_eq!(password.stage(), &PasswordStage::LoggingIn);
	answer(password, work).await
}

async fn finish(desk: &mut Desk, password: &mut PasswordSignIn, token: Token) {
	assert_eq!(password.stage(), &PasswordStage::SigningIn);
	assert_eq!(token.as_str(), desk.server.token());
	let check = desk.flow.redeem(token).expect("the login screen takes the token");
	let failure = desk.flow.redeemed(check.await);
	assert_eq!(failure, None);
	password.signed_in(failure);
	password.follow(&desk.flow);
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	desk.connected().await;
	assert_eq!(desk.user_id(), Snowflake(100));
	assert_eq!(desk.store.accounts.current(), Some(Snowflake(100)));
}

fn requests(desk: &Desk, path: &str) -> Vec<RestRequest> {
	desk.server
		.control()
		.recording()
		.requests
		.into_iter()
		.filter(|request| request.is("POST", path))
		.collect()
}

fn from_the_login_page(request: &RestRequest) {
	assert_eq!(request.header("authorization"), None);
	assert_eq!(request.header("referer"), Some("https://discord.com/login"));
	assert_eq!(request.header("origin"), Some("https://discord.com"));
	assert!(request.header("x-super-properties").is_some());
	assert!(
		request
			.header("user-agent")
			.is_some_and(|agent| agent.contains("Chrome/"))
	);
}

fn mfa_view(password: &PasswordSignIn) -> MfaView {
	match password.stage() {
		PasswordStage::Mfa(view) => view.clone(),
		other => panic!("expected the two-factor step, got {other:?}"),
	}
}

fn signed_out(desk: &Desk) {
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::Ready));
	assert!(desk.flow.session().is_none());
	assert!(desk.store.accounts.list().is_empty());
	assert!(desk.flow.gate().stored_token().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_email_and_password_sign_in_without_two_factor() {
	let mut desk = Desk::open("password-plain").await;
	desk.flow.launch(None);
	let mut password = PasswordSignIn::new();
	assert!(
		password
			.submit(&desk.flow, PASSWORD_LOGIN.to_owned(), MOCK_PASSWORD.to_owned())
			.is_none(),
		"nothing is sent before the notice is accepted"
	);
	desk.acknowledge();
	assert!(
		password
			.submit(&desk.flow, PASSWORD_LOGIN.to_owned(), String::new())
			.is_none()
	);
	assert_eq!(password.problem(), Some(&PasswordProblem::Empty));
	assert!(requests(&desk, LOGIN).is_empty());

	let token = log_in(&mut desk, &mut password, PASSWORD_LOGIN, MOCK_PASSWORD)
		.await
		.expect("a password-only account gets its token at once");
	assert!(password.problem().is_none());
	finish(&mut desk, &mut password, token).await;

	let sent = requests(&desk, LOGIN);
	assert_eq!(sent.len(), 1);
	from_the_login_page(&sent[0]);
	let body = sent[0].json().expect("a json body");
	assert_eq!(body["login"], PASSWORD_LOGIN);
	assert_eq!(body["password"], MOCK_PASSWORD);
	assert_eq!(body["undelete"], false);
	let fields: Vec<&str> = body
		.as_object()
		.expect("an object")
		.keys()
		.map(String::as_str)
		.collect();
	assert_eq!(fields.len(), 5);
	for field in [
		"login",
		"password",
		"undelete",
		"login_source",
		"gift_code_sku_id",
	] {
		assert!(fields.contains(&field), "{field} is sent");
	}
	assert!(files_containing(&desk.scratch, MOCK_PASSWORD).is_empty());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_authenticator_code_completes_two_factor() {
	let mut desk = Desk::open("password-totp").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	assert!(
		log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD)
			.await
			.is_none()
	);
	let view = mfa_view(&password);
	assert_eq!(view.method, MfaMethod::Totp);
	assert!(view.methods.totp && view.methods.sms && view.methods.backup);
	assert!(!view.methods.security_key);
	assert_eq!(view.others(), [MfaMethod::Backup, MfaMethod::Sms]);
	assert!(desk.flow.session().is_none());
	assert_eq!(desk.server.control().mfa_tickets_issued(), 1);

	assert!(password.verify(" ".to_owned()).is_none());
	assert_eq!(password.problem(), Some(&PasswordProblem::NoCode));
	let work = password.verify("123 456".to_owned()).expect("the code is sent");
	assert!(matches!(password.stage(), PasswordStage::Verifying(_)));
	let token = answer(&mut password, work)
		.await
		.expect("the right code gives the token");
	finish(&mut desk, &mut password, token).await;

	let verify = requests(&desk, "/auth/mfa/totp");
	assert_eq!(verify.len(), 1);
	from_the_login_page(&verify[0]);
	let body = verify[0].json().expect("the code request has a body");
	assert_eq!(body["code"], TOTP_CODE);
	assert!(
		body["ticket"]
			.as_str()
			.is_some_and(|ticket| ticket.starts_with("mock-mfa-ticket."))
	);
	assert!(
		body["login_instance_id"]
			.as_str()
			.is_some_and(|instance| instance.starts_with("mock-login-instance-"))
	);
	assert!(files_containing(&desk.scratch, TOTP_CODE).is_empty());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_text_message_code_completes_two_factor() {
	let mut desk = Desk::open("password-sms").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	assert!(requests(&desk, TEXT).is_empty(), "no text before it is asked for");
	let work = password
		.choose(MfaMethod::Sms)
		.expect("asking for a text sends one");
	assert!(matches!(password.stage(), PasswordStage::Texting(_)));
	assert!(answer(&mut password, work).await.is_none());
	let view = mfa_view(&password);
	assert_eq!(view.method, MfaMethod::Sms);
	assert_eq!(view.phone.as_deref(), Some(SMS_PHONE));
	assert_eq!(view.others(), [MfaMethod::Totp, MfaMethod::Backup]);
	assert_eq!(desk.server.control().mfa_texts_sent(), 1);
	let text = requests(&desk, TEXT);
	assert_eq!(text.len(), 1);
	from_the_login_page(&text[0]);
	let body = text[0].json().expect("a json body");
	assert_eq!(body.as_object().map(|fields| fields.len()), Some(1));

	let work = password.verify(SMS_CODE.to_owned()).expect("the code is sent");
	let token = answer(&mut password, work)
		.await
		.expect("the texted code gives the token");
	finish(&mut desk, &mut password, token).await;
	assert_eq!(requests(&desk, "/auth/mfa/sms").len(), 1);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_backup_code_completes_two_factor() {
	let mut desk = Desk::open("password-backup").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	assert!(password.choose(MfaMethod::Backup).is_none());
	assert_eq!(mfa_view(&password).method, MfaMethod::Backup);
	let work = password.verify(BACKUP_CODE.to_owned()).expect("the code is sent");
	let token = answer(&mut password, work)
		.await
		.expect("a backup code gives the token");
	finish(&mut desk, &mut password, token).await;
	assert_eq!(requests(&desk, "/auth/mfa/backup").len(), 1);
	assert!(requests(&desk, "/auth/mfa/totp").is_empty());
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_password_is_explained_and_not_retried() {
	let mut desk = Desk::open("password-wrong").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	assert!(
		log_in(&mut desk, &mut password, PASSWORD_LOGIN, "not-the-password")
			.await
			.is_none()
	);
	assert_eq!(password.stage(), &PasswordStage::Form);
	let problem = password.problem().expect("the form says what went wrong");
	assert_eq!(
		problem,
		&PasswordProblem::WrongLogin(Some("Login or password is invalid.".to_owned()))
	);
	assert_eq!(problem.message(), "Login or password is invalid.");
	assert_eq!(requests(&desk, LOGIN).len(), 1);
	signed_out(&desk);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_code_keeps_the_two_factor_step_and_is_not_retried() {
	let mut desk = Desk::open("password-wrong-code").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	let work = password.verify("654321".to_owned()).expect("the code is sent");
	assert!(answer(&mut password, work).await.is_none());
	assert_eq!(mfa_view(&password).method, MfaMethod::Totp);
	assert_eq!(
		password.problem(),
		Some(&PasswordProblem::WrongCode(Some(
			"Invalid two-factor code".to_owned()
		)))
	);
	assert_eq!(requests(&desk, "/auth/mfa/totp").len(), 1);
	signed_out(&desk);

	let work = password
		.verify(TOTP_CODE.to_owned())
		.expect("another try is the user's");
	let token = answer(&mut password, work)
		.await
		.expect("the same ticket takes the right code");
	finish(&mut desk, &mut password, token).await;
	assert_eq!(requests(&desk, "/auth/mfa/totp").len(), 2);
	assert_eq!(requests(&desk, LOGIN).len(), 1);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_captcha_hands_over_to_web_login_once_and_saves_nothing() {
	let mut desk = Desk::open("password-captcha").await;
	desk.flow.launch(None);
	desk.acknowledge();
	desk.server
		.control()
		.respond_once("POST", LOGIN, Reply::captcha_required());
	let mut password = PasswordSignIn::new();
	assert!(
		log_in(&mut desk, &mut password, PASSWORD_LOGIN, MOCK_PASSWORD)
			.await
			.is_none()
	);
	assert_eq!(
		password.stage(),
		&PasswordStage::HandedOver(PasswordHandover::Captcha)
	);
	assert!(PasswordHandover::Captcha.message().contains("discord.com"));
	assert!(password.problem().is_none());
	let sent = requests(&desk, LOGIN);
	assert_eq!(sent.len(), 1, "a captcha is never retried");
	assert!(sent[0].injected);
	signed_out(&desk);
	assert!(files_containing(&desk.scratch, MOCK_PASSWORD).is_empty());

	let profiles = std::env::temp_dir().join(format!("kyoko-password-captcha-{}", std::process::id()));
	let mut web = WebSignIn::new(WebLoginConfig {
		login_url: Url::parse(&desk.server.login_page_url()).expect("login page url"),
		profiles: profiles.clone(),
	});
	let page = web.open(&desk.flow).expect("web login takes over");
	assert_eq!(page.url, desk.server.login_page_url());
	web.cancel();
	if let Some(profile) = web.release() {
		profile.remove();
	}
	let _ = std::fs::remove_dir_all(&profiles);

	password.back();
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert!(
		log_in(&mut desk, &mut password, CAPTCHA_LOGIN, MOCK_PASSWORD)
			.await
			.is_none()
	);
	assert_eq!(
		password.stage(),
		&PasswordStage::HandedOver(PasswordHandover::Captcha)
	);
	assert_eq!(requests(&desk, LOGIN).len(), 2);
	signed_out(&desk);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rate_limit_is_explained_and_not_retried() {
	let mut desk = Desk::open("password-limited").await;
	desk.flow.launch(None);
	desk.acknowledge();
	desk.server
		.control()
		.respond_once("POST", LOGIN, Reply::rate_limited(0.05, false));
	let mut password = PasswordSignIn::new();
	assert!(
		log_in(&mut desk, &mut password, PASSWORD_LOGIN, MOCK_PASSWORD)
			.await
			.is_none()
	);
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert_eq!(password.problem(), Some(&PasswordProblem::Refused));
	assert!(password.problem().is_some_and(|problem| {
		problem
			.message()
			.starts_with("Discord refused the sign-in for now.")
	}));
	tokio::time::sleep(std::time::Duration::from_millis(200)).await;
	assert_eq!(requests(&desk, LOGIN).len(), 1, "a rate limit is never retried");
	signed_out(&desk);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_security_key_only_account_is_sent_to_discord_com() {
	let mut desk = Desk::open("password-key").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	log_in(&mut desk, &mut password, SECURITY_KEY_LOGIN, MOCK_PASSWORD).await;
	assert_eq!(
		password.stage(),
		&PasswordStage::HandedOver(PasswordHandover::SecurityKey)
	);
	assert!(requests(&desk, "/auth/mfa/totp").is_empty());
	signed_out(&desk);
	desk.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn going_back_or_leaving_the_screen_drops_the_two_factor_step() {
	let mut desk = Desk::open("password-back").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut password = PasswordSignIn::new();
	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	let work = password.verify(TOTP_CODE.to_owned()).expect("the code is sent");
	password.back();
	assert!(
		matches!(password.stage(), PasswordStage::Verifying(_)),
		"a request in flight stays"
	);
	let token = answer(&mut password, work).await.expect("the token arrives");
	drop(token);
	password.signed_in(Some(bridge::Failure::Network));
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert_eq!(password.problem(), Some(&PasswordProblem::Network));

	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	password.back();
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert!(password.verify(TOTP_CODE.to_owned()).is_none());

	log_in(&mut desk, &mut password, MFA_LOGIN, MOCK_PASSWORD).await;
	let stale = password.verify(TOTP_CODE.to_owned()).expect("the code is sent");
	let check = desk
		.flow
		.submit(desk.server.token().to_owned())
		.expect("the token form still works");
	let checked = check.await;
	desk.flow.checked(checked);
	assert_eq!(desk.flow.screen(), &Screen::Main);
	password.follow(&desk.flow);
	assert_eq!(password.stage(), &PasswordStage::Form);
	assert!(
		answer(&mut password, stale).await.is_none(),
		"an answer for a dropped step is ignored"
	);
	assert_eq!(password.stage(), &PasswordStage::Form);
	desk.stop().await;
}
