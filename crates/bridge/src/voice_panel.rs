use protocol::Snowflake;
use state::{State, VoiceStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePanel {
	pub channel_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub status: &'static str,
	pub connected: bool,
	pub place: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VoiceControls {
	pub muted: bool,
	pub deafened: bool,
	pub server_muted: bool,
	pub server_deafened: bool,
	pub camera: bool,
	pub live: bool,
}

fn status_label(status: &VoiceStatus) -> Option<&'static str> {
	match status {
		VoiceStatus::Connecting => Some("Connecting"),
		VoiceStatus::Connected => Some("Voice Connected"),
		VoiceStatus::Reconnecting => Some("Reconnecting"),
		VoiceStatus::Disconnected { .. } => None,
	}
}

pub fn voice_panel(state: &State) -> Option<VoicePanel> {
	let session = state.voice()?;
	let status = status_label(&session.status)?;
	let channel = state
		.channel(session.channel_id)
		.and_then(|channel| channel.name.clone())
		.unwrap_or_default();
	let guild = session
		.guild_id
		.and_then(|guild_id| state.guild(guild_id))
		.map(|guild| guild.name.clone())
		.unwrap_or_default();
	let place = if guild.is_empty() {
		channel
	} else {
		format!("{channel} / {guild}")
	};
	Some(VoicePanel {
		channel_id: session.channel_id,
		guild_id: session.guild_id,
		status,
		connected: session.status == VoiceStatus::Connected,
		place,
	})
}

pub fn voice_controls(state: &State) -> VoiceControls {
	let chosen = state.voice_self();
	let own = state.own_voice_state();
	VoiceControls {
		muted: chosen.self_mute || chosen.self_deaf,
		deafened: chosen.self_deaf,
		server_muted: own.is_some_and(|own| own.mute),
		server_deafened: own.is_some_and(|own| own.deaf),
		camera: state.voice().is_some_and(|session| session.camera_on),
		live: state
			.voice()
			.is_some_and(|session| session.screen_share.is_some()),
	}
}
