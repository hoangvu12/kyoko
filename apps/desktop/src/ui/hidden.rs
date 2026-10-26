use std::rc::Rc;

use bridge::{HiddenChannel, Overwriter};
use chrono::Utc;
use plugins::Slot;
use protocol::{ChannelType, Snowflake};
use state::State;
use zgx::reactive::{Runtime, Signal};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, record};

pub(super) struct HiddenPane {
	shown: Signal<bool>,
	view: Shared,
}

impl HiddenPane {
	pub(super) fn new(host: &ScriptHost, runtime: &Runtime) -> Self {
		Self {
			shown: runtime.signal(false),
			view: host.shared("hidden.view", closed_view()),
		}
	}

	pub(super) fn shown(&self) -> Signal<bool> {
		self.shown.clone()
	}

	fn show(&self, info: &HiddenChannel) {
		self.view.set(view_value(info));
		if !self.shown.with_untracked(|shown| *shown) {
			self.shown.set(true);
		}
	}

	fn hide(&self) {
		if self.shown.with_untracked(|shown| *shown) {
			self.shown.set(false);
			self.view.set(closed_view());
		}
	}
}

impl Link {
	pub(crate) fn open_hidden_channel(&self, channel_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let info = self.sidebar.open_hidden(&shared.read(), channel_id);
		let Some(info) = info else {
			return;
		};
		if self.thread_open().is_some() {
			self.close_thread_pane(false);
		}
		self.leave_forum();
		let state = shared.read();
		self.timeline.borrow_mut().close();
		self.window.runtime.batch(|| {
			self.hidden.show(&info);
			self.publish_timeline(&state);
			self.channel_name.set(info.name.as_str().into());
			self.channel_topic
				.set(info.topic.as_deref().unwrap_or_default().into());
		});
	}

	pub(super) async fn drive_hidden_channel(&self) {
		let target = self.state().and_then(|shared| {
			let state = shared.read();
			state.guilds().find_map(|guild| {
				guild
					.channels
					.values()
					.find(|channel| channel.kind == ChannelType::GuildText && state.is_hidden(channel.id))
					.map(|channel| (guild.id, channel.id))
			})
		});
		let Some((guild_id, channel_id)) = target else {
			tracing::warn!("no hidden channel to show");
			return;
		};
		self.select_guild(guild_id);
		self.until_capture(|link| {
			link.sidebar
				.sidebar()
				.channel(channel_id)
				.is_some_and(|channel| channel.opens_info())
		})
		.await;
		self.open_hidden_channel(channel_id);
		self.until_capture(|link| link.rail_icons_shown()).await;
	}

	pub(super) fn sync_hidden(&self, state: &State, opened: bool) {
		if opened {
			self.sidebar.close_hidden();
		}
		let info = self.sidebar.sidebar().hidden_view(state);
		match info {
			Some(info) => self.hidden.show(&info),
			None => {
				self.sidebar.close_hidden();
				self.hidden.hide();
			}
		}
	}

	pub(super) fn apply_plugin_views(&self) {
		let host = self.plugins.host();
		let (hidden, notes) = (host.shows(Slot::HiddenChannels), host.shows(Slot::ModeratorNotes));
		self.members.set_moderator_notes(notes);
		let shared = self.state();
		let state = shared.as_ref().map(|shared| shared.read());
		if self.sidebar.set_show_hidden(state.as_deref(), hidden)
			&& let Some(state) = state.as_deref()
		{
			self.window.runtime.batch(|| self.publish_selection(state));
		}
	}
}

fn closed_view() -> Value {
	record([
		("open", false.into()),
		("id", "".into()),
		("name", "".into()),
		("kind", "".into()),
		("icon", "hash".into()),
		("topic", "".into()),
		("activity", "".into()),
		("created", "".into()),
		("nsfw", false.into()),
		("slowmode", "".into()),
		("allowed", Value::List(Rc::new([]))),
		("denied", Value::List(Rc::new([]))),
	])
}

fn view_value(info: &HiddenChannel) -> Value {
	let now = Utc::now();
	record([
		("open", true.into()),
		("id", info.id.to_string().as_str().into()),
		("name", info.name.as_str().into()),
		("kind", info.kind.into()),
		("icon", info.icon.into()),
		("topic", info.topic.as_deref().unwrap_or_default().into()),
		("activity", info.activity_text(now).as_str().into()),
		("created", info.created_text(now).as_str().into()),
		("nsfw", info.nsfw.into()),
		(
			"slowmode",
			info.slowmode_text().unwrap_or_default().as_str().into(),
		),
		("allowed", overwriters(&info.allowed)),
		("denied", overwriters(&info.denied)),
	])
}

fn overwriters(entries: &[Overwriter]) -> Value {
	Value::List(
		entries
			.iter()
			.map(|entry| {
				let color = entry
					.color
					.map(|color| format!("#{:06x}", color & 0xff_ffff))
					.unwrap_or_default();
				record([
					("key", entry.id.to_string().as_str().into()),
					("name", entry.name.as_str().into()),
					("member", entry.member.into()),
					("color", color.as_str().into()),
				])
			})
			.collect(),
	)
}
