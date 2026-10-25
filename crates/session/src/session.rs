use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use gateway::{Gateway, GatewayConfig, GatewayHandle};
use protocol::{Message, Token};
use rest::{RestClient, RestConfig};
use state::{SharedState, State};
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::{mpsc, oneshot};

use crate::catalog::{ThemeCatalog, ThemeCatalogs};
use crate::command::Command;
use crate::config::SessionConfig;
use crate::hooks::{Hooks, guarded};
use crate::notifier::Notifier;
use crate::search::SearchResults;
use crate::task::{Core, CoreParts, SharedSearch, Startup, Stats};

const COMMAND_CAPACITY: usize = 256;
const RUNTIME_WORKERS: usize = 4;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const RUNTIME_GRACE: Duration = Duration::from_secs(2);

pub struct Session {
	state: SharedState,
	commands: mpsc::Sender<Command>,
	gateway: GatewayHandle,
	search: SharedSearch,
	stats: Arc<Stats>,
	themes: Arc<ThemeCatalogs>,
	hooks: Hooks,
	finished: std::sync::mpsc::Receiver<()>,
	runtime: Option<Runtime>,
}

fn install_crypto_provider() {
	let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

fn runtime_for(requested: Option<Handle>) -> io::Result<(Option<Runtime>, Handle)> {
	match requested {
		Some(handle) => Ok((None, handle)),
		None => {
			let runtime = Builder::new_multi_thread()
				.worker_threads(RUNTIME_WORKERS)
				.thread_name("kyoko-session")
				.enable_all()
				.build()?;
			let handle = runtime.handle().clone();
			Ok((Some(runtime), handle))
		}
	}
}

impl Session {
	pub fn start(config: SessionConfig) -> io::Result<Session> {
		install_crypto_provider();
		let (runtime, handle) = runtime_for(config.runtime)?;
		let locale = config.identity.fingerprint().locale().to_owned();
		let rest = RestClient::new(RestConfig {
			tuning: config.rest,
			..RestConfig::new(config.rest_base, config.identity.clone())
		});
		rest.set_token(Token::new(config.token.as_str()));
		let token = Arc::new(Token::new(config.token.as_str()));
		let mut gateway_config =
			GatewayConfig::new(config.token, config.identity.fingerprint().properties().clone());
		gateway_config.gateway_url = config.gateway_url;
		gateway_config.tuning = config.gateway;
		let (gateway_start, gateway_started) = oneshot::channel();
		let (gateway, gateway_events) =
			Gateway::spawn_when(async move { gateway_started.await.ok() }, handle.clone());
		let state = State::shared_with(config.caches);
		state
			.write()
			.set_safety_locks(config.store.settings.read().safety_locks);
		let search = SharedSearch::default();
		let stats = Arc::new(Stats::default());
		let themes = Arc::new(ThemeCatalogs::default());
		let (commands, command_receiver) = mpsc::channel(COMMAND_CAPACITY);
		let (internal, internal_receiver) = mpsc::unbounded_channel();
		let core = Core::new(CoreParts {
			state: state.clone(),
			rest,
			gateway: gateway.clone(),
			store: config.store,
			internal,
			hooks: config.hooks.clone(),
			themes: themes.clone(),
			locale,
			notifier: Notifier::new(config.notify),
			runtime: handle.clone(),
			search: search.clone(),
			stats: stats.clone(),
			token,
			remember_account: config.remember_account,
			tuning: config.rest,
			startup: Startup {
				gateway: gateway_start,
				gateway_config,
				build_source: config.build_source,
			},
		});
		let (done, finished) = std::sync::mpsc::channel();
		handle.spawn(async move {
			core.run(gateway_events, command_receiver, internal_receiver)
				.await;
			let _ = done.send(());
		});
		Ok(Session {
			state,
			commands,
			gateway,
			search,
			stats,
			themes,
			hooks: config.hooks,
			finished,
			runtime,
		})
	}

	pub fn state(&self) -> &SharedState {
		&self.state
	}

	pub fn send(&self, command: Command) {
		if let Err(error) = self.commands.try_send(command) {
			tracing::warn!(%error, "session command was not queued");
		}
	}

	pub fn should_notify(&self, message: &Message) -> bool {
		guarded(&self.hooks, "allow_notification", |hooks| {
			hooks.allow_notification(message)
		})
		.unwrap_or(true)
	}

	pub fn theme_catalog(&self) -> Option<Arc<ThemeCatalog>> {
		self.themes.catalog()
	}

	pub fn theme_catalog_error(&self) -> Option<String> {
		self.themes.error()
	}

	pub fn search_results(&self) -> Option<Arc<SearchResults>> {
		self.search.lock().clone()
	}

	pub fn reconnects(&self) -> u64 {
		self.stats.reconnects.load(Ordering::Relaxed)
	}

	pub fn shutdown(mut self) {
		self.gateway.shutdown();
		let _ = self.commands.try_send(Command::Shutdown);
		if self.finished.recv_timeout(SHUTDOWN_WAIT).is_err() {
			tracing::warn!("session did not stop in time");
		}
		if let Some(runtime) = self.runtime.take() {
			runtime.shutdown_timeout(RUNTIME_GRACE);
		}
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		self.gateway.shutdown();
		let _ = self.commands.try_send(Command::Shutdown);
		if let Some(runtime) = self.runtime.take() {
			runtime.shutdown_background();
		}
	}
}
