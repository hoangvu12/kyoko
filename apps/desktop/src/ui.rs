use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use bridge::{ChannelEntry, GuildEntry, Inbox, Sidebar};
use protocol::Snowflake;
use session::{Command, Session};
use state::{Changes, ConnectionStatus, DisconnectReason, State};
use zgui::reactive::Runtime;
use zgui_desktop::{Application, WindowOptions};
use zgui_script::{ScriptHost, Shared, Value};

const UI_DIR_VARIABLE: &str = "KYOKO_UI_DIR";
const RELOAD_EVERY: Duration = Duration::from_millis(250);
const INITIALS: usize = 3;

pub fn run(session: Session, inbox: Inbox<Changes>) -> Result<()> {
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
			let link = Link::install(&host, cx.ui.runtime.clone(), session, inbox.clone());
			let drain = link.clone();
			cx.tasks.spawn(async move { drain.drain().await });
			if let Err(error) = host.reload() {
				tracing::warn!(%error, "the interface script did not load");
			}
			if cfg!(debug_assertions) {
				cx.tasks.spawn(host.watch(RELOAD_EVERY));
			}
			cx.on_closed(move || link.shutdown());
			cx.render(host.view());
		})
		.map_err(|error| anyhow!("the window failed: {error}"))
}

fn ui_dir() -> PathBuf {
	std::env::var_os(UI_DIR_VARIABLE)
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui"))
}

struct Link {
	runtime: Runtime,
	session: RefCell<Option<Session>>,
	inbox: Inbox<Changes>,
	sidebar: RefCell<Sidebar>,
	guilds: Shared,
	channels: Shared,
	selected: Shared,
	guild_name: Shared,
	status: Shared,
}

impl Link {
	fn install(host: &ScriptHost, runtime: Runtime, session: Session, inbox: Inbox<Changes>) -> Rc<Link> {
		let link = Rc::new(Link {
			runtime,
			session: RefCell::new(Some(session)),
			inbox,
			sidebar: RefCell::new(Sidebar::default()),
			guilds: host.shared("guilds", Value::List(Rc::new([]))),
			channels: host.shared("channels", Value::List(Rc::new([]))),
			selected: host.shared("selected_guild", ""),
			guild_name: host.shared("guild_name", ""),
			status: host.shared("status", "Connecting"),
		});
		let actions = Rc::downgrade(&link);
		host.action("select_guild", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			if let Some(guild_id) = snowflake(&argument) {
				link.select_guild(guild_id);
			}
		});
		link
	}

	async fn drain(self: Rc<Self>) {
		while let Some(batch) = self.inbox.next().await {
			let mut changes = Changes::default();
			for item in batch {
				changes.merge(item);
			}
			self.apply(&changes);
		}
	}

	fn apply(&self, changes: &Changes) {
		let Some(state) = self.state() else {
			return;
		};
		let state = state.read();
		let refresh = self.sidebar.borrow_mut().apply(&state, changes);
		self.runtime.batch(|| {
			if changes.session || changes.current_user {
				self.status.set(status_text(&state).as_str().into());
			}
			if refresh.guilds {
				self.publish_guilds();
			}
			if refresh.channels || refresh.selected || refresh.guilds {
				self.publish_selection(&state);
			}
		});
	}

	fn select_guild(&self, guild_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let command = {
			let state = shared.read();
			let command = self.sidebar.borrow_mut().select(&state, guild_id);
			self.runtime.batch(|| self.publish_selection(&state));
			command
		};
		if let Some(command) = command {
			self.send(command);
		}
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

	fn shutdown(&self) {
		self.inbox.close();
		if let Some(session) = self.session.borrow_mut().take() {
			session.shutdown();
		}
	}
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

fn record<const N: usize>(mut fields: [(&str, Value); N]) -> Value {
	fields.sort_by(|left, right| left.0.cmp(right.0));
	Value::Map(
		fields
			.into_iter()
			.map(|(key, value)| (Rc::from(key), value))
			.collect(),
	)
}
