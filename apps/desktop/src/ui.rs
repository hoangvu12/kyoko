use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use bridge::{ChannelEntry, ChannelKind, GuildEntry, Inbox, MessageEntry, OpenChannel, Sidebar, Timeline};
use media::MediaService;
use protocol::Snowflake;
use session::{Command, Session};
use state::{Changes, ConnectionStatus, DisconnectReason, State};
use zgx::image::ImageData;
use zgx::reactive::Runtime;
use zgx_desktop::{Application, TaskSpawner, WindowInfo, WindowOptions};
use zgx_script::{ScriptHost, Shared, Value};

const UI_DIR_VARIABLE: &str = "KYOKO_UI_DIR";
const RELOAD_EVERY: Duration = Duration::from_millis(250);
const INITIALS: usize = 3;
const AVATAR_POINTS: f64 = 32.;
const AVATAR_IMAGE: &str = "avatar";

pub struct Avatars {
	pub media: MediaService,
	pub cdn: String,
	pub runtime: tokio::runtime::Handle,
}

pub fn run(
	session: Session,
	inbox: Inbox<Changes>,
	avatars: Avatars,
	open_busiest: bool,
	after_close: impl Fn() + 'static,
) -> Result<()> {
	let script = ui_dir().join("app.luau");
	Application::new()
		.window(WindowOptions {
			title: "Kyoko".into(),
			width: 1100.,
			height: 720.,
			min_size: Some((640., 420.)),
			..Default::default()
		})
		.run(move |cx| {
			let host = ScriptHost::new(&cx.ui.runtime, script);
			let window = Window {
				runtime: cx.ui.runtime.clone(),
				tasks: cx.tasks.clone(),
				info: cx.window_info.clone(),
			};
			let link = Link::install(&host, window, session, inbox.clone(), avatars);
			link.open_busiest.set(open_busiest);
			let drain = link.clone();
			cx.tasks.spawn(async move { drain.drain().await });
			if let Err(error) = host.reload() {
				tracing::warn!(%error, "the interface script did not load");
			}
			if cfg!(debug_assertions) {
				cx.tasks.spawn(host.watch(RELOAD_EVERY));
			}
			cx.on_closed(move || {
				link.shutdown();
				after_close();
			});
			cx.render(host.view());
		})
		.map_err(|error| anyhow!("the window failed: {error}"))
}

fn ui_dir() -> PathBuf {
	std::env::var_os(UI_DIR_VARIABLE)
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"))
}

struct Window {
	runtime: Runtime,
	tasks: TaskSpawner,
	info: WindowInfo,
}

struct Link {
	window: Window,
	host: ScriptHost,
	session: RefCell<Option<Session>>,
	inbox: Inbox<Changes>,
	avatars: Avatars,
	avatar_requested: Cell<bool>,
	open_busiest: Cell<bool>,
	sidebar: RefCell<Sidebar>,
	timeline: RefCell<Timeline>,
	guilds: Shared,
	channels: Shared,
	selected: Shared,
	guild_name: Shared,
	status: Shared,
	open_channel: Shared,
	channel_name: Shared,
	messages: Shared,
	avatar_ready: Shared,
}

impl Link {
	fn install(
		host: &ScriptHost,
		window: Window,
		session: Session,
		inbox: Inbox<Changes>,
		avatars: Avatars,
	) -> Rc<Link> {
		let link = Rc::new(Link {
			window,
			host: host.clone(),
			session: RefCell::new(Some(session)),
			inbox,
			avatars,
			avatar_requested: Cell::new(false),
			open_busiest: Cell::new(false),
			sidebar: RefCell::new(Sidebar::default()),
			timeline: RefCell::new(Timeline::default()),
			guilds: host.shared("guilds", Value::List(Rc::new([]))),
			channels: host.shared("channels", Value::List(Rc::new([]))),
			selected: host.shared("selected_guild", ""),
			guild_name: host.shared("guild_name", ""),
			status: host.shared("status", "Connecting"),
			open_channel: host.shared("open_channel", ""),
			channel_name: host.shared("channel_name", ""),
			messages: host.shared("messages", Value::List(Rc::new([]))),
			avatar_ready: host.shared("avatar_ready", false),
		});
		let actions = Rc::downgrade(&link);
		host.action("select_guild", move |argument| {
			if let (Some(link), Some(guild_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.select_guild(guild_id);
			}
		});
		let actions = Rc::downgrade(&link);
		host.action("select_channel", move |argument| {
			if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.select_channel(channel_id);
			}
		});
		link
	}

	async fn drain(self: Rc<Self>) {
		while let Some(batch) = self.inbox.next().await {
			tracing::trace!(items = batch.len(), "inbox batch");
			let mut changes = Changes::default();
			for item in batch {
				changes.merge(item);
			}
			self.apply(&changes);
		}
	}

	fn apply(&self, changes: &Changes) {
		let busiest = self.publish(changes);
		if let Some(open) = busiest {
			if let Some(guild_id) = open.guild_id {
				self.select_guild(guild_id);
			}
			self.select_channel(open.channel_id);
			tracing::info!(
				guilds = self.sidebar.borrow().guilds().len(),
				channels = self.sidebar.borrow().channels().len(),
				"opened the most recently active text channel"
			);
		}
	}

	fn publish(&self, changes: &Changes) -> Option<OpenChannel> {
		let state = self.state()?;
		let state = state.read();
		let newest = self.timeline.borrow().messages().last().map(|message| message.id);
		let refresh = self.sidebar.borrow_mut().apply(&state, changes);
		let moved = refresh.selected && self.open_guild() != self.sidebar.borrow().selected();
		let messages = if moved {
			self.timeline.borrow_mut().close()
		} else {
			self.timeline.borrow_mut().apply(&state, changes)
		};
		if changes.current_user {
			self.request_avatar(&state);
		}
		self.window.runtime.batch(|| {
			if changes.session || changes.current_user {
				self.status.set(status_text(&state).as_str().into());
			}
			if refresh.guilds {
				self.publish_guilds();
			}
			if refresh.channels || refresh.selected || refresh.guilds {
				self.publish_selection(&state);
			}
			if messages || moved {
				self.publish_timeline(&state);
			}
		});
		if messages {
			let timeline = self.timeline.borrow();
			let appended = timeline
				.messages()
				.iter()
				.filter(|message| newest.is_some_and(|newest| message.id > newest))
				.count();
			tracing::debug!(shown = timeline.messages().len(), appended, "timeline published");
		}
		let ready = self.open_busiest.get() && !self.sidebar.borrow().guilds().is_empty();
		if !ready {
			return None;
		}
		self.open_busiest.set(false);
		bridge::busiest_channel(&state)
	}

	fn open_guild(&self) -> Option<Snowflake> {
		self.timeline.borrow().open().and_then(|open| open.guild_id)
	}

	fn select_guild(&self, guild_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let command = {
			let state = shared.read();
			let command = self.sidebar.borrow_mut().select(&state, guild_id);
			if command.is_some() {
				self.timeline.borrow_mut().close();
			}
			self.window.runtime.batch(|| {
				self.publish_selection(&state);
				self.publish_timeline(&state);
			});
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	fn select_channel(&self, channel_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let command = {
			let state = shared.read();
			let sidebar = self.sidebar.borrow();
			let openable = sidebar
				.channels()
				.iter()
				.any(|channel| channel.id == channel_id && readable(channel.kind));
			if !openable {
				return;
			}
			let command = self
				.timeline
				.borrow_mut()
				.select(&state, sidebar.selected(), channel_id);
			self.window.runtime.batch(|| self.publish_timeline(&state));
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
	}

	fn request_avatar(&self, state: &State) {
		if self.avatar_requested.get() {
			return;
		}
		let Some(user) = state.current_user() else {
			return;
		};
		let pixels = bridge::device_pixels(AVATAR_POINTS, self.scale_factor());
		let Some(url) = bridge::avatar_url(&self.avatars.cdn, user, pixels) else {
			return;
		};
		self.avatar_requested.set(true);
		let media = self.avatars.media.clone();
		let started = Instant::now();
		let load = self
			.avatars
			.runtime
			.spawn(async move { bridge::load_avatar(&media, &url, pixels).await });
		let host = self.host.clone();
		let ready = self.avatar_ready.clone();
		self.window.tasks.spawn(async move {
			let decoded = match load.await {
				Ok(Ok(decoded)) => decoded,
				Ok(Err(error)) => {
					tracing::warn!(%error, "the avatar did not load");
					return;
				}
				Err(error) => {
					tracing::warn!(%error, "the avatar task failed");
					return;
				}
			};
			match ImageData::new(decoded.width, decoded.height, decoded.rgba) {
				Ok(image) => {
					host.image(AVATAR_IMAGE, Some(Arc::new(image)));
					ready.set(true.into());
					tracing::info!(
						pixels,
						elapsed_ms = started.elapsed().as_millis() as u64,
						"avatar decoded and handed to the window"
					);
				}
				Err(error) => tracing::warn!(error, "the avatar was not a usable image"),
			}
		});
	}

	fn scale_factor(&self) -> f64 {
		let info = &self.window.info;
		let current = info.current_display.with_untracked(|index| *index);
		info.displays.with_untracked(|displays| {
			current
				.and_then(|index| displays.get(index))
				.or_else(|| displays.iter().find(|display| display.primary))
				.map_or(1., |display| display.scale_factor)
		})
	}

	fn state(&self) -> Option<state::SharedState> {
		self.session
			.borrow()
			.as_ref()
			.map(|session| session.state().clone())
	}

	fn send(&self, command: Command) {
		if let Some(session) = self.session.borrow().as_ref() {
			session.send(command);
		}
	}

	fn publish_guilds(&self) {
		let sidebar = self.sidebar.borrow();
		let items = sidebar.guilds().iter().map(guild_value).collect();
		self.guilds.set(Value::List(items));
	}

	fn publish_selection(&self, state: &State) {
		let sidebar = self.sidebar.borrow();
		let items = sidebar.channels().iter().map(channel_value).collect();
		self.channels.set(Value::List(items));
		let selected = sidebar.selected();
		let id = selected.map(|id| id.to_string()).unwrap_or_default();
		self.selected.set(id.as_str().into());
		let name = selected
			.and_then(|id| state.guild(id))
			.map(|guild| guild.name.as_str())
			.unwrap_or_default();
		self.guild_name.set(name.into());
	}

	fn publish_timeline(&self, state: &State) {
		let timeline = self.timeline.borrow();
		let items = timeline.messages().iter().map(message_value).collect();
		self.messages.set(Value::List(items));
		let open = timeline.open();
		let id = open.map(|open| open.channel_id.to_string()).unwrap_or_default();
		self.open_channel.set(id.as_str().into());
		let name = open
			.and_then(|open| state.channel(open.channel_id))
			.and_then(|channel| channel.name.as_deref())
			.unwrap_or_default();
		self.channel_name.set(name.into());
	}

	fn shutdown(&self) {
		tracing::debug!("window closed, stopping the session");
		self.inbox.close();
		let session = self.session.borrow_mut().take();
		if let Some(session) = session {
			session.shutdown();
		}
		tracing::debug!("session stopped");
	}
}

fn readable(kind: ChannelKind) -> bool {
	matches!(
		kind,
		ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Thread
	)
}

fn snowflake(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn status_text(state: &State) -> String {
	match &state.session().connection {
		ConnectionStatus::Connecting => "Connecting".to_owned(),
		ConnectionStatus::Resuming => "Resuming".to_owned(),
		ConnectionStatus::Connected => match state.current_user() {
			Some(user) => format!("Signed in as {}", user.username),
			None => "Connected".to_owned(),
		},
		ConnectionStatus::Reconnecting { attempt, retry_in } => {
			format!(
				"Reconnecting in {}s, attempt {attempt}",
				retry_in.as_secs().max(1)
			)
		}
		ConnectionStatus::Disconnected(DisconnectReason::AuthenticationFailed) => {
			"Signed out: the token was rejected".to_owned()
		}
		ConnectionStatus::Disconnected(DisconnectReason::Rejected { message, .. }) => {
			format!("Disconnected: {message}")
		}
	}
}

fn initials(name: &str) -> String {
	name.split_whitespace()
		.filter_map(|word| word.chars().next())
		.take(INITIALS)
		.collect()
}

fn guild_value(guild: &GuildEntry) -> Value {
	record([
		("id", guild.id.to_string().as_str().into()),
		("name", guild.name.as_str().into()),
		("initials", initials(&guild.name).as_str().into()),
		("unread", guild.unread.into()),
		("mentions", f64::from(guild.mentions).into()),
	])
}

fn channel_value(channel: &ChannelEntry) -> Value {
	record([
		("id", channel.id.to_string().as_str().into()),
		("name", channel.name.as_str().into()),
		("kind", channel.kind.as_str().into()),
		("unread", channel.unread.into()),
		("muted", channel.muted.into()),
		("mentions", f64::from(channel.mentions).into()),
	])
}

fn message_value(message: &MessageEntry) -> Value {
	record([
		("id", message.id.to_string().as_str().into()),
		("author", message.author.as_str().into()),
		("content", message.content.as_str().into()),
		("timestamp", message.timestamp.as_str().into()),
	])
}

fn record<const N: usize>(mut fields: [(&str, Value); N]) -> Value {
	fields.sort_by(|left, right| left.0.cmp(right.0));
	Value::Map(
		fields
			.into_iter()
			.map(|(key, value)| (Rc::from(key), value))
			.collect(),
	)
}
