use login::{LoginError, QrConfig, QrEvent, QrLogin, QrSession, Scanner};
use media::ImageRequest;
use protocol::{Token, User};
use tokio::sync::mpsc::Receiver;

use crate::sign_in::{Failure, Flow, Screen, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrEnd {
	Expired,
	Cancelled,
	Captcha,
	Refused,
	Rejected,
	Network,
	Broken,
}

impl QrEnd {
	fn of(error: &LoginError) -> QrEnd {
		match error {
			LoginError::Expired => QrEnd::Expired,
			LoginError::Cancelled => QrEnd::Cancelled,
			LoginError::Captcha => QrEnd::Captcha,
			LoginError::Refused => QrEnd::Refused,
			LoginError::InvalidToken => QrEnd::Rejected,
			LoginError::Network(_) => QrEnd::Network,
			_ => QrEnd::Broken,
		}
	}

	fn after(failure: Failure) -> QrEnd {
		match failure {
			Failure::Empty | Failure::Rejected => QrEnd::Rejected,
			Failure::Network => QrEnd::Network,
			Failure::Refused => QrEnd::Refused,
		}
	}

	pub fn message(self) -> Option<&'static str> {
		match self {
			QrEnd::Expired => None,
			QrEnd::Cancelled => Some("The login was cancelled on your phone."),
			QrEnd::Captcha => Some(
				"Discord wants a captcha. Kyoko won't solve or retry it by itself: continue on discord.com to finish it on Discord's own page, or refresh to try again.",
			),
			QrEnd::Refused => Some(Failure::Refused.message()),
			QrEnd::Rejected => Some("Discord rejected this sign-in. Refresh to try again."),
			QrEnd::Network => Some(Failure::Network.message()),
			QrEnd::Broken => Some("The QR login stopped unexpectedly. Refresh to try again."),
		}
	}

	pub fn is_error(self) -> bool {
		!matches!(self, QrEnd::Expired | QrEnd::Cancelled)
	}

	pub fn offers_web_login(self) -> bool {
		self == QrEnd::Captcha
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrStage {
	Idle,
	Waiting,
	Code { url: String },
	Scanned(Scanner),
	SigningIn,
	Ended(QrEnd),
}

pub struct QrEvents {
	pub generation: u64,
	pub events: Receiver<QrEvent>,
}

pub struct QrSignIn {
	config: QrConfig,
	stage: QrStage,
	session: Option<QrSession>,
	generation: u64,
}

pub fn scanner_avatar_request(cdn: &str, scanner: &Scanner, points: f64, scale: f64) -> Option<ImageRequest> {
	let user = User {
		id: scanner.id,
		username: scanner.username.clone(),
		discriminator: scanner.discriminator.clone(),
		avatar: scanner.avatar.clone(),
		..User::default()
	};
	crate::images::avatar_request(cdn, &user, points, scale, false)
}

fn wants_code(flow: &Flow) -> bool {
	matches!(flow.screen(), Screen::Login(_)) && flow.gate().acknowledged()
}

impl QrSignIn {
	pub fn new(config: QrConfig) -> Self {
		Self {
			config,
			stage: QrStage::Idle,
			session: None,
			generation: 0,
		}
	}

	pub fn stage(&self) -> &QrStage {
		&self.stage
	}

	pub fn follow(&mut self, flow: &Flow) -> Option<QrEvents> {
		match (wants_code(flow), &self.stage) {
			(true, QrStage::Idle) => Some(self.start()),
			(false, QrStage::Idle) | (true, _) => None,
			(false, _) => {
				self.stop();
				None
			}
		}
	}

	pub fn refresh(&mut self, flow: &Flow) -> Option<QrEvents> {
		let idle = matches!(flow.screen(), Screen::Login(status) if *status != Status::Checking);
		if !idle || !wants_code(flow) || self.stage == QrStage::SigningIn {
			return None;
		}
		Some(self.start())
	}

	pub fn apply(&mut self, generation: u64, event: QrEvent) -> Option<Token> {
		if generation != self.generation || matches!(self.stage, QrStage::Idle | QrStage::Ended(_)) {
			return None;
		}
		match event {
			QrEvent::Code { url } => self.stage = QrStage::Code { url },
			QrEvent::Scanned(scanner) => self.stage = QrStage::Scanned(scanner),
			QrEvent::Token(token) => {
				self.session = None;
				self.stage = QrStage::SigningIn;
				return Some(token);
			}
			QrEvent::Expired => self.end(QrEnd::Expired),
			QrEvent::Cancelled => self.end(QrEnd::Cancelled),
			QrEvent::Failed(error) => self.end(QrEnd::of(&error)),
		}
		None
	}

	pub fn signed_in(&mut self, failure: Option<Failure>) {
		if self.stage != QrStage::SigningIn {
			return;
		}
		match failure {
			Some(failure) => self.end(QrEnd::after(failure)),
			None => self.stage = QrStage::Idle,
		}
	}

	pub fn stop(&mut self) {
		self.abort();
		self.stage = QrStage::Idle;
	}

	fn start(&mut self) -> QrEvents {
		self.abort();
		let mut session = QrLogin::start(self.config.clone());
		let events = session.events().expect("a new session hands out its events once");
		self.session = Some(session);
		self.stage = QrStage::Waiting;
		QrEvents {
			generation: self.generation,
			events,
		}
	}

	fn end(&mut self, end: QrEnd) {
		self.session = None;
		self.stage = QrStage::Ended(end);
	}

	fn abort(&mut self) {
		self.generation += 1;
		if let Some(session) = self.session.take() {
			session.abort();
		}
	}
}

impl Flow {
	pub fn redeem(&mut self, token: Token) -> Option<impl Future<Output = crate::Checked> + Send + 'static> {
		let idle = matches!(self.screen(), Screen::Login(status) if *status != Status::Checking);
		if !idle || !self.gate().acknowledged() {
			return None;
		}
		self.begin_check();
		Some(self.gate().clone().check_token(token))
	}

	pub fn redeemed(&mut self, checked: crate::Checked) -> Option<Failure> {
		if *self.screen() != Screen::Login(Status::Checking) {
			return None;
		}
		match checked {
			crate::Checked::Failed(failure) => {
				self.end_check();
				Some(failure)
			}
			other => {
				self.checked(other);
				None
			}
		}
	}
}
