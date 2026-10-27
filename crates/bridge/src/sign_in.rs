use std::future::Future;
use std::io;
use std::sync::Arc;

use login::{LoginError, TokenLogin};
use protocol::{Snowflake, Token, User};
use rest::ClientIdentity;
use session::{Command, Session};
use state::{ConnectionStatus, DisconnectReason, State};
use store::{Account, Store};
use url::Url;

pub type StartSession = Box<dyn Fn(Start) -> io::Result<Session>>;

pub struct Start {
	pub token: Token,
	pub remember: Remember,
	pub store: Arc<Store>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remember {
	Account,
	SessionOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
	Empty,
	Rejected,
	Network,
	Refused,
}

impl Failure {
	pub fn message(self) -> &'static str {
		match self {
			Failure::Empty => "Enter or copy a token first.",
			Failure::Rejected => "That token was rejected.",
			Failure::Network => "Could not reach Discord.",
			Failure::Refused => {
				"Discord refused the sign-in for now. Finish any check in the official client, then try again later."
			}
		}
	}
}

fn failure(error: &LoginError) -> Failure {
	match error {
		LoginError::InvalidToken => Failure::Rejected,
		LoginError::Refused => Failure::Refused,
		_ => Failure::Network,
	}
}

pub struct Verified {
	token: Token,
	user: User,
}

impl Verified {
	pub fn user(&self) -> &User {
		&self.user
	}
}

pub enum Checked {
	Saved(Verified),
	Unsaved { verified: Verified, reason: String },
	Failed(Failure),
}

#[derive(Clone)]
pub struct Gate {
	store: Arc<Store>,
	identity: ClientIdentity,
	rest_base: Url,
}

impl Gate {
	pub fn new(store: Arc<Store>, identity: ClientIdentity, rest_base: Url) -> Self {
		Self {
			store,
			identity,
			rest_base,
		}
	}

	pub fn store(&self) -> &Arc<Store> {
		&self.store
	}

	pub fn identity(&self) -> &ClientIdentity {
		&self.identity
	}

	pub fn qr_config(&self, gateway_url: Url) -> login::QrConfig {
		login::QrConfig {
			gateway_url,
			origin: protocol::fingerprint::WEB_ORIGIN.to_owned(),
			rest_base: self.rest_base.clone(),
			identity: self.identity.clone(),
			trust: session::Trust::default(),
		}
	}

	pub fn password_login(&self) -> login::PasswordLogin {
		login::PasswordLogin::new(self.rest_base.clone(), &self.identity)
	}

	pub fn stored_token(&self) -> Option<Token> {
		self.stored_sign_in().map(|(token, _)| token)
	}

	fn stored_sign_in(&self) -> Option<(Token, Option<Snowflake>)> {
		let current = self
			.store
			.accounts
			.current()
			.and_then(|user_id| account_token(&self.store, user_id).map(|token| (token, Some(user_id))));
		current.or_else(|| login::persist::load_token(&self.store).map(|token| (token, None)))
	}

	pub fn acknowledged(&self) -> bool {
		self.store.settings.read().first_run_acknowledged
	}

	pub fn acknowledge(&self) -> store::Result<()> {
		if self.acknowledged() {
			return Ok(());
		}
		self.store
			.update_settings(|settings| settings.first_run_acknowledged = true)
	}

	pub async fn check(self, pasted: String) -> Checked {
		if pasted.trim().is_empty() {
			return Checked::Failed(Failure::Empty);
		}
		let (token, user) = match TokenLogin::validate(self.rest_base.clone(), &self.identity, pasted).await {
			Ok(validated) => validated,
			Err(error) => return Checked::Failed(failure(&error)),
		};
		self.keep_verified(token, user).await
	}

	pub async fn check_token(self, token: Token) -> Checked {
		self.check(token.as_str().to_owned()).await
	}

	async fn keep_verified(&self, token: Token, user: User) -> Checked {
		let verified = Verified { token, user };
		let store = self.store.clone();
		let user = verified.user.clone();
		let token = Token::new(verified.token.as_str());
		let kept = tokio::task::spawn_blocking(move || keep(&store, &user, &token)).await;
		match kept {
			Ok(Ok(())) => Checked::Saved(verified),
			Ok(Err(error)) => Checked::Unsaved {
				verified,
				reason: error.to_string(),
			},
			Err(_) => Checked::Unsaved {
				verified,
				reason: "saving the sign-in stopped unexpectedly".to_owned(),
			},
		}
	}
}

fn keep(store: &Store, user: &User, token: &Token) -> store::Result<()> {
	store
		.accounts
		.save(user.id, &user.username, user.avatar.as_deref(), token.as_str())?;
	store.accounts.set_current(user.id)
}

fn account_token(store: &Store, user_id: Snowflake) -> Option<Token> {
	match store.accounts.token_for(user_id) {
		Ok(token) => token.map(Token::new),
		Err(error) => {
			tracing::warn!(%error, "the saved sign-in could not be read");
			None
		}
	}
}

fn drop_account_database(store: &Store, user_id: Snowflake) {
	if let Err(error) = store.remove_account_database(user_id) {
		tracing::debug!(%error, "the account's emptied cache file stays until it is closed");
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
	Ready,
	Checking,
	Failed(Failure),
	Unsaved(String),
	Expired,
	SignedOut,
	Unavailable,
	SignInAgain,
}

impl Status {
	pub fn message(&self) -> Option<String> {
		match self {
			Status::Ready | Status::SignedOut => None,
			Status::Checking => Some("Checking token...".to_owned()),
			Status::Failed(failure) => Some(failure.message().to_owned()),
			Status::Unsaved(reason) => Some(format!(
				"Windows Credential Manager could not save the token ({reason}). Continue for this session only, and sign in again next time."
			)),
			Status::Expired => Some("Your session expired. Sign in again to continue.".to_owned()),
			Status::Unavailable => Some("The session could not start. Try again.".to_owned()),
			Status::SignInAgain => {
				Some("That account's saved sign-in is gone. Sign in to it again to add it back.".to_owned())
			}
		}
	}

	pub fn is_error(&self) -> bool {
		matches!(
			self,
			Status::Failed(_)
				| Status::Unsaved(_)
				| Status::Expired
				| Status::Unavailable
				| Status::SignInAgain
		)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
	Login(Status),
	Main,
	SigningOut,
	Switching,
}

pub struct SignOut {
	session: Session,
	store: Arc<Store>,
	device: Arc<Store>,
}

impl SignOut {
	pub fn finish(self) {
		self.session.shutdown();
		let account = self.store.account();
		if let Some(user_id) = self.store.accounts.current()
			&& let Err(error) = self.store.sign_out(Some(user_id))
		{
			tracing::warn!(%error, "sign-out did not wipe everything");
		}
		drop(self.store);
		if let Some(user_id) = account {
			drop_account_database(&self.device, user_id);
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
	Account(Snowflake),
	Login,
}

pub struct Handover {
	session: Session,
	store: Arc<Store>,
	next: Next,
}

impl Handover {
	pub fn finish(self) -> Arrival {
		self.session.shutdown();
		let token = match self.next {
			Next::Account(user_id) => account_token(&self.store, user_id),
			Next::Login => None,
		};
		if let (Next::Account(user_id), Some(_)) = (self.next, &token)
			&& let Err(error) = self.store.accounts.set_current(user_id)
		{
			tracing::warn!(%error, "the account switched to was not remembered as current");
		}
		Arrival {
			next: self.next,
			token,
		}
	}
}

pub struct Arrival {
	next: Next,
	token: Option<Token>,
}

pub struct Removal {
	store: Arc<Store>,
	user_id: Snowflake,
}

impl Removal {
	pub fn finish(self) {
		if let Err(error) = self.store.accounts.remove(self.user_id) {
			tracing::warn!(%error, "the account was not removed completely");
		}
		drop_account_database(&self.store, self.user_id);
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEntry {
	pub account: Account,
	pub current: bool,
}

pub struct Flow {
	gate: Gate,
	start: StartSession,
	screen: Screen,
	session: Option<Session>,
	store: Option<Arc<Store>>,
	unsaved: Option<Verified>,
	adding: bool,
	generation: u64,
}

impl Flow {
	pub fn new(gate: Gate, start: StartSession) -> Self {
		Self {
			gate,
			start,
			screen: Screen::Login(Status::Ready),
			session: None,
			store: None,
			unsaved: None,
			adding: false,
			generation: 0,
		}
	}

	pub fn launch(&mut self, developer_token: Option<Token>) {
		let launch = match developer_token {
			Some(token) => Some((token, Remember::SessionOnly, None)),
			None => self
				.gate
				.stored_sign_in()
				.map(|(token, account)| (token, Remember::Account, account)),
		};
		if let Some((token, remember, account)) = launch {
			self.begin(token, remember, account);
		}
	}

	pub fn gate(&self) -> &Gate {
		&self.gate
	}

	pub fn screen(&self) -> &Screen {
		&self.screen
	}

	pub fn session(&self) -> Option<&Session> {
		self.session.as_ref()
	}

	pub fn store(&self) -> Option<&Arc<Store>> {
		self.store.as_ref()
	}

	pub fn generation(&self) -> u64 {
		self.generation
	}

	pub fn adding(&self) -> bool {
		self.adding
	}

	pub fn current_account(&self) -> Option<Snowflake> {
		let signed_in = self
			.session
			.as_ref()
			.and_then(|session| session.state().read().current_user_id());
		signed_in.or_else(|| self.store.as_ref().and_then(|store| store.account()))
	}

	pub fn accounts(&self) -> Vec<AccountEntry> {
		let current = self.current_account();
		let mut entries: Vec<AccountEntry> = self
			.gate
			.store
			.accounts
			.list()
			.into_iter()
			.map(|account| AccountEntry {
				current: Some(account.user_id) == current,
				account,
			})
			.collect();
		if !entries.iter().any(|entry| entry.current)
			&& let Some(account) = self.signed_in_account()
		{
			entries.insert(
				0,
				AccountEntry {
					account,
					current: true,
				},
			);
		}
		entries
	}

	fn signed_in_account(&self) -> Option<Account> {
		let state = self.session.as_ref()?.state().read();
		let user = state.current_user()?;
		Some(Account {
			user_id: user.id,
			username: user.username.clone(),
			avatar: user.avatar.clone(),
		})
	}

	pub fn submit(&mut self, pasted: String) -> Option<impl Future<Output = Checked> + Send + 'static> {
		let idle = matches!(&self.screen, Screen::Login(status) if *status != Status::Checking);
		if !idle || !self.gate.acknowledged() {
			return None;
		}
		self.begin_check();
		Some(self.gate.clone().check(pasted))
	}

	pub(crate) fn begin_check(&mut self) {
		self.unsaved = None;
		self.screen = Screen::Login(Status::Checking);
	}

	pub(crate) fn end_check(&mut self) {
		self.screen = Screen::Login(Status::Ready);
	}

	pub fn checked(&mut self, checked: Checked) {
		if self.screen != Screen::Login(Status::Checking) {
			return;
		}
		match checked {
			Checked::Saved(verified) => {
				let account = Some(verified.user.id);
				self.begin(verified.token, Remember::Account, account);
			}
			Checked::Unsaved { verified, reason } => {
				self.unsaved = Some(verified);
				self.screen = Screen::Login(Status::Unsaved(reason));
			}
			Checked::Failed(failure) => self.screen = Screen::Login(Status::Failed(failure)),
		}
	}

	pub fn unsaved_user(&self) -> Option<&User> {
		self.unsaved.as_ref().map(Verified::user)
	}

	pub fn discard_unsaved(&mut self) {
		if self.unsaved.take().is_some() {
			self.screen = Screen::Login(Status::Ready);
		}
	}

	pub fn continue_unsaved(&mut self) {
		if let Some(verified) = self.unsaved.take() {
			let account = Some(verified.user.id);
			self.begin(verified.token, Remember::SessionOnly, account);
		}
	}

	pub fn switch(&mut self, user_id: Snowflake) -> Option<Handover> {
		let known = self
			.gate
			.store
			.accounts
			.list()
			.iter()
			.any(|account| account.user_id == user_id);
		if !known || self.current_account() == Some(user_id) {
			return None;
		}
		self.hand_over(Next::Account(user_id))
	}

	pub fn add_account(&mut self) -> Option<Handover> {
		self.hand_over(Next::Login)
	}

	pub fn arrived(&mut self, arrival: Arrival) {
		if self.screen != Screen::Switching {
			return;
		}
		match (arrival.next, arrival.token) {
			(Next::Account(user_id), Some(token)) => self.begin(token, Remember::Account, Some(user_id)),
			(Next::Account(_), None) => {
				self.adding = true;
				self.screen = Screen::Login(Status::SignInAgain);
			}
			(Next::Login, _) => {
				self.adding = true;
				self.screen = Screen::Login(Status::Ready);
			}
		}
	}

	pub fn cancel_add(&mut self) {
		let idle = matches!(&self.screen, Screen::Login(status) if *status != Status::Checking);
		if !self.adding || !idle {
			return;
		}
		self.adding = false;
		self.unsaved = None;
		match self.gate.stored_sign_in() {
			Some((token, account)) => self.begin(token, Remember::Account, account),
			None => self.screen = Screen::Login(Status::Ready),
		}
	}

	pub fn remove_account(&self, user_id: Snowflake) -> Option<Removal> {
		if self.screen != Screen::Main || self.current_account() == Some(user_id) {
			return None;
		}
		Some(Removal {
			store: self.gate.store.clone(),
			user_id,
		})
	}

	pub fn observe(&mut self, state: &State) -> Option<Session> {
		let rejected = state.session().connection
			== ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed);
		if self.screen != Screen::Main || !rejected {
			return None;
		}
		tracing::info!("the token was rejected, returning to the sign-in screen");
		self.screen = Screen::Login(Status::Expired);
		self.store = None;
		self.session.take()
	}

	pub fn sign_out(&mut self) -> Option<SignOut> {
		if self.screen != Screen::Main {
			return None;
		}
		let session = self.session.take()?;
		session.send(Command::Logout);
		self.screen = Screen::SigningOut;
		Some(SignOut {
			session,
			store: self.store.take().unwrap_or_else(|| self.gate.store.clone()),
			device: self.gate.store.clone(),
		})
	}

	pub fn signed_out(&mut self) {
		if self.screen == Screen::SigningOut {
			self.screen = Screen::Login(Status::SignedOut);
		}
	}

	pub fn shutdown(&mut self) -> Option<Session> {
		self.unsaved = None;
		self.store = None;
		self.session.take()
	}

	fn hand_over(&mut self, next: Next) -> Option<Handover> {
		if self.screen != Screen::Main {
			return None;
		}
		let session = self.session.take()?;
		self.store = None;
		self.unsaved = None;
		self.screen = Screen::Switching;
		Some(Handover {
			session,
			store: self.gate.store.clone(),
			next,
		})
	}

	fn begin(&mut self, token: Token, remember: Remember, account: Option<Snowflake>) {
		let store = match account {
			Some(user_id) => self.gate.store.for_account(user_id).map(Arc::new),
			None => Ok(self.gate.store.clone()),
		};
		let started = store.map_err(io::Error::other).and_then(|store| {
			let session = (self.start)(Start {
				token,
				remember,
				store: store.clone(),
			})?;
			Ok((session, store))
		});
		match started {
			Ok((session, store)) => {
				self.session = Some(session);
				self.store = Some(store);
				self.adding = false;
				self.generation += 1;
				self.screen = Screen::Main;
			}
			Err(error) => {
				tracing::warn!(%error, "the session did not start");
				self.screen = Screen::Login(Status::Unavailable);
			}
		}
	}
}
