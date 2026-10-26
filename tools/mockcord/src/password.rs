use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::scenario::Reply;
use crate::state::Shared;

pub const PASSWORD_LOGIN: &str = "mock@kyoko.invalid";
pub const MFA_LOGIN: &str = "mfa@kyoko.invalid";
pub const SECURITY_KEY_LOGIN: &str = "key@kyoko.invalid";
pub const CAPTCHA_LOGIN: &str = "captcha@kyoko.invalid";
pub const MOCK_PASSWORD: &str = "mock-password";
pub const TOTP_CODE: &str = "123456";
pub const SMS_CODE: &str = "246810";
pub const BACKUP_CODE: &str = "kyoko123";
pub const LIMITED_CODE: &str = "000000";
pub const SMS_PHONE: &str = "+*******0085";
const INVALID_LOGIN: &str = "Login or password is invalid.";
const INVALID_CODE: &str = "Invalid two-factor code";
const INVALID_TICKET: &str = "Invalid two-factor auth ticket";
const REQUIRED: &str = "This field is required";
const WEBAUTHN_CHALLENGE: &str = r#"{"publicKey":{"challenge":"bW9jaw","timeout":60000,"rpId":"discord.com","allowCredentials":[],"userVerification":"preferred"}}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Methods {
	Codes,
	SecurityKey,
}

#[derive(Debug, Default)]
struct Ticket {
	sms_sent: bool,
}

#[derive(Default)]
pub struct PasswordAuth {
	tickets: Mutex<HashMap<String, Ticket>>,
	issued: AtomicU64,
	texts: AtomicU64,
}

impl PasswordAuth {
	pub fn tickets_issued(&self) -> u64 {
		self.issued.load(Ordering::Relaxed)
	}

	pub fn texts_sent(&self) -> u64 {
		self.texts.load(Ordering::Relaxed)
	}

	fn issue(&self) -> (String, String) {
		let count = self.issued.fetch_add(1, Ordering::Relaxed) + 1;
		let ticket = format!("mock-mfa-ticket.{count:08x}");
		self.tickets.lock().insert(ticket.clone(), Ticket::default());
		(ticket, format!("mock-login-instance-{count:08x}"))
	}
}

#[derive(Deserialize)]
pub struct Login {
	login: Option<String>,
	password: Option<String>,
}

#[derive(Deserialize)]
pub struct Code {
	code: Option<String>,
	ticket: Option<String>,
}

#[derive(Deserialize)]
pub struct Sms {
	ticket: Option<String>,
}

fn field_errors(fields: &[&str], code: &str, message: &str) -> Reply {
	let errors: serde_json::Map<String, Value> = fields
		.iter()
		.map(|field| {
			(
				(*field).to_owned(),
				json!({ "_errors": [{ "code": code, "message": message }] }),
			)
		})
		.collect();
	Reply::json(
		400,
		json!({ "message": "Invalid Form Body", "code": 50035, "errors": errors }),
	)
}

fn filled(field: Option<&String>) -> Option<&str> {
	field.map(|text| text.trim()).filter(|text| !text.is_empty())
}

fn signed_in(shared: &Shared) -> Value {
	json!({
		"user_id": shared.fixtures.current_user.id.to_string(),
		"token": shared.current_token(),
		"user_settings": { "locale": "en-US", "theme": "dark" },
	})
}

fn challenge(shared: &Shared, methods: Methods) -> Value {
	let (ticket, instance) = shared.password.issue();
	let codes = methods == Methods::Codes;
	let mut body = json!({
		"user_id": shared.fixtures.current_user.id.to_string(),
		"mfa": true,
		"sms": codes,
		"ticket": ticket,
		"login_instance_id": instance,
		"backup": codes,
		"totp": codes,
	});
	if !codes {
		body["webauthn"] = Value::from(WEBAUTHN_CHALLENGE);
	}
	body
}

pub async fn login(State(shared): State<Arc<Shared>>, Json(body): Json<Login>) -> Response {
	let (Some(login), Some(password)) = (filled(body.login.as_ref()), filled(body.password.as_ref())) else {
		return field_errors(&["login", "password"], "BASE_TYPE_REQUIRED", REQUIRED).into_response();
	};
	if login.eq_ignore_ascii_case(CAPTCHA_LOGIN) {
		return Reply::captcha_required().into_response();
	}
	if password != MOCK_PASSWORD {
		return field_errors(&["login", "password"], "INVALID_LOGIN", INVALID_LOGIN).into_response();
	}
	let answer = if login.eq_ignore_ascii_case(PASSWORD_LOGIN) {
		signed_in(&shared)
	} else if login.eq_ignore_ascii_case(MFA_LOGIN) {
		challenge(&shared, Methods::Codes)
	} else if login.eq_ignore_ascii_case(SECURITY_KEY_LOGIN) {
		challenge(&shared, Methods::SecurityKey)
	} else {
		return field_errors(&["login", "password"], "INVALID_LOGIN", INVALID_LOGIN).into_response();
	};
	Json(answer).into_response()
}

pub async fn send_sms(State(shared): State<Arc<Shared>>, Json(body): Json<Sms>) -> Response {
	let ticket = filled(body.ticket.as_ref()).unwrap_or_default();
	let mut tickets = shared.password.tickets.lock();
	let Some(entry) = tickets.get_mut(ticket) else {
		return field_errors(&["ticket"], "INVALID_TICKET", INVALID_TICKET).into_response();
	};
	entry.sms_sent = true;
	shared.password.texts.fetch_add(1, Ordering::Relaxed);
	Json(json!({ "phone": SMS_PHONE })).into_response()
}

fn accepts(kind: &str, code: &str, ticket: &Ticket) -> bool {
	match kind {
		"totp" => code == TOTP_CODE || code == BACKUP_CODE,
		"sms" => ticket.sms_sent && code == SMS_CODE,
		"backup" => code == BACKUP_CODE,
		_ => false,
	}
}

pub async fn verify(
	State(shared): State<Arc<Shared>>,
	Path(kind): Path<String>,
	Json(body): Json<Code>,
) -> Response {
	let ticket = filled(body.ticket.as_ref()).unwrap_or_default();
	let Some(code) = filled(body.code.as_ref()) else {
		return field_errors(&["code"], "BASE_TYPE_REQUIRED", REQUIRED).into_response();
	};
	if code == LIMITED_CODE {
		return Reply::rate_limited(30.0, false).into_response();
	}
	let mut tickets = shared.password.tickets.lock();
	let Some(entry) = tickets.get(ticket) else {
		return field_errors(&["ticket"], "INVALID_TICKET", INVALID_TICKET).into_response();
	};
	if !accepts(&kind, code, entry) {
		return Reply::error(400, 60008, INVALID_CODE).into_response();
	}
	tickets.remove(ticket);
	drop(tickets);
	let mut answer = signed_in(&shared);
	if let Some(fields) = answer.as_object_mut() {
		fields.remove("user_id");
	}
	Json(answer).into_response()
}
