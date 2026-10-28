use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use bridge::themes::parse_color;
use bridge::{
	Arrival, Checked, Failure, Flow, Handover, Inbox, LoginChoice, LoginColumn, MfaMethod, MfaView,
	PasswordProblem, PasswordReply, PasswordSignIn, PasswordStage, PasswordWork, QrEvent, QrEvents, QrSignIn,
	QrStage, Scanner, Screen, Status, WebPage, WebSignIn, WebStage,
};
use media::ImageName;
use protocol::Snowflake;
use session::Session;
use state::{Changes, SecretStorage, State};
use tokio::runtime::Handle;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, Slot, field, qr_code, text};
use crate::web_page::{Bounds, PageEvent, PageHost};
use crate::{capture, theme};

const QR_IMAGE: &str = "login-qr";
const CAPTURE_TOKEN: &str = "kyoko-capture-token";
const CAPTURE_PASSWORD: &str = "kyoko-capture-password";

pub struct SignIn {
	pub flow: Flow,
	pub runtime: Handle,
	pub storage_warnings: bool,
	pub qr: QrSignIn,
	pub web: WebSignIn,
}

enum Event {
	Checked(Box<Checked>),
	SignedOut,
	Arrived(Box<Arrival>),
	AccountsChanged,
	Qr(u64, Box<QrEvent>),
	QrChecked(Box<Checked>),
	Web(PageEvent),
	WebPlaced,
	WebChecked(Box<Checked>),
	Password(Box<PasswordReply>),
	PasswordChecked(Box<Checked>),
}

pub(super) struct Account {
	runtime: Handle,
	events: Inbox<Event>,
	generation: Cell<u64>,
	storage_warnings: bool,
	screen: Shared,
	acknowledged: Shared,
	checking: Shared,
	message: Shared,
	error: Shared,
	unsaved: Shared,
	adding: Shared,
	storage_warning: Shared,
	qr: RefCell<QrSignIn>,
	qr_view: Shared,
	qr_image: RefCell<Option<String>>,
	qr_codes: Cell<u64>,
	scanner_avatar: RefCell<Option<ImageName>>,
	scanner_ready: Cell<bool>,
	web: RefCell<WebSignIn>,
	web_view: Shared,
	web_slot: Cell<Option<Slot>>,
	web_page: RefCell<Option<WebPage>>,
	web_host: RefCell<Option<PageHost>>,
	password: RefCell<PasswordSignIn>,
	password_view: Shared,
	mode: Shared,
	column: RefCell<LoginChoice>,
	unsaved_name: Shared,
	capture_code: RefCell<Option<String>>,
}

impl Account {
	pub(super) fn install(
		host: &ScriptHost,
		runtime: Handle,
		storage_warnings: bool,
		qr: QrSignIn,
		web: WebSignIn,
	) -> Account {
		Account {
			runtime,
			events: Inbox::new(),
			generation: Cell::new(0),
			storage_warnings,
			screen: host.shared("login.screen", "login"),
			acknowledged: host.shared("login.acknowledged", false),
			checking: host.shared("login.checking", false),
			message: host.shared("login.message", ""),
			error: host.shared("login.error", false),
			unsaved: host.shared("login.unsaved", false),
			adding: host.shared("login.adding", false),
			storage_warning: host.shared("session.storage_warning", ""),
			qr: RefCell::new(qr),
			qr_view: host.shared("login.qr", Value::Nil),
			qr_image: RefCell::new(None),
			qr_codes: Cell::new(0),
			scanner_avatar: RefCell::new(None),
			scanner_ready: Cell::new(false),
			web: RefCell::new(web),
			web_view: host.shared("login.web", Value::Nil),
			web_slot: Cell::new(None),
			web_page: RefCell::new(None),
			web_host: RefCell::new(None),
			password: RefCell::new(PasswordSignIn::new()),
			password_view: host.shared("login.password", Value::Nil),
			mode: host.shared("login.mode", LoginColumn::default().name()),
			column: RefCell::new(LoginChoice::default()),
			unsaved_name: host.shared("login.unsaved_name", ""),
			capture_code: RefCell::new(None),
		}
	}
}

impl Link {
	pub(super) fn install_sign_in(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("login.acknowledge", move |_| {
			if let Some(link) = actions.upgrade() {
				link.acknowledge();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.submit", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.submit(text(&argument).unwrap_or_default().to_owned());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.continue", move |_| {
			if let Some(link) = actions.upgrade() {
				link.flow.borrow_mut().continue_unsaved();
				link.after_flow_change();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.sign_out", move |_| {
			if let Some(link) = actions.upgrade() {
				link.sign_out();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.qr_refresh", move |_| {
			if let Some(link) = actions.upgrade() {
				link.refresh_qr();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.web_open", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_web();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.web_cancel", move |_| {
			if let Some(link) = actions.upgrade() {
				link.account.web.borrow_mut().cancel();
				link.sync_web();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.password_submit", move |argument| {
			if let Some(link) = actions.upgrade() {
				let typed = |key: &str| {
					field(&argument, key)
						.and_then(text)
						.unwrap_or_default()
						.to_owned()
				};
				link.submit_password(typed("login"), typed("password"));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.mfa_verify", move |argument| {
			if let Some(link) = actions.upgrade() {
				let code = text(&argument).unwrap_or_default().to_owned();
				let work = link.account.password.borrow_mut().verify(code);
				link.run_password(work);
				link.publish_password();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.mfa_method", move |argument| {
			if let Some(link) = actions.upgrade() {
				let method = text(&argument).and_then(method_named);
				let work = method.and_then(|method| link.account.password.borrow_mut().choose(method));
				link.run_password(work);
				link.publish_password();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.password_back", move |_| {
			if let Some(link) = actions.upgrade() {
				link.account.password.borrow_mut().back();
				link.publish_password();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.mode", move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(column) = text(&argument).and_then(LoginColumn::named)
			{
				link.choose_column(column);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.discard_unsaved", move |_| {
			if let Some(link) = actions.upgrade() {
				link.flow.borrow_mut().discard_unsaved();
				link.after_flow_change();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("login.cancel_add", move |_| {
			if let Some(link) = actions.upgrade() {
				link.flow.borrow_mut().cancel_add();
				link.after_flow_change();
			}
		});
		link.after_flow_change();
	}

	pub(super) async fn follow_sign_in(self: Rc<Self>) {
		while let Some(events) = self.account.events.next().await {
			for event in events {
				match event {
					Event::Checked(checked) => self.flow.borrow_mut().checked(*checked),
					Event::SignedOut => self.flow.borrow_mut().signed_out(),
					Event::Arrived(arrival) => {
						self.flow.borrow_mut().arrived(*arrival);
						self.announce_switch();
					}
					Event::AccountsChanged => self.show_accounts(),
					Event::Qr(generation, event) => self.on_qr(generation, *event),
					Event::QrChecked(checked) => {
						let failure = self.flow.borrow_mut().redeemed(*checked);
						self.account.qr.borrow_mut().signed_in(failure);
					}
					Event::Web(event) => self.on_page(event),
					Event::WebPlaced => self.show_page(),
					Event::WebChecked(checked) => {
						let failure = self.flow.borrow_mut().redeemed(*checked);
						self.account.web.borrow_mut().signed_in(failure);
					}
					Event::Password(reply) => self.on_password(*reply),
					Event::PasswordChecked(checked) => {
						let failure = self.flow.borrow_mut().redeemed(*checked);
						self.account.password.borrow_mut().signed_in(failure);
					}
				}
				self.after_flow_change();
			}
		}
	}

	pub(super) fn watch_session(&self, changes: &Changes) {
		let Some(shared) = self.state() else {
			return;
		};
		if changes.session && self.account.storage_warnings {
			let warning = storage_warning(&shared.read().session().secrets);
			self.account.storage_warning.set(warning.as_str().into());
		}
		let rejected = self.flow.borrow_mut().observe(&shared.read());
		if let Some(session) = rejected {
			self.release_media();
			self.account.runtime.spawn_blocking(move || session.shutdown());
			self.after_flow_change();
		}
	}

	pub(super) fn drive_capture(&self, screen: capture::Screen) {
		if screen != capture::Screen::LoginTos {
			self.acknowledge();
		}
		if matches!(
			screen,
			capture::Screen::LoginEmail
				| capture::Screen::Login2fa
				| capture::Screen::LoginCaptcha
				| capture::Screen::LoginUnsaved
		) {
			self.choose_column(LoginColumn::Email);
		}
		match screen {
			capture::Screen::LoginWeb => self.open_web(),
			capture::Screen::LoginToken => {
				self.choose_column(LoginColumn::Token);
				self.submit(CAPTURE_TOKEN.to_owned());
			}
			capture::Screen::LoginEmail => {
				self.submit_password(mockcord::PASSWORD_LOGIN.to_owned(), CAPTURE_PASSWORD.to_owned());
			}
			capture::Screen::Login2fa => {
				*self.account.capture_code.borrow_mut() = Some(mockcord::LIMITED_CODE.to_owned());
				self.submit_password(mockcord::MFA_LOGIN.to_owned(), mockcord::MOCK_PASSWORD.to_owned());
			}
			capture::Screen::LoginCaptcha => {
				self.submit_password(
					mockcord::CAPTCHA_LOGIN.to_owned(),
					mockcord::MOCK_PASSWORD.to_owned(),
				);
			}
			capture::Screen::LoginUnsaved => {
				self.submit_password(
					mockcord::PASSWORD_LOGIN.to_owned(),
					mockcord::MOCK_PASSWORD.to_owned(),
				);
			}
			_ => {}
		}
	}

	pub(super) fn login_settled(&self, screen: capture::Screen) -> bool {
		let flow = self.flow.borrow();
		matches!(flow.screen(), Screen::Login(_))
			&& (screen == capture::Screen::LoginTos || flow.gate().acknowledged())
			&& self.qr_settled(screen)
	}

	fn choose_column(&self, column: LoginColumn) {
		if self
			.account
			.column
			.borrow_mut()
			.choose(&self.flow.borrow(), column)
		{
			self.account.mode.set(column.name().into());
		}
	}

	fn submit_password(&self, login: String, password: String) {
		let work = self
			.account
			.password
			.borrow_mut()
			.submit(&self.flow.borrow(), login, password);
		self.run_password(work);
		self.publish_password();
	}

	fn run_password(&self, work: Option<PasswordWork>) {
		if let Some(work) = work {
			let events = self.account.events.clone();
			self.account
				.runtime
				.spawn(async move { events.push(Event::Password(Box::new(work.await))) });
		}
	}

	fn on_password(&self, reply: PasswordReply) {
		let token = self.account.password.borrow_mut().apply(reply);
		if let Some(token) = token {
			tracing::info!("signing in with the email form's token");
			let check = self.flow.borrow_mut().redeem(token);
			match check {
				Some(check) => {
					let events = self.account.events.clone();
					self.account
						.runtime
						.spawn(async move { events.push(Event::PasswordChecked(Box::new(check.await))) });
				}
				None => self
					.account
					.password
					.borrow_mut()
					.signed_in(Some(Failure::Rejected)),
			}
		}
		let waiting = matches!(self.account.password.borrow().stage(), PasswordStage::Mfa(_));
		let code = self.account.capture_code.borrow_mut().take_if(|_| waiting);
		if let Some(code) = code {
			let work = self.account.password.borrow_mut().verify(code);
			self.run_password(work);
		}
	}

	fn publish_password(&self) {
		let password = self.account.password.borrow();
		let mut view = match password.stage() {
			PasswordStage::Form => serde_json::json!({ "stage": "form" }),
			PasswordStage::LoggingIn => serde_json::json!({ "stage": "logging_in" }),
			PasswordStage::Mfa(view) => mfa_view("mfa", view),
			PasswordStage::Texting(view) => mfa_view("texting", view),
			PasswordStage::Verifying(view) => mfa_view("verifying", view),
			PasswordStage::SigningIn => serde_json::json!({ "stage": "signing_in" }),
			PasswordStage::HandedOver(handover) => serde_json::json!({
				"stage": "handover",
				"title": handover.title(),
				"message": handover.message(),
			}),
		};
		view["problem"] = password
			.problem()
			.map(PasswordProblem::message)
			.unwrap_or_default()
			.into();
		if let Some(value) = Value::from_json(&view.to_string()) {
			self.account.password_view.set(value);
		}
	}

	fn acknowledge(&self) {
		if let Err(error) = self.flow.borrow().gate().acknowledge() {
			tracing::warn!(%error, "the notice acknowledgement was not saved");
		}
		self.follow_qr();
		self.publish_sign_in();
	}

	fn submit(&self, typed: String) {
		let pasted = if typed.trim().is_empty() {
			clipboard_text()
		} else {
			typed
		};
		let check = self.flow.borrow_mut().submit(pasted);
		if let Some(check) = check {
			let events = self.account.events.clone();
			self.account
				.runtime
				.spawn(async move { events.push(Event::Checked(Box::new(check.await))) });
		}
		self.publish_sign_in();
	}

	pub(super) fn sign_out(&self) {
		let sign_out = self.flow.borrow_mut().sign_out();
		let Some(sign_out) = sign_out else {
			return;
		};
		tracing::info!("signing out");
		self.close_settings();
		self.release_media();
		let events = self.account.events.clone();
		self.account.runtime.spawn_blocking(move || {
			sign_out.finish();
			events.push(Event::SignedOut);
		});
		self.after_flow_change();
	}

	pub(super) fn switch_account(&self, user_id: Snowflake) {
		let handover = self.flow.borrow_mut().switch(user_id);
		if let Some(handover) = handover {
			tracing::info!("switching accounts");
			self.hand_over(handover);
		}
	}

	pub(super) fn add_account(&self) {
		let handover = self.flow.borrow_mut().add_account();
		if let Some(handover) = handover {
			tracing::info!("adding an account");
			self.hand_over(handover);
		}
	}

	pub(super) fn remove_account(&self, user_id: Snowflake) {
		let Some(removal) = self.flow.borrow().remove_account(user_id) else {
			return;
		};
		let events = self.account.events.clone();
		self.account.runtime.spawn_blocking(move || {
			removal.finish();
			events.push(Event::AccountsChanged);
		});
	}

	pub(super) fn show_accounts(&self) {
		let accounts = self.flow.borrow().accounts();
		self.profile.show_accounts(accounts);
	}

	fn announce_switch(&self) {
		let flow = self.flow.borrow();
		if *flow.screen() != Screen::Main {
			return;
		}
		let name = flow
			.accounts()
			.into_iter()
			.find(|entry| entry.current)
			.map(|entry| entry.account.username);
		if let Some(name) = name {
			crate::toasts::show(bridge::ToastKind::Success, &format!("Switched to {name}"));
		}
	}

	fn hand_over(&self, handover: Handover) {
		self.close_account_views();
		self.release_media();
		self.window.runtime.batch(|| self.reset_session_views());
		let events = self.account.events.clone();
		self.account.runtime.spawn_blocking(move || {
			let arrival = handover.finish();
			events.push(Event::Arrived(Box::new(arrival)));
		});
		self.after_flow_change();
	}

	fn close_account_views(&self) {
		self.close_settings();
		self.close_server_settings();
		self.close_thread_pane(false);
		self.close_expressions();
		self.close_apps();
		self.profile.close_all();
		self.community.close();
		self.moderation.close_menu();
		self.timeline_view.close_lightbox();
		self.finder.reset();
		self.joins.reset();
	}

	fn release_media(&self) {
		let gallery = &self.images.gallery;
		self.luau_visible.borrow_mut().clear();
		gallery.set_visible(std::iter::empty::<&str>());
		gallery.set_budget(0);
		self.timeline_view.pictures().listening().stop();
		*self.avatar.borrow_mut() = None;
		self.avatar_requested_at.set(None);
		self.avatar_shown.set(false);
	}

	fn after_flow_change(&self) {
		let generation = self.flow.borrow().generation();
		if self.account.generation.replace(generation) != generation {
			self.session_started();
		}
		self.follow_qr();
		self.sync_web();
		self.account.password.borrow_mut().follow(&self.flow.borrow());
		self.account.column.borrow_mut().follow(&self.flow.borrow());
		self.publish_sign_in();
	}

	fn follow_qr(&self) {
		let started = {
			let _runtime = self.account.runtime.enter();
			self.account.qr.borrow_mut().follow(&self.flow.borrow())
		};
		if let Some(events) = started {
			self.listen_qr(events);
		}
	}

	fn refresh_qr(&self) {
		let started = {
			let _runtime = self.account.runtime.enter();
			self.account.qr.borrow_mut().refresh(&self.flow.borrow())
		};
		if let Some(events) = started {
			self.listen_qr(events);
		}
		self.publish_qr();
	}

	fn listen_qr(&self, events: QrEvents) {
		let inbox = self.account.events.clone();
		self.account.runtime.spawn(async move {
			let QrEvents {
				generation,
				mut events,
			} = events;
			while let Some(event) = events.recv().await {
				inbox.push(Event::Qr(generation, Box::new(event)));
			}
		});
	}

	fn on_qr(&self, generation: u64, event: QrEvent) {
		let token = self.account.qr.borrow_mut().apply(generation, event);
		if let Some(token) = token {
			let check = self.flow.borrow_mut().redeem(token);
			match check {
				Some(check) => {
					let events = self.account.events.clone();
					self.account
						.runtime
						.spawn(async move { events.push(Event::QrChecked(Box::new(check.await))) });
				}
				None => self.account.qr.borrow_mut().signed_in(Some(Failure::Rejected)),
			}
		}
		let stage = self.account.qr.borrow().stage().clone();
		match &stage {
			QrStage::Code { url } => self.show_code(url),
			QrStage::Scanned(scanner) => self.request_scanner_avatar(scanner),
			_ => {}
		}
	}

	fn palette_size(&self, key: &str) -> f64 {
		let current = theme::current(&self.window.runtime);
		let settings = self.settings_view();
		let catalog = settings.catalog();
		let size = catalog
			.palettes
			.iter()
			.find(|palette| palette.name == current)
			.and_then(|palette| palette.size(key));
		f64::from(size.unwrap_or_default())
	}

	fn palette_color(&self, key: &str) -> Option<bridge::themes::Rgba> {
		let current = theme::current(&self.window.runtime);
		let settings = self.settings_view();
		let catalog = settings.catalog();
		catalog
			.palettes
			.iter()
			.find(|palette| palette.name == current)
			.and_then(|palette| palette.color(key))
			.and_then(parse_color)
	}

	fn show_code(&self, url: &str) {
		let points = self.palette_size("qr_size") - 2. * self.palette_size("space_8");
		let Some(ink) = self.palette_color("text_0") else {
			tracing::warn!("the theme has no colour for the QR code");
			return;
		};
		let side = (points * self.scale_factor()).round() as u32;
		let Some(image) = qr_code::render(url, side, ink) else {
			tracing::warn!("the QR code could not be drawn");
			return;
		};
		let count = self.account.qr_codes.get() + 1;
		self.account.qr_codes.set(count);
		let name = format!("{QR_IMAGE}-{count}");
		self.host.image(&name, Some(Arc::new(image)));
		if let Some(old) = self.account.qr_image.replace(Some(name)) {
			self.host.image(&old, None);
		}
	}

	fn request_scanner_avatar(&self, scanner: &Scanner) {
		let points = self.palette_size("avatar_size_xl");
		let scale = self.scale_factor();
		let Some(request) = bridge::scanner_avatar_request(&self.images.cdn, scanner, points, scale) else {
			return;
		};
		let name = self.images.gallery.request(request);
		self.account.scanner_ready.set(false);
		*self.account.scanner_avatar.borrow_mut() = Some(name);
		self.refresh_visible();
	}

	pub(super) fn login_images(&self) -> Vec<String> {
		self.account
			.scanner_avatar
			.borrow()
			.iter()
			.map(|name| name.as_str().to_owned())
			.collect()
	}

	pub(super) fn login_image_shown(&self, name: &ImageName) {
		if self.account.scanner_avatar.borrow().as_ref() == Some(name) {
			self.account.scanner_ready.set(true);
			self.publish_qr();
		}
	}

	pub(super) fn qr_settled(&self, wanted: capture::Screen) -> bool {
		let qr = self.account.qr.borrow();
		let password = self.account.password.borrow();
		let code_shown =
			matches!(qr.stage(), QrStage::Code { .. }) && self.account.qr_image.borrow().is_some();
		match (wanted, qr.stage()) {
			(capture::Screen::LoginQrScanned, QrStage::Scanned(_)) => self.account.scanner_ready.get(),
			(capture::Screen::LoginQrExpired, QrStage::Ended(_)) => true,
			(capture::Screen::LoginToken, _) => {
				code_shown && *self.flow.borrow().screen() == Screen::Login(Status::Failed(Failure::Rejected))
			}
			(capture::Screen::LoginEmail, _) => {
				code_shown && matches!(password.problem(), Some(PasswordProblem::WrongLogin(_)))
			}
			(capture::Screen::LoginCaptcha, _) => {
				code_shown && matches!(password.stage(), PasswordStage::HandedOver(_))
			}
			(capture::Screen::Login2fa, _) => {
				matches!(password.stage(), PasswordStage::Mfa(_))
					&& password.problem() == Some(&PasswordProblem::Refused)
			}
			(capture::Screen::LoginUnsaved, _) => {
				matches!(self.flow.borrow().screen(), Screen::Login(Status::Unsaved(_)))
			}
			(capture::Screen::LoginWeb, QrStage::Code { .. }) => {
				self.account.qr_image.borrow().is_some()
					&& self.account.web.borrow().loaded()
					&& self.account.web_host.borrow().is_some()
			}
			(capture::Screen::LoginTos, _) => true,
			_ => false,
		}
	}

	fn publish_qr(&self) {
		let account = &self.account;
		let image = account
			.qr_image
			.borrow()
			.as_ref()
			.map(|name| format!("host:{name}"))
			.unwrap_or_default();
		let avatar = account
			.scanner_avatar
			.borrow()
			.as_ref()
			.filter(|_| account.scanner_ready.get())
			.map(|name| format!("host:{name}"))
			.unwrap_or_default();
		let view = match account.qr.borrow().stage() {
			QrStage::Idle | QrStage::Waiting => serde_json::json!({ "stage": "waiting" }),
			QrStage::Code { .. } => serde_json::json!({ "stage": "code", "image": image }),
			QrStage::Scanned(scanner) => serde_json::json!({
				"stage": "scanned",
				"username": scanner.username,
				"avatar": avatar,
			}),
			QrStage::SigningIn => serde_json::json!({ "stage": "signing_in" }),
			QrStage::Ended(end) => serde_json::json!({
				"stage": "ended",
				"image": image,
				"message": end.message().unwrap_or_default(),
				"error": end.is_error(),
				"web": end.offers_web_login(),
			}),
		};
		if let Some(value) = Value::from_json(&view.to_string()) {
			account.qr_view.set(value);
		}
	}

	fn session_started(&self) {
		let gallery = &self.images.gallery;
		gallery.set_budget(self.images.budget);
		if let Some(commander) = self.flow.borrow().session().map(Session::commander) {
			gallery.set_refresher(move |origin| commander.send(bridge::refresh_command(origin)));
		}
		if let Some(store) = self.flow.borrow().store().cloned() {
			self.rail.restore(store);
		}
		self.themes_session_started();
		self.community.close();
		self.community.attach(self.state());
		self.window.runtime.batch(|| self.reset_session_views());
	}

	fn reset_session_views(&self) {
		self.timeline.borrow_mut().close();
		*self.avatar.borrow_mut() = None;
		self.avatar_requested_at.set(None);
		self.avatar_shown.set(false);
		self.rail.reset();
		self.rail_icons.borrow_mut().clear();
		self.sidebar.reset();
		self.sidebar_avatars.borrow_mut().clear();
		self.dms.reset();
		self.dm_avatars.borrow_mut().clear();
		self.friends.reset();
		self.dm_panel.reset();
		self.voice.reset();
		self.members.reset();
		self.member_avatars.borrow_mut().clear();
		for value in [
			&self.open_channel,
			&self.channel_name,
			&self.channel_topic,
			&self.avatar_name,
			&self.account.storage_warning,
		] {
			value.set("".into());
		}
		self.avatar_ready.set(false.into());
		self.status.set("Connecting".into());
		self.reset_connection();
		self.panel.set(super::profiles::empty_panel());
		self.panel_shown.borrow_mut().take();
		self.profile.set_panel(None);
		match self.state() {
			Some(shared) => self.publish_timeline(&shared.read()),
			None => self.publish_timeline(&State::default()),
		}
	}

	fn publish_sign_in(&self) {
		let flow = self.flow.borrow();
		let (screen, status) = match flow.screen() {
			Screen::Login(status) => ("login", Some(status)),
			Screen::Main | Screen::Switching => ("main", None),
			Screen::SigningOut => ("signing_out", None),
		};
		if screen != "main" {
			self.window.slot.set(Slot::default());
		}
		let account = &self.account;
		let message = status.and_then(Status::message).unwrap_or_default();
		self.window.runtime.batch(|| {
			account.screen.set(screen.into());
			account.acknowledged.set(flow.gate().acknowledged().into());
			account
				.checking
				.set(matches!(status, Some(Status::Checking)).into());
			account.message.set(message.as_str().into());
			account.error.set(status.is_some_and(Status::is_error).into());
			account
				.unsaved
				.set(matches!(status, Some(Status::Unsaved(_))).into());
			account.adding.set(flow.adding().into());
			account.mode.set(account.column.borrow().column().name().into());
			let unsaved = flow.unsaved_user().map(|user| user.username.as_str());
			account.unsaved_name.set(unsaved.unwrap_or_default().into());
		});
		drop(flow);
		self.publish_qr();
		self.publish_password();
	}
}

impl Link {
	fn open_web(&self) {
		let page = self.account.web.borrow_mut().open(&self.flow.borrow());
		if let Some(page) = page {
			*self.account.web_page.borrow_mut() = Some(page);
			self.show_page();
		}
		self.sync_web();
	}

	pub(super) fn place_web(&self, slot: Slot) {
		self.account.web_slot.set(Some(slot));
		self.account.events.push(Event::WebPlaced);
	}

	pub(super) fn close_web(&self) {
		if let Some(host) = self.account.web_host.borrow_mut().take() {
			host.close();
		}
		let profile = {
			let mut web = self.account.web.borrow_mut();
			web.cancel();
			web.release()
		};
		if let Some(profile) = profile {
			std::thread::spawn(move || profile.remove());
		}
	}

	fn page_bounds(&self) -> Option<Bounds> {
		let slot = self.account.web_slot.get()?;
		let scale = self.scale_factor();
		let pixels = |points: f32| (f64::from(points) * scale).round();
		(slot.width > 0. && slot.height > 0.).then(|| Bounds {
			x: pixels(slot.x) as i32,
			y: pixels(slot.y) as i32,
			width: pixels(slot.width) as u32,
			height: pixels(slot.height) as u32,
		})
	}

	fn show_page(&self) {
		let Some(bounds) = self.page_bounds() else {
			return;
		};
		if let Some(host) = self.account.web_host.borrow().as_ref() {
			host.place(bounds);
			return;
		}
		let Some(page) = self.account.web_page.borrow_mut().take() else {
			return;
		};
		let Some(frame) = self.window.frame.get() else {
			self.page_unavailable("the native window was not found");
			return;
		};
		let events = self.account.events.clone();
		let opened = PageHost::open(frame.handle(), &page, bounds, move |event| {
			events.push(Event::Web(event))
		});
		match opened {
			Ok(host) => *self.account.web_host.borrow_mut() = Some(host),
			Err(error) => self.page_unavailable(&error),
		}
	}

	fn page_unavailable(&self, error: &str) {
		tracing::warn!(%error, "the web login page could not be made");
		self.account.web.borrow_mut().unavailable();
		self.sync_web();
	}

	fn on_page(&self, event: PageEvent) {
		match event {
			PageEvent::Navigated(url) => self.account.web.borrow_mut().navigated(&url),
			PageEvent::Loaded(url) => {
				let mut web = self.account.web.borrow_mut();
				web.navigated(&url);
				web.page_loaded();
			}
			PageEvent::Message { source, body } => {
				let token = self.account.web.borrow_mut().message(&source, &body);
				if let Some(token) = token {
					self.hand_off(token);
				}
			}
		}
		self.publish_web();
	}

	fn hand_off(&self, token: protocol::Token) {
		let origin = self.account.web.borrow().config().origin();
		let cookies = self
			.account
			.web_host
			.borrow()
			.as_ref()
			.map(|host| host.cookies(&origin))
			.unwrap_or_default();
		self.account
			.web
			.borrow()
			.carry_cookies(&self.flow.borrow(), &cookies);
		self.sync_web();
		tracing::info!("signing in with the web login page's token");
		let check = self.flow.borrow_mut().redeem(token);
		match check {
			Some(check) => {
				let events = self.account.events.clone();
				self.account
					.runtime
					.spawn(async move { events.push(Event::WebChecked(Box::new(check.await))) });
			}
			None => self.account.web.borrow_mut().signed_in(Some(Failure::Rejected)),
		}
	}

	fn sync_web(&self) {
		let profile = {
			let mut web = self.account.web.borrow_mut();
			web.follow(&self.flow.borrow());
			web.release()
		};
		if !self.account.web.borrow().wants_page() {
			self.account.web_page.borrow_mut().take();
			self.account.web_slot.set(None);
			if let Some(host) = self.account.web_host.borrow_mut().take() {
				host.close();
			}
		}
		if let Some(profile) = profile {
			self.account.runtime.spawn_blocking(move || profile.remove());
		}
		self.publish_web();
	}

	fn publish_web(&self) {
		let web = self.account.web.borrow();
		let stage = web.stage();
		let address = web.address();
		let host = address
			.and_then(|url| {
				url.host_str().map(|host| match url.port() {
					Some(port) => format!("{host}:{port}"),
					None => host.to_owned(),
				})
			})
			.unwrap_or_default();
		let view = serde_json::json!({
			"open": matches!(stage, WebStage::Open | WebStage::SigningIn),
			"signing_in": stage == WebStage::SigningIn,
			"loaded": web.loaded(),
			"host": host,
			"path": address.map(url::Url::path).unwrap_or_default(),
			"secure": address.is_some_and(|url| url.scheme() == "https"),
			"message": match stage {
				WebStage::Ended(end) => end.message(),
				_ => "",
			},
			"error": matches!(stage, WebStage::Ended(_)),
		});
		if let Some(value) = Value::from_json(&view.to_string()) {
			self.account.web_view.set(value);
		}
	}
}

fn method_named(name: &str) -> Option<MfaMethod> {
	[MfaMethod::Totp, MfaMethod::Sms, MfaMethod::Backup]
		.into_iter()
		.find(|method| method.name() == name)
}

fn mfa_view(stage: &str, view: &MfaView) -> serde_json::Value {
	let others: Vec<&str> = view.others().into_iter().map(MfaMethod::name).collect();
	serde_json::json!({
		"stage": stage,
		"method": view.method.name(),
		"phone": view.phone.as_deref().unwrap_or_default(),
		"awaits_text": view.awaits_text(),
		"others": others,
	})
}

fn clipboard_text() -> String {
	match platform::clipboard::read_text() {
		Ok(text) => text.unwrap_or_default(),
		Err(error) => {
			tracing::warn!(%error, "the clipboard could not be read");
			String::new()
		}
	}
}

fn storage_warning(secrets: &SecretStorage) -> String {
	let mut warning = if secrets.plaintext {
		"Your token is saved in plain text on this computer, because plain-text token storage is turned on."
			.to_owned()
	} else if let Some(error) = &secrets.keyring_error {
		format!(
			"Windows Credential Manager is unavailable ({error}), so you will need to sign in again next time."
		)
	} else {
		String::new()
	};
	if secrets.cache_in_memory {
		if !warning.is_empty() {
			warning.push(' ');
		}
		warning.push_str("Messages are cached in memory only until Credential Manager is available.");
	}
	warning
}
