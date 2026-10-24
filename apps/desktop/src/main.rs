use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use clap::Parser;
use mockcord::{MockConfig, Server};
use protocol::Token;
use protocol::gateway::{CHROME_VERSION, FALLBACK_BUILD_NUMBER, IdentifyProperties};
use session::{Session, SessionConfig};
use state::State;
use store::{Paths, Store};
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;
use url::Url;

const MOCK_LOCALE: &str = "en-US";
const SUMMARY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Parser)]
#[command(name = "kyoko", version, about = "A native Discord client")]
struct Cli {
	#[arg(long, help = "Run headless against the in-process mock Discord server")]
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

struct Summary {
	guilds: usize,
	channels: usize,
	private_channels: usize,
	unread_channels: usize,
}

impl Summary {
	fn measure(state: &State) -> Self {
		let mut summary = Self {
			guilds: state.guilds().count(),
			channels: 0,
			private_channels: state.private_channels().count(),
			unread_channels: 0,
		};
		let channels = state
			.guilds()
			.flat_map(|guild| guild.channels.values())
			.chain(state.private_channels());
		for channel in channels {
			summary.channels += 1;
			let read = state
				.read_state(channel.id)
				.and_then(|read_state| read_state.last_message_id);
			if matches!((read, channel.last_message_id), (Some(read), Some(last)) if read < last) {
				summary.unread_channels += 1;
			}
		}
		summary
	}
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	init_tracing(&cli.log);
	if !cli.mock {
		println!("kyoko: the desktop UI is not built yet; run with --mock for a headless session");
		return Ok(());
	}
	let runtime = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.thread_name("kyoko-worker")
		.build()
		.context("start async runtime")?;
	runtime.block_on(run_mock(cli))
}

async fn run_mock(cli: Cli) -> Result<()> {
	let server = Server::start(MockConfig {
		seed: cli.seed,
		chaos: cli.chaos,
		live_traffic: cli.live_traffic,
		..MockConfig::default()
	})
	.await
	.context("start the mock server")?;
	let outcome = drive_session(&cli, &server).await;
	server.shutdown().await;
	outcome
}

async fn drive_session(cli: &Cli, server: &Server) -> Result<()> {
	let store = Arc::new(open_store(cli.data_dir.clone())?);
	let wake = Arc::new(Notify::new());
	let notifier = wake.clone();
	let session = Session::start(SessionConfig {
		store,
		rest_base: Url::parse(server.rest_base_url()).context("mock rest url")?,
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		identify: IdentifyProperties::web_client(MOCK_LOCALE, FALLBACK_BUILD_NUMBER, CHROME_VERSION),
		locale: MOCK_LOCALE.to_owned(),
		runtime: Some(tokio::runtime::Handle::current()),
		notify: Arc::new(move |_| notifier.notify_one()),
		hooks: None,
	})
	.context("start the session")?;
	let state = session.state().clone();
	let ready = tokio::time::timeout(SUMMARY_TIMEOUT, async {
		loop {
			let connected = {
				let state = state.read();
				state.current_user().is_some() && state.session().connected
			};
			if connected {
				break;
			}
			wake.notified().await;
		}
	})
	.await;
	match ready {
		Ok(()) => {
			let summary = Summary::measure(&state.read());
			tracing::info!(
				guilds = summary.guilds,
				channels = summary.channels,
				private_channels = summary.private_channels,
				unread_channels = summary.unread_channels,
				"connected to the mock server, press Ctrl-C to stop"
			);
		}
		Err(_) => tracing::warn!("the mock session did not become ready in time"),
	}
	let stopped = tokio::signal::ctrl_c().await;
	tokio::task::spawn_blocking(move || session.shutdown())
		.await
		.context("session shutdown panicked")?;
	stopped.context("wait for Ctrl-C")
}

fn open_store(data_dir: Option<PathBuf>) -> Result<Store> {
	let root =
		data_dir.unwrap_or_else(|| std::env::temp_dir().join(format!("kyoko-mock-{}", std::process::id())));
	let store = Store::open(Paths::at(&root).context("prepare data directories")?).context("open store")?;
	store
		.update_settings(|settings| settings.storage.plaintext_token = true)
		.context("configure the mock store")?;
	Ok(store)
}

fn init_tracing(filter: &str) {
	let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
	tracing_subscriber::fmt().with_env_filter(filter).init();
}
