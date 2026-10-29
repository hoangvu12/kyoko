use std::cell::RefCell;
use std::rc::Rc;

use bridge::{VoiceControls, VoicePanel};
use protocol::{ChannelType, Snowflake};
use session::Command;
use state::State;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, record, snowflake};

pub(super) struct VoicePanelView {
	panel: Shared,
	controls: Shared,
	shown: RefCell<Option<(Option<VoicePanel>, VoiceControls)>>,
}

impl VoicePanelView {
	pub(super) fn new(host: &ScriptHost) -> Self {
		Self {
			panel: host.shared("voice.panel", panel_value(None)),
			controls: host.shared("voice.controls", controls_value(VoiceControls::default())),
			shown: RefCell::new(None),
		}
	}

	pub(super) fn reset(&self) {
		self.shown.borrow_mut().take();
		self.panel.set(panel_value(None));
		self.controls.set(controls_value(VoiceControls::default()));
	}

	fn publish(&self, state: &State) {
		let panel = bridge::voice_panel(state);
		let controls = bridge::voice_controls(state);
		let next = (panel, controls);
		if self.shown.borrow().as_ref() == Some(&next) {
			return;
		}
		self.panel.set(panel_value(next.0.as_ref()));
		self.controls.set(controls_value(next.1));
		*self.shown.borrow_mut() = Some(next);
	}
}

fn panel_value(panel: Option<&VoicePanel>) -> Value {
	record([
		("open", panel.is_some().into()),
		(
			"channel_id",
			panel
				.map(|panel| panel.channel_id.to_string())
				.unwrap_or_default()
				.as_str()
				.into(),
		),
		("status", panel.map_or("", |panel| panel.status).into()),
		("connected", panel.is_some_and(|panel| panel.connected).into()),
		("place", panel.map_or("", |panel| panel.place.as_str()).into()),
	])
}

fn controls_value(controls: VoiceControls) -> Value {
	record([
		("muted", controls.muted.into()),
		("deafened", controls.deafened.into()),
		("server_muted", controls.server_muted.into()),
		("server_deafened", controls.server_deafened.into()),
		("camera", controls.camera.into()),
		("live", controls.live.into()),
	])
}

impl Link {
	pub(super) fn install_voice(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.join", move |argument| {
			if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.join_voice(channel_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.leave", move |_| {
			if let Some(link) = actions.upgrade() {
				link.send(Command::LeaveVoice);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.toggle_mute", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(controls) = link.voice_controls()
			{
				link.send(Command::SetSelfMute(!controls.muted));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.toggle_deafen", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(controls) = link.voice_controls()
			{
				link.send(Command::SetSelfDeaf(!controls.deafened));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.toggle_camera", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(controls) = link.voice_controls()
			{
				link.send(Command::SetSelfVideo(!controls.camera));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.open_call", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_call();
			}
		});
	}

	pub(super) fn voice_apply(&self, state: &State) {
		self.voice.publish(state);
		self.publish_call(state);
		self.publish_dm_call(state);
	}

	pub(super) fn shows_live(&self) -> bool {
		self.voice
			.shown
			.borrow()
			.as_ref()
			.is_some_and(|(_, controls)| controls.live)
	}

	fn voice_controls(&self) -> Option<VoiceControls> {
		self.state().map(|shared| bridge::voice_controls(&shared.read()))
	}

	pub(super) fn voice_channel(&self) -> Option<Snowflake> {
		self.state()
			.and_then(|shared| shared.read().voice().map(|voice| voice.channel_id))
	}

	pub(super) async fn drive_voice_panel(&self) {
		let target = self.state().and_then(|shared| {
			let state = shared.read();
			let guild = state.guild(self.sidebar.sidebar().selected()?)?;
			guild
				.channels
				.values()
				.filter(|channel| channel.kind == ChannelType::GuildVoice)
				.min_by_key(|channel| (channel.position.unwrap_or(0), channel.id))
				.map(|channel| channel.id)
		});
		let Some(channel_id) = target else {
			tracing::warn!("no voice channel to join");
			return;
		};
		self.join_voice(channel_id);
		self.until_capture(|link| {
			link.voice
				.shown
				.borrow()
				.as_ref()
				.is_some_and(|(panel, _)| panel.as_ref().is_some_and(|panel| panel.connected))
		})
		.await;
		tracing::info!("joined voice for the capture");
	}

	fn join_voice(&self, channel_id: Snowflake) {
		if self.voice_channel() == Some(channel_id) {
			self.open_call();
			return;
		}
		let guild_id = self.sidebar.sidebar().selected();
		if let Some(guild_id) = guild_id {
			self.send(Command::JoinVoice { guild_id, channel_id });
		}
	}
}
