use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use protocol::{AttachmentDescriptor, MessageFlags, Snowflake};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::cdn::Signature;
use crate::config::MockConfig;
use crate::control::Control;
use crate::fixtures::Fixtures;
use crate::json::parse_id;
use crate::ops::{Draft, UploadedFile};
use crate::rest;
use crate::state::{Shared, Urls};
use crate::traffic;
use crate::voice;

const ANCHOR_GRANULARITY_MS: i64 = 3_600_000;
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

pub struct Server {
	shared: Arc<Shared>,
	address: SocketAddr,
	stop: watch::Sender<bool>,
	tasks: Vec<JoinHandle<()>>,
	voice: voice::VoiceControl,
}

pub fn remote_auth_url(origin: &str) -> String {
	let socket_origin = origin.replacen("http", "ws", 1);
	format!("{socket_origin}{}?v=2", crate::remote_auth::PATH)
}

pub fn login_page_url(origin: &str) -> String {
	format!("{origin}{}", crate::web::LOGIN_PAGE)
}

fn fixture_anchor() -> DateTime<Utc> {
	let now_ms = Utc::now().timestamp_millis();
	DateTime::from_timestamp_millis(now_ms - now_ms % ANCHOR_GRANULARITY_MS).unwrap_or_default()
}

impl Server {
	pub async fn start(config: MockConfig) -> io::Result<Server> {
		let listener = TcpListener::bind(("127.0.0.1", config.port.unwrap_or(0))).await?;
		let address = listener.local_addr()?;
		let fixtures = Fixtures::generate(&config, config.anchor.unwrap_or_else(fixture_anchor));
		let (stop, stopped) = watch::channel(false);
		let cameras = config
			.cameras
			.as_ref()
			.map(|plan| voice::Cameras::new(plan, &fixtures));
		let voice = voice::spawn(stopped.clone(), cameras).await?;
		let urls = Urls {
			origin: format!("http://{address}"),
			rest: format!("http://{address}/api/v9"),
			gateway: format!("ws://{address}/gateway"),
			voice: voice.url,
		};
		let shared = Arc::new(Shared::new(config, fixtures, urls, stopped.clone()));
		let mut graceful = stopped;
		let serve = axum::serve(listener, rest::router(shared.clone())).with_graceful_shutdown(async move {
			let _ = graceful.wait_for(|stopped| *stopped).await;
		});
		let mut tasks = vec![tokio::spawn(async move {
			if let Err(error) = serve.await {
				tracing::error!(%error, "mock server stopped");
			}
		})];
		tasks.extend(voice.tasks);
		tasks.extend(traffic::spawn(&shared));
		Ok(Server {
			shared,
			address,
			stop,
			tasks,
			voice: voice.control,
		})
	}

	pub fn rest_base_url(&self) -> &str {
		&self.shared.urls.rest
	}

	pub fn rest_base_url_for(&self, version: u32) -> String {
		format!("{}/api/v{version}", self.shared.urls.origin)
	}

	pub fn app_url(&self) -> String {
		format!("{}/app", self.shared.urls.origin)
	}

	pub fn control(&self) -> Control {
		Control::new(self.shared.clone())
	}

	pub fn gateway_url(&self) -> &str {
		&self.shared.urls.gateway
	}

	pub fn voice_url(&self) -> &str {
		&self.shared.urls.voice
	}

	pub fn voice(&self) -> voice::VoiceControl {
		self.voice.clone()
	}

	pub fn remote_auth_url(&self) -> String {
		remote_auth_url(&self.shared.urls.origin)
	}

	pub fn login_page_url(&self) -> String {
		login_page_url(&self.shared.urls.origin)
	}

	pub fn address(&self) -> SocketAddr {
		self.address
	}

	pub fn token(&self) -> &str {
		&self.shared.config.token
	}

	pub fn alt_token(&self) -> &str {
		&self.shared.config.alt_token
	}

	pub fn fixtures(&self) -> &Fixtures {
		&self.shared.fixtures
	}

	pub fn create_guild(&self, name: &str) -> Option<Snowflake> {
		let guild = self.shared.create_guild(&json!({ "name": name })).ok()?;
		parse_id(&guild["id"]).map(Snowflake)
	}

	pub fn create_channel(&self, guild_id: Snowflake, name: &str) -> Option<Snowflake> {
		let channel = self
			.shared
			.create_channel(guild_id.0, &json!({ "name": name, "type": 0 }))
			.ok()?;
		parse_id(&channel["id"]).map(Snowflake)
	}

	pub fn cdn_url(&self) -> String {
		format!("{}/cdn", self.shared.urls.origin)
	}

	pub fn post_message(&self, channel_id: Snowflake, content: &str) -> Option<Snowflake> {
		let author = self.shared.fixtures.someone_in(channel_id.0)?.clone();
		let message = self
			.shared
			.post_message(channel_id.0, &author, Draft::text(content))
			.ok()?;
		parse_id(&message["id"]).map(Snowflake)
	}

	pub fn post_image(
		&self,
		channel_id: Snowflake,
		filename: &str,
		signature: Signature,
	) -> Option<Snowflake> {
		let author = self.shared.fixtures.someone_in(channel_id.0)?.clone();
		let draft = Draft {
			files: vec![UploadedFile {
				filename: filename.to_owned(),
				content_type: None,
				size: 0,
			}],
			hosted: Some(signature),
			..Draft::default()
		};
		let message = self.shared.post_message(channel_id.0, &author, draft).ok()?;
		parse_id(&message["id"]).map(Snowflake)
	}

	pub fn post_images(&self, channel_id: Snowflake, images: &[(&str, (u32, u32))]) -> Option<Snowflake> {
		let author = self.shared.fixtures.someone_in(channel_id.0)?.clone();
		let draft = Draft {
			files: images
				.iter()
				.map(|(filename, _)| UploadedFile {
					filename: (*filename).to_owned(),
					content_type: None,
					size: 0,
				})
				.collect(),
			sizes: images.iter().map(|(_, size)| *size).collect(),
			hosted: Some(Signature::Fresh),
			..Draft::default()
		};
		let message = self.shared.post_message(channel_id.0, &author, draft).ok()?;
		parse_id(&message["id"]).map(Snowflake)
	}

	pub fn post_audio(
		&self,
		channel_id: Snowflake,
		filename: &str,
		voice: bool,
		signature: Signature,
	) -> Option<Snowflake> {
		let author = self.shared.fixtures.someone_in(channel_id.0)?.clone();
		let descriptors = if voice {
			vec![AttachmentDescriptor {
				id: Snowflake(0),
				duration_secs: Some(crate::audio::seconds_of(filename)),
				waveform: Some(crate::audio::VOICE_WAVEFORM.to_owned()),
				..AttachmentDescriptor::default()
			}]
		} else {
			Vec::new()
		};
		let draft = Draft {
			files: vec![UploadedFile {
				filename: filename.to_owned(),
				content_type: crate::audio::content_type(filename).map(str::to_owned),
				size: crate::audio::size_of(filename),
			}],
			descriptors,
			flags: if voice {
				MessageFlags::IS_VOICE_MESSAGE
			} else {
				MessageFlags::empty()
			},
			hosted: Some(signature),
			..Draft::default()
		};
		let message = self.shared.post_message(channel_id.0, &author, draft).ok()?;
		parse_id(&message["id"]).map(Snowflake)
	}

	pub async fn shutdown(mut self) {
		let _ = self.stop.send(true);
		for mut task in self.tasks.drain(..) {
			if tokio::time::timeout(SHUTDOWN_GRACE, &mut task).await.is_err() {
				task.abort();
			}
		}
	}
}

impl Drop for Server {
	fn drop(&mut self) {
		let _ = self.stop.send(true);
		for task in &self.tasks {
			task.abort();
		}
	}
}
