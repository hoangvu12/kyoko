use protocol::Token;
use protocol::fingerprint::LOGIN_REFERER;
use rest::{ClientIdentity, Request, RestClient, RestConfig, RestError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;
use zeroize::Zeroizing;

use crate::error::{LoginError, Result};
use crate::token_login::{clean_token, plausible, without_retries};

const LOGIN_PATH: &str = "/auth/login";
const SMS_SEND_PATH: &str = "/auth/mfa/sms/send";
const MAX_MESSAGE_CHARS: usize = 300;
const GENERIC_FORM_MESSAGE: &str = "Invalid Form Body";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaMethod {
	Totp,
	Sms,
	Backup,
}

impl MfaMethod {
	pub fn name(self) -> &'static str {
		match self {
			MfaMethod::Totp => "totp",
			MfaMethod::Sms => "sms",
			MfaMethod::Backup => "backup",
		}
	}

	fn path(self) -> String {
		format!("/auth/mfa/{}", self.name())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MfaMethods {
	pub totp: bool,
	pub sms: bool,
	pub backup: bool,
	pub security_key: bool,
}

impl MfaMethods {
	pub fn allows(self, method: MfaMethod) -> bool {
		match method {
			MfaMethod::Totp => self.totp,
			MfaMethod::Sms => self.sms,
			MfaMethod::Backup => self.backup,
		}
	}

	pub fn first(self) -> Option<MfaMethod> {
		[MfaMethod::Totp, MfaMethod::Sms, MfaMethod::Backup]
			.into_iter()
			.find(|method| self.allows(*method))
	}
}

pub struct MfaChallenge {
	ticket: Zeroizing<String>,
	login_instance_id: Option<String>,
	methods: MfaMethods,
}

impl MfaChallenge {
	pub fn methods(&self) -> MfaMethods {
		self.methods
	}
}

impl std::fmt::Debug for MfaChallenge {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("MfaChallenge")
			.field("methods", &self.methods)
			.finish_non_exhaustive()
	}
}

#[derive(Debug)]
pub enum LoginStep {
	Token(Token),
	Mfa(MfaChallenge),
}

#[derive(Serialize)]
struct Credentials<'a> {
	login: &'a str,
	password: &'a str,
	undelete: bool,
	login_source: Option<&'a str>,
	gift_code_sku_id: Option<&'a str>,
}

#[derive(Serialize)]
struct MfaCode<'a> {
	code: &'a str,
	ticket: &'a str,
	login_instance_id: Option<&'a str>,
	login_source: Option<&'a str>,
	gift_code_sku_id: Option<&'a str>,
}

#[derive(Serialize)]
struct SmsRequest<'a> {
	ticket: &'a str,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LoginReply {
	token: Option<String>,
	ticket: Option<String>,
	login_instance_id: Option<String>,
	mfa: bool,
	totp: bool,
	sms: bool,
	backup: bool,
	webauthn: Option<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TokenReply {
	token: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SmsReply {
	phone: Option<String>,
}

#[derive(Clone)]
pub struct PasswordLogin {
	rest: RestClient,
}

impl PasswordLogin {
	pub fn new(rest_base: Url, identity: &ClientIdentity) -> Self {
		let rest = RestClient::new(RestConfig {
			tuning: without_retries(),
			..RestConfig::new(rest_base, identity.clone())
		});
		rest.set_referer(LOGIN_REFERER);
		Self { rest }
	}

	pub async fn login(&self, login: &str, password: &str) -> Result<LoginStep> {
		let request = Request::post(LOGIN_PATH).json(&Credentials {
			login: login.trim(),
			password,
			undelete: false,
			login_source: None,
			gift_code_sku_id: None,
		});
		let reply: LoginReply = self
			.rest
			.send(request)
			.await
			.map_err(|error| refusal(error, LoginError::WrongLogin))?;
		step(reply)
	}

	pub async fn send_sms(&self, challenge: &MfaChallenge) -> Result<Option<String>> {
		let request = Request::post(SMS_SEND_PATH).json(&SmsRequest {
			ticket: &challenge.ticket,
		});
		let reply: SmsReply = self
			.rest
			.send(request)
			.await
			.map_err(|error| refusal(error, LoginError::WrongCode))?;
		Ok(reply.phone.map(|phone| plain_text(&phone)))
	}

	pub async fn verify(&self, challenge: &MfaChallenge, method: MfaMethod, code: &str) -> Result<Token> {
		let code = Zeroizing::new(code.split_whitespace().collect::<String>());
		let request = Request::post(method.path()).json(&MfaCode {
			code: &code,
			ticket: &challenge.ticket,
			login_instance_id: challenge.login_instance_id.as_deref(),
			login_source: None,
			gift_code_sku_id: None,
		});
		let reply: TokenReply = self
			.rest
			.send(request)
			.await
			.map_err(|error| refusal(error, LoginError::WrongCode))?;
		reply
			.token
			.as_deref()
			.and_then(token)
			.ok_or_else(|| LoginError::Protocol("the answer had no token".to_owned()))
	}
}

fn token(text: &str) -> Option<Token> {
	let text = Zeroizing::new(text.to_owned());
	let cleaned = clean_token(&text);
	plausible(cleaned).then(|| Token::new(cleaned))
}

fn step(reply: LoginReply) -> Result<LoginStep> {
	if let Some(token) = reply.token.as_deref().and_then(token) {
		return Ok(LoginStep::Token(token));
	}
	let ticket = reply
		.ticket
		.filter(|ticket| reply.mfa && !ticket.is_empty())
		.ok_or_else(|| LoginError::Protocol("the answer had neither a token nor a ticket".to_owned()))?;
	Ok(LoginStep::Mfa(MfaChallenge {
		ticket: Zeroizing::new(ticket),
		login_instance_id: reply.login_instance_id,
		methods: MfaMethods {
			totp: reply.totp,
			sms: reply.sms,
			backup: reply.backup,
			security_key: reply.webauthn.is_some_and(|challenge| !challenge.is_null()),
		},
	}))
}

fn refusal(error: RestError, rejected: fn(Option<String>) -> LoginError) -> LoginError {
	match error {
		RestError::Captcha { .. } => LoginError::Captcha,
		RestError::Http(status, api) if status.is_client_error() => rejected(api.and_then(|api| {
			api.errors
				.as_ref()
				.and_then(form_message)
				.or_else(|| (api.message != GENERIC_FORM_MESSAGE).then_some(api.message))
				.map(|message| plain_text(&message))
				.filter(|message| !message.is_empty())
		})),
		RestError::Forbidden | RestError::MissingPermissions => LoginError::Refused,
		other => LoginError::from(other),
	}
}

fn form_message(errors: &Value) -> Option<String> {
	let object = errors.as_object()?;
	let own = object
		.get("_errors")
		.and_then(Value::as_array)
		.and_then(|list| list.iter().find_map(|entry| entry.get("message")?.as_str()))
		.map(str::to_owned);
	own.or_else(|| {
		object
			.iter()
			.filter(|(key, _)| key.as_str() != "_errors")
			.find_map(|(_, nested)| form_message(nested))
	})
}

fn plain_text(text: &str) -> String {
	text.chars()
		.filter(|character| !character.is_control())
		.take(MAX_MESSAGE_CHARS)
		.collect::<String>()
		.trim()
		.to_owned()
}
