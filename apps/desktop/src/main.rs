#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod churn;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use bridge::{Inbox, changes_notifier};
use capture::{Capture, Opening, Screen};
use clap::Parser;
use media::{Gallery, Limits, MediaService, Playback};
use mockcord::{MockConfig, Server};
use protocol::Token;
use session::{Session, SessionConfig};
use state::Changes;
use store::{AnimationPlayback, MediaSettings, Paths, Settings, Store};
use tracing_subscriber::EnvFilter;
use url::Url;

const TOKEN_VARIABLE: &str = "KYOKO_TOKEN";
const MIB: usize = 1024 * 1024;

#[derive(Parser)]
#[command(name = "kyoko", version, about = "A native Discord client")]
struct Cli {
	#[arg(long, help = "Run against the in-process mock Discord server")]
	mock: bool,
	#[arg(
		long,
		default_value_t = 1,
		help = "Seed for the mock server fixtures, used with --mock"
	)]
	seed: u64,
	#[arg(
		long,
		help = "Make the mock server drop connections and rate limit, used with --mock"
	)]
	chaos: bool,
	#[arg(
		long,
		help = "Make the mock server send messages and typing on its own, used with --mock"
	)]
	live_traffic: bool,
	#[arg(
		long,
		value_name = "SECONDS",
		help = "Create a channel or guild on the mock server every few seconds, used with --mock"
	)]
	churn: Option<u64>,
	#[arg(
		long,
		value_name = "SECONDS",
		help = "Post a message into the open channel every few seconds, used with --mock"
	)]
	chatter: Option<u64>,
	#[arg(
		long,
		help = "Open the most recently active text channel once the guilds arrive"
	)]
	open_busiest: bool,
	#[arg(
		long,
		value_name = "URL",
		hide = true,
		conflicts_with = "mock",
		help = "Use a mock Discord server already running at this origin"
	)]
	mock_server: Option<Url>,
	#[arg(
		long,
		value_name = "NAME",
		hide = true,
		requires = "capture_ready",
		help = "Drive the window to a parity screen and signal when it settles, used by cargo xtask parity"
	)]
	capture_screen: Option<String>,
	#[arg(
		long,
		value_name = "PATH",
		hide = true,
		requires = "capture_screen",
		help = "File to write once the capture screen has settled"
	)]
	capture_ready: Option<PathBuf>,
	#[arg(
		long,
		value_name = "PATH",
		help = "Store settings and cache under this directory instead of a scratch one"
	)]
	data_dir: Option<PathBuf>,
	#[arg(
		long,
		value_name = "FILTER",
		default_value = "info",
		help = "Log filter, for example debug or session=trace"
	)]
	log: String,
}

struct Backend {
	rest_base: Url,
	gateway_url: String,
	token: Token,
	build_source: Option<Url>,
	remember_account: bool,
	cdn: String,
	loopback_port: Option<u16>,
}

#[derive(Clone, Default)]
struct Scratch(Arc<ScratchDir>);

#[derive(Default)]
struct ScratchDir(Mutex<Option<PathBuf>>);

impl Scratch {
	fn at(path: PathBuf) -> Self {
		Self(Arc::new(ScratchDir(Mutex::new(Some(path)))))
	}

	fn is_set(&self) -> bool {
		self.0.0.lock().is_ok_and(|path| path.is_some())
	}

	fn remove(&self) {
		self.0.remove();
	}
}

impl ScratchDir {
	fn remove(&self) {
		let Ok(mut slot) = self.0.lock() else {
			return;
		};
		let gone = slot
			.as_ref()
			.is_some_and(|path| std::fs::remove_dir_all(path).is_ok() || !path.exists());
		if gone {
			*slot = None;
			tracing::debug!("scratch data removed");
		}
	}
}

impl Drop for ScratchDir {
	fn drop(&mut self) {
		self.remove();
	}
}

fn main() -> Result<ExitCode> {
	let cli = Cli::parse();
	let capture = match (&cli.capture_screen, &cli.capture_ready) {
		(Some(name), Some(ready)) => match Screen::named(name) {
			Some(screen) => Some(Capture {
				name: name.clone(),
				screen,
				ready: ready.clone(),
			}),
			None => {
				eprintln!("kyoko: no capture steps for {name} yet");
				return Ok(ExitCode::from(capture::UNKNOWN_SCREEN));
			}
		},
		_ => None,
	};
	let mocked = cli.mock || cli.mock_server.is_some();
	if capture.is_some() && !mocked {
		bail!("--capture-screen needs --mock or --mock-server");
	}
	init_tracing(&cli.log);
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.thread_name("kyoko-worker")
		.build()
		.context("start async runtime")?;
	let (store, cache_dir, scratch) = open_store(cli.data_dir.clone(), mocked)?;
	let server = if cli.mock {
		Some(Arc::new(
			runtime
				.block_on(Server::start(MockConfig {
					seed: cli.seed,
					chaos: cli.chaos,
					live_traffic: cli.live_traffic,
					..MockConfig::default()
				}))
				.context("start the mock server")?,
		))
	} else {
		None
	};
	let backend = match (&server, &cli.mock_server) {
		(Some(server), _) => mock_backend(server)?,
		(None, Some(origin)) => external_mock_backend(origin)?,
		(None, None) => discord_backend()?,
	};
	let churn = match (&server, cli.churn) {
		(Some(server), Some(seconds)) => {
			Some(runtime.spawn(churn::run(server.clone(), Duration::from_secs(seconds.max(1)))))
		}
		_ => None,
	};
	let inbox: Inbox<Changes> = Inbox::new();
	let identity = session::client_identity(&store);
	let gallery = gallery(
		&identity,
		backend.loopback_port,
		cache_dir,
		&store.settings.read().media,
		runtime.handle(),
	)?;
	let reduced_motion = store.settings.read().accessibility.reduced_motion;
	let retention = session::Retention::for_settings(&store.settings.read().storage);
	let session = Session::start(SessionConfig {
		identity,
		retention,
		build_source: backend.build_source,
		store: Arc::new(store),
		rest_base: backend.rest_base,
		gateway_url: backend.gateway_url,
		token: backend.token,
		runtime: Some(runtime.handle().clone()),
		notify: changes_notifier(inbox.clone()),
		hooks: None,
		gateway: Default::default(),
		rest: Default::default(),
		caches: Default::default(),
		remember_account: backend.remember_account,
	})
	.context("start the session")?;
	let chatter = match (&server, cli.chatter) {
		(Some(server), Some(seconds)) => Some(runtime.spawn(churn::chatter(
			server.clone(),
			session.state().clone(),
			Duration::from_secs(seconds.max(1)),
		))),
		_ => None,
	};
	let opening = match &capture {
		Some(capture) => Some(capture.screen.opening()),
		None => cli.open_busiest.then_some(Opening::Busiest),
	};
	let outcome = ui::run(
		session,
		inbox,
		ui::Images {
			gallery,
			cdn: backend.cdn,
			reduced_motion,
		},
		opening,
		capture,
		{
			let scratch = scratch.clone();
			move || scratch.remove()
		},
	);
	tracing::debug!("window loop ended");
	for task in [churn, chatter].into_iter().flatten() {
		task.abort();
	}
	if let Some(server) = server {
		runtime.block_on(async move {
			if let Ok(server) = Arc::try_unwrap(server) {
				server.shutdown().await;
			}
		});
	}
	scratch.remove();
	outcome.map(|()| ExitCode::SUCCESS)
}

fn mock_backend(server: &Server) -> Result<Backend> {
	Ok(Backend {
		rest_base: Url::parse(server.rest_base_url()).context("mock rest url")?,
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		build_source: None,
		remember_account: true,
		cdn: server.cdn_url(),
		loopback_port: Some(server.address().port()),
	})
}

fn external_mock_backend(origin: &Url) -> Result<Backend> {
	let host = origin.host_str().context("the mock server url has no host")?;
	let port = origin
		.port_or_known_default()
		.context("the mock server url has no port")?;
	let base = format!("http://{host}:{port}");
	Ok(Backend {
		rest_base: Url::parse(&format!("{base}/api/v9")).context("mock rest url")?,
		gateway_url: format!("ws://{host}:{port}/gateway"),
		token: Token::new(MockConfig::default().token),
		build_source: None,
		remember_account: true,
		cdn: format!("{base}/cdn"),
		loopback_port: Some(port),
	})
}

fn discord_backend() -> Result<Backend> {
	let token = std::env::var(TOKEN_VARIABLE)
		.ok()
		.map(|token| token.trim().to_owned())
		.filter(|token| !token.is_empty());
	let Some(token) = token else {
		bail!("set {TOKEN_VARIABLE} to sign in, or run with --mock");
	};
	Ok(Backend {
		rest_base: rest::RestConfig::default_base_url(),
		gateway_url: gateway::GatewayConfig::DEFAULT_GATEWAY_URL.to_owned(),
		token: Token::new(token),
		build_source: Some(session::discord_build_source()),
		remember_account: false,
		cdn: media::cdn::CDN.to_owned(),
		loopback_port: None,
	})
}

fn gallery(
	identity: &rest::ClientIdentity,
	loopback_port: Option<u16>,
	cache_dir: PathBuf,
	settings: &MediaSettings,
	runtime: &tokio::runtime::Handle,
) -> Result<Gallery> {
	let fetcher =
		bridge::media_fetcher(&identity.fingerprint(), loopback_port).context("build the media fetcher")?;
	let loader = MediaService::new(fetcher, cache_dir, Limits::default());
	let gallery = Gallery::with_budget(loader, runtime.clone(), settings.memory_mb.max(1) as usize * MIB);
	gallery.set_playback(match settings.animation {
		AnimationPlayback::OnHover => Playback::OnHover,
		AnimationPlayback::Never => Playback::Never,
		AnimationPlayback::Always | AnimationPlayback::Unknown => Playback::Always,
	});
	Ok(gallery)
}

fn open_store(data_dir: Option<PathBuf>, mock: bool) -> Result<(Store, PathBuf, Scratch)> {
	let (root, scratch) = match data_dir {
		Some(path) => (path, Scratch::default()),
		None => {
			let path = std::env::temp_dir().join(format!("kyoko-scratch-{}", std::process::id()));
			(path.clone(), Scratch::at(path))
		}
	};
	let paths = Paths::at(&root).context("prepare data directories")?;
	if mock || scratch.is_set() {
		let mut settings = Settings::load(&paths);
		settings.storage.plaintext_token = true;
		settings
			.save(&paths)
			.context("keep the store's secrets out of Credential Manager")?;
	}
	let cache_dir = paths.cache_dir().to_path_buf();
	let store = Store::open(paths).context("open store")?;
	Ok((store, cache_dir, scratch))
}

fn init_tracing(filter: &str) {
	let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
	tracing_subscriber::fmt().with_env_filter(filter).init();
}
