use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::Inbox;
use mockcord::{MockConfig, Server};
use protocol::Token;
use session::{Session, SessionConfig};
use state::State;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{FriendConfirm, FriendDialog};

const WAIT: Duration = Duration::from_secs(15);
const QUIET: Duration = Duration::from_millis(600);

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn open_session(server: &Server, root: &std::path::Path) -> Session {
	let paths = Paths::at(root.join("session")).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = Store::open(paths).expect("store opens");
	Session::start(SessionConfig {
		identity: session::client_identity(&store),
		build_source: None,
		store: Arc::new(store),
		rest_base: url::Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		runtime: None,
		notify: bridge::changes_notifier(Inbox::new()),
		hooks: None,
		gateway: Default::default(),
		rest: Default::default(),
		caches: Default::default(),
		alerts: Default::default(),
		retention: Default::default(),
		remember_account: false,
		theme_links: Default::default(),
		trust: Default::default(),
	})
	.expect("session starts")
}

fn until(session: &Session, what: &str, ready: impl Fn(&State) -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready(&session.state().read()) {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		std::thread::sleep(Duration::from_millis(20));
	}
}

fn relationship_requests(server: &Server) -> Vec<(String, String)> {
	server
		.control()
		.recording()
		.requests
		.iter()
		.filter(|request| {
			request.is("PUT", "/users/@me/relationships/*")
				|| request.is("DELETE", "/users/@me/relationships/*")
				|| request.is("POST", "/users/@me/relationships")
		})
		.map(|request| (request.method.clone(), request.path.clone()))
		.collect()
}

fn settle_quietly(app: &mut TestApp) {
	let until = Instant::now() + QUIET;
	while Instant::now() < until {
		app.frames(1);
		std::thread::sleep(Duration::from_millis(20));
	}
}

fn shown(app: &mut TestApp, texts: &[&str]) -> bool {
	let on_screen = app.texts();
	texts
		.iter()
		.all(|wanted| on_screen.iter().any(|text| text == wanted))
}

#[test]
fn friend_confirmations_send_nothing_on_cancel_and_one_request_on_confirm() {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(2)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let server = tokio
		.block_on(Server::start(MockConfig {
			record: true,
			..MockConfig::default()
		}))
		.expect("mockcord starts");
	let root = scratch("friend-confirmations");
	let session = {
		let _entered = tokio.enter();
		open_session(&server, &root)
	};
	until(&session, "a friend", |state| !state.friends().is_empty());
	let (friend, name) = {
		let state = session.state().read();
		let friend = state.friends()[0].user_id;
		(friend, state.display_name(None, friend))
	};
	let made: Rc<RefCell<Option<Rc<FriendDialog>>>> = Rc::default();
	let into = made.clone();
	let commander = session.commander();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let view = built.clone();
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let dialog = FriendDialog::new(runtime);
			dialog.on_command(move |command| commander.send(command));
			*view.borrow_mut() = Some(div().w_full().h_full().child(dialog.view()));
			*into.borrow_mut() = Some(dialog);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	let dialog = made.borrow().clone().expect("the dialog");
	let remove = FriendConfirm::Remove {
		user_id: friend,
		name: name.clone(),
	};
	let block = FriendConfirm::Block {
		user_id: friend,
		name: name.clone(),
		friend: true,
	};
	let remove_texts = [
		format!("Remove '{name}'"),
		format!("Are you sure you want to permanently remove {name} from your friends?"),
		"Remove Friend".to_owned(),
	];
	let block_texts = [
		format!("Block '{name}'?"),
		format!(
			"Are you sure you want to block {name}? Blocking will also remove them from your friends list."
		),
		"Block".to_owned(),
	];
	for (confirm, texts) in [(&remove, &remove_texts), (&block, &block_texts)] {
		let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
		dialog.ask(confirm.clone());
		assert!(
			app.wait_until(|app| shown(app, &texts) && app.find("friend-confirm-cancel").is_some()),
			"{confirm:?} shows Discord's wording"
		);
		app.frames(30);
		app.click("friend-confirm-cancel");
		settle_quietly(&mut app);
		assert!(!dialog.is_open(), "Cancel closes {confirm:?}");
		assert_eq!(
			relationship_requests(&server),
			Vec::<(String, String)>::new(),
			"Cancel on {confirm:?} sends nothing"
		);
	}
	let path = format!("/users/@me/relationships/{friend}");
	for (confirm, method, sent) in [(&block, "PUT", 1), (&remove, "DELETE", 2)] {
		dialog.ask(confirm.clone());
		assert!(
			app.wait_until(|app| app.find("friend-confirm-accept").is_some()),
			"{confirm:?} opens"
		);
		app.frames(30);
		app.click("friend-confirm-accept");
		app.frames(2);
		assert!(!dialog.is_open(), "confirming closes {confirm:?}");
		let deadline = Instant::now() + WAIT;
		while relationship_requests(&server).len() < sent {
			assert!(Instant::now() < deadline, "{confirm:?} sent nothing");
			std::thread::sleep(Duration::from_millis(20));
		}
		settle_quietly(&mut app);
		let requests = relationship_requests(&server);
		assert_eq!(requests.len(), sent, "{confirm:?} sends exactly one request");
		assert_eq!(
			requests
				.last()
				.map(|(method, path)| (method.as_str(), path.as_str())),
			Some((method, path.as_str()))
		);
	}
	session.shutdown();
	let _ = std::fs::remove_dir_all(&root);
}
