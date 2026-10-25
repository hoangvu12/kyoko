#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod churn;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use bridge::{Inbox, changes_notifier};
use clap::Parser;
use media::{Limits, MediaFetcher, MediaService};
use mockcord::{MockConfig, Server};
use protocol::Token;
use session::{Session, SessionConfig};
use state::Changes;
use store::{Paths, Settings, Store};
use tracing_subscriber::EnvFilter;
use url::Url;

const TOKEN_VARIABLE: &str = "KYOKO_TOKEN";
const MEDIA_DIR: &str = "media";

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

struct Scratch(Option<PathBuf>);

impl Drop for Scratch {
	fn drop(&mut self) {
		if let Some(path) = self.0.take() {
			let _ = std::fs::remove_dir_all(path);
		}
	}
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	init_tracing(&cli.log);
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.thread_name("kyoko-worker")
		.build()
		.context("start async runtime")?;
	let (store, media_dir, scratch) = open_store(cli.data_dir.clone(), cli.mock)?;
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
	let backend = match &server {
		Some(server) => mock_backend(server)?,
		None => discord_backend()?,
	};
	let churn = match (&server, cli.churn) {
		(Some(server), Some(seconds)) => {
			Some(runtime.spawn(churn::run(server.clone(), Duration::from_secs(seconds.max(1)))))
		}
		_ => None,
	};
	let inbox: Inbox<Changes> = Inbox::new();
	let identity = session::client_identity(&store);
	let media = {
		let _entered = runtime.enter();
		media_service(&identity, backend.loopback_port, media_dir)?
	};
	let session = Session::start(SessionConfig {
		identity,
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
	let outcome = ui::run(
		session,
		inbox,
		ui::Avatars {
			media,
			cdn: backend.cdn,
			runtime: runtime.handle().clone(),
		},
		cli.open_busiest,
	);
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
	drop(scratch);
	outcome
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

fn media_service(
	identity: &rest::ClientIdentity,
	loopback_port: Option<u16>,
	dir: PathBuf,
) -> Result<MediaService> {
	let mut fetcher = MediaFetcher::builder().user_agent(identity.fingerprint().user_agent());
	if let Some(port) = loopback_port {
		fetcher = fetcher.allow_loopback_http(port);
	}
	let fetcher = fetcher.build().context("build the media fetcher")?;
	Ok(MediaService::new(fetcher, dir, Limits::default()))
}

fn open_store(data_dir: Option<PathBuf>, mock: bool) -> Result<(Store, PathBuf, Scratch)> {
	let (root, scratch) = match data_dir {
		Some(path) => (path, Scratch(None)),
		None => {
			let path = std::env::temp_dir().join(format!("kyoko-scratch-{}", std::process::id()));
			(path.clone(), Scratch(Some(path)))
		}
	};
	let paths = Paths::at(&root).context("prepare data directories")?;
	if mock || scratch.0.is_some() {
		let mut settings = Settings::load(&paths);
		settings.storage.plaintext_token = true;
		settings
			.save(&paths)
			.context("keep the store's secrets out of Credential Manager")?;
	}
	let media_dir = paths.cache_dir().join(MEDIA_DIR);
	let store = Store::open(paths).context("open store")?;
	Ok((store, media_dir, scratch))
}

fn init_tracing(filter: &str) {
	let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
	tracing_subscriber::fmt().with_env_filter(filter).init();
}
