use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use login::{LoginError, LoginStep, MfaChallenge, MfaMethod, MfaMethods, PasswordLogin};
use protocol::Token;
use zeroize::Zeroizing;

use crate::sign_in::{Failure, Flow, Screen, Status};

pub type PasswordWork = Pin<Box<dyn Future<Output = PasswordReply> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordProblem {
	Empty,
	NoCode,
	WrongLogin(Option<String>),
	WrongCode(Option<String>),
	Refused,
	Network,
	Rejected,
}

impl PasswordProblem {
	fn of(error: LoginError) -> PasswordProblem {
		match error {
			LoginError::WrongLogin(message) => PasswordProblem::WrongLogin(message),
			LoginError::WrongCode(message) => PasswordProblem::WrongCode(message),
			LoginError::Refused => PasswordProblem::Refused,
			LoginError::InvalidToken => PasswordProblem::Rejected,
			_ => PasswordProblem::Network,
		}
	}

	fn after(failure: Failure) -> PasswordProblem {
		match failure {
			Failure::Empty | Failure::Rejected => PasswordProblem::Rejected,
			Failure::Network => PasswordProblem::Network,
			Failure::Refused => PasswordProblem::Refused,
		}
	}

	pub fn message(&self) -> String {
		match self {
			PasswordProblem::Empty => "Enter your email or phone number and your password.".to_owned(),
			PasswordProblem::NoCode => "Enter the code first.".to_owned(),
			PasswordProblem::WrongLogin(message) => message
				.clone()
				.unwrap_or_else(|| "Login or password is invalid.".to_owned()),
			PasswordProblem::WrongCode(message) => message
				.clone()
				.unwrap_or_else(|| "Invalid two-factor code.".to_owned()),
			PasswordProblem::Refused => Failure::Refused.message().to_owned(),
			PasswordProblem::Network => Failure::Network.message().to_owned(),
			PasswordProblem::Rejected => "Discord rejected this sign-in. Try again.".to_owned(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handover {
	Captcha,
	SecurityKey,
}

impl Handover {
	pub fn title(self) -> &'static str {
		match self {
			Handover::Captcha => "Discord wants a captcha",
			Handover::SecurityKey => "Discord wants your security key",
		}
	}

	pub fn message(self) -> &'static str {
		match self {
			Handover::Captcha => {
				"Kyoko can't show Discord's captcha in its own form. Finish signing in on discord.com in a Kyoko window. Kyoko won't retry the sign-in by itself."
			}
			Handover::SecurityKey => {
				"Kyoko's form can't use a security key. Finish signing in on discord.com in a Kyoko window."
			}
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MfaView {
	pub method: MfaMethod,
	pub methods: MfaMethods,
	pub phone: Option<String>,
}

impl MfaView {
	pub fn awaits_text(&self) -> bool {
		self.method == MfaMethod::Sms && self.phone.is_none()
	}

	pub fn others(&self) -> Vec<MfaMethod> {
		[MfaMethod::Totp, MfaMethod::Backup, MfaMethod::Sms]
			.into_iter()
			.filter(|method| *method != self.method && self.methods.allows(*method))
			.collect()
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PasswordStage {
	#[default]
	Form,
	LoggingIn,
	Mfa(MfaView),
	Texting(MfaView),
	Verifying(MfaView),
	SigningIn,
	HandedOver(Handover),
}

impl PasswordStage {
	pub fn busy(&self) -> bool {
		matches!(
			self,
			PasswordStage::LoggingIn
				| PasswordStage::Texting(_)
				| PasswordStage::Verifying(_)
				| PasswordStage::SigningIn
		)
	}
}

enum Outcome {
	Step(login::Result<LoginStep>),
	Texted(login::Result<Option<String>>),
	Verified(login::Result<Token>),
}

pub struct PasswordReply {
	generation: u64,
	outcome: Outcome,
}

#[derive(Default)]
pub struct PasswordSignIn {
	stage: PasswordStage,
	problem: Option<PasswordProblem>,
	client: Option<PasswordLogin>,
	challenge: Option<Arc<MfaChallenge>>,
	generation: u64,
}

fn idle(flow: &Flow) -> bool {
	matches!(flow.screen(), Screen::Login(status) if *status != Status::Checking)
		&& flow.gate().acknowledged()
}

fn work(future: impl Future<Output = PasswordReply> + Send + 'static) -> PasswordWork {
	Box::pin(future)
}

impl PasswordSignIn {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn stage(&self) -> &PasswordStage {
		&self.stage
	}

	pub fn problem(&self) -> Option<&PasswordProblem> {
		self.problem.as_ref()
	}

	pub fn submit(&mut self, flow: &Flow, login: String, password: String) -> Option<PasswordWork> {
		let password = Zeroizing::new(password);
		let login = Zeroizing::new(login);
		if !idle(flow) || self.stage != PasswordStage::Form {
			return None;
		}
		if login.trim().is_empty() || password.is_empty() {
			self.problem = Some(PasswordProblem::Empty);
			return None;
		}
		let client = flow.gate().password_login();
		self.reset(PasswordStage::LoggingIn);
		self.client = Some(client.clone());
		let generation = self.generation;
		Some(work(async move {
			let step = client.login(&login, &password).await;
			PasswordReply {
				generation,
				outcome: Outcome::Step(step),
			}
		}))
	}

	pub fn choose(&mut self, method: MfaMethod) -> Option<PasswordWork> {
		let PasswordStage::Mfa(view) = &self.stage else {
			return None;
		};
		if !view.methods.allows(method) || (view.method == method && !view.awaits_text()) {
			return None;
		}
		let view = view.clone();
		self.problem = None;
		if method != MfaMethod::Sms {
			self.stage = PasswordStage::Mfa(MfaView { method, ..view });
			return None;
		}
		self.send_text(view)
	}

	pub fn verify(&mut self, code: String) -> Option<PasswordWork> {
		let code = Zeroizing::new(code);
		let PasswordStage::Mfa(view) = &self.stage else {
			return None;
		};
		if view.awaits_text() {
			return None;
		}
		if code.trim().is_empty() {
			self.problem = Some(PasswordProblem::NoCode);
			return None;
		}
		let (client, challenge) = (self.client.clone()?, self.challenge.clone()?);
		let method = view.method;
		self.problem = None;
		self.stage = PasswordStage::Verifying(view.clone());
		let generation = self.generation;
		Some(work(async move {
			let token = client.verify(&challenge, method, &code).await;
			PasswordReply {
				generation,
				outcome: Outcome::Verified(token),
			}
		}))
	}

	pub fn apply(&mut self, reply: PasswordReply) -> Option<Token> {
		if reply.generation != self.generation {
			return None;
		}
		let stage = std::mem::take(&mut self.stage);
		match (stage, reply.outcome) {
			(PasswordStage::LoggingIn, Outcome::Step(step)) => self.stepped(step),
			(PasswordStage::Texting(view), Outcome::Texted(texted)) => {
				self.texted(view, texted);
				None
			}
			(PasswordStage::Verifying(view), Outcome::Verified(verified)) => self.verified(view, verified),
			(stage, _) => {
				self.stage = stage;
				None
			}
		}
	}

	pub fn signed_in(&mut self, failure: Option<Failure>) {
		if self.stage != PasswordStage::SigningIn {
			return;
		}
		self.reset(PasswordStage::Form);
		self.problem = failure.map(PasswordProblem::after);
	}

	pub fn back(&mut self) {
		if !self.stage.busy() {
			self.reset(PasswordStage::Form);
		}
	}

	pub fn follow(&mut self, flow: &Flow) {
		let on_login = matches!(flow.screen(), Screen::Login(_));
		if !on_login && (self.stage != PasswordStage::Form || self.problem.is_some()) {
			self.reset(PasswordStage::Form);
		}
	}

	fn reset(&mut self, stage: PasswordStage) {
		self.generation += 1;
		self.stage = stage;
		self.problem = None;
		self.client = None;
		self.challenge = None;
	}

	fn send_text(&mut self, view: MfaView) -> Option<PasswordWork> {
		let (client, challenge) = (self.client.clone()?, self.challenge.clone()?);
		self.stage = PasswordStage::Texting(view);
		let generation = self.generation;
		Some(work(async move {
			let texted = client.send_sms(&challenge).await;
			PasswordReply {
				generation,
				outcome: Outcome::Texted(texted),
			}
		}))
	}

	fn hand_over(&mut self, handover: Handover) {
		self.reset(PasswordStage::HandedOver(handover));
	}

	fn stepped(&mut self, step: login::Result<LoginStep>) -> Option<Token> {
		match step {
			Ok(LoginStep::Token(token)) => {
				self.stage = PasswordStage::SigningIn;
				Some(token)
			}
			Ok(LoginStep::Mfa(challenge)) => {
				let methods = challenge.methods();
				match methods.first() {
					Some(method) => {
						self.challenge = Some(Arc::new(challenge));
						self.stage = PasswordStage::Mfa(MfaView {
							method,
							methods,
							phone: None,
						});
					}
					None => self.hand_over(Handover::SecurityKey),
				}
				None
			}
			Err(LoginError::Captcha) => {
				self.hand_over(Handover::Captcha);
				None
			}
			Err(error) => {
				self.reset(PasswordStage::Form);
				self.problem = Some(PasswordProblem::of(error));
				None
			}
		}
	}

	fn texted(&mut self, view: MfaView, texted: login::Result<Option<String>>) {
		match texted {
			Ok(phone) => {
				self.stage = PasswordStage::Mfa(MfaView {
					method: MfaMethod::Sms,
					phone: Some(phone.unwrap_or_default()),
					..view
				});
			}
			Err(LoginError::Captcha) => self.hand_over(Handover::Captcha),
			Err(error) => {
				self.stage = PasswordStage::Mfa(view);
				self.problem = Some(PasswordProblem::of(error));
			}
		}
	}

	fn verified(&mut self, view: MfaView, verified: login::Result<Token>) -> Option<Token> {
		match verified {
			Ok(token) => {
				self.stage = PasswordStage::SigningIn;
				self.challenge = None;
				Some(token)
			}
			Err(LoginError::Captcha) => {
				self.hand_over(Handover::Captcha);
				None
			}
			Err(error) => {
				self.stage = PasswordStage::Mfa(view);
				self.problem = Some(PasswordProblem::of(error));
				None
			}
		}
	}
}
