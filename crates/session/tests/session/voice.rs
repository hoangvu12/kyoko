use std::cell::RefCell;
use std::time::{Duration, Instant};

use mockcord::{GatewayFrame, VOICE_TOKEN, VoiceFrame, VoiceRecording};
use protocol::{ChannelType, Opcode, Snowflake};
use serde_json::{Value, json};
use session::{Command, Trust, VoiceSetup};
use state::{State, VoiceStatus};
use voice::{AudioIo, FRAME_SAMPLES, SILENCE_FRAME};

use crate::harness::{Options, Running};

const JOIN_TIMEOUT: Duration = Duration::from_millis(400);
const VOICE_IDENTIFY: i64 = 0;
const SETTLE: Duration = Duration::from_millis(150);
const QUIET_WINDOW: Duration = Duration::from_millis(600);
const TONE_FRAMES: usize = 60;
const HELD_PACKETS: usize = 20;
const TONE_HZ: f32 = 1_000.0;
const TONE_AMPLITUDE: f32 = 0.25;
const SAMPLE_RATE: f32 = 48_000.0;
const OP_4_KEYS: [&str; 8] = [
	"guild_id",
	"channel_id",
	"self_mute",
	"self_deaf",
	"self_video",
	"preferred_region",
	"preferred_regions",
	"flags",
];
const ALLOW_VOICE_RECORDING: u64 = 2;

struct Voice {
	running: Running,
	guild_id: Snowflake,
	lounge: Snowflake,
	gaming: Snowflake,
}

async fn start(name: &'static str) -> Voice {
	let running = Running::start_with(Options::new(name)).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	let (lounge, gaming) = {
		let state = running.session().state().read();
		let guild = state.guild(guild_id).expect("the guild is loaded");
		let named = |name: &str| {
			guild
				.channels
				.values()
				.find(|channel| {
					channel.kind == ChannelType::GuildVoice && channel.name.as_deref() == Some(name)
				})
				.map(|channel| channel.id)
				.expect("a voice channel")
		};
		(named("Lounge"), named("Gaming"))
	};
	running.session().set_voice_setup(VoiceSetup {
		audio: AudioIo::Manual,
		trust: Trust::only(
			running
				.voice()
				.root_pem()
				.expect("mockcord's voice server runs on tls"),
		),
		join_timeout: JOIN_TIMEOUT,
	});
	Voice {
		running,
		guild_id,
		lounge,
		gaming,
	}
}

impl Voice {
	fn join(&self, channel_id: Snowflake) -> Instant {
		self.command(Command::JoinVoice {
			guild_id: self.guild_id,
			channel_id,
		})
	}

	fn command(&self, command: Command) -> Instant {
		let sent = Instant::now();
		self.running.session().send(command);
		sent
	}

	async fn connected_to(&self, channel_id: Snowflake) {
		self.running
			.until("the voice connection to come up", |state| {
				state.voice().is_some_and(|voice| {
					voice.channel_id == channel_id && voice.status == VoiceStatus::Connected
				})
			})
			.await;
	}

	async fn self_flags_become(&self, what: &str, flags: (bool, bool)) {
		self.running
			.until(what, |state| self_flags(state) == Some(flags))
			.await;
	}

	async fn voice_until(&self, what: &str, done: impl Fn(&VoiceRecording) -> bool) -> VoiceRecording {
		let control = self.running.voice();
		self.running.eventually(what, || done(&control.recording())).await;
		control.recording()
	}

	async fn op_4(&self, since: Instant, what: &str, wanted: impl Fn(&GatewayFrame) -> bool) -> GatewayFrame {
		self.running
			.until_wire(what, |recording| {
				recording
					.frames_with(Opcode::VoiceStateUpdate)
					.filter(|frame| frame.at > since && wanted(frame))
					.last()
					.cloned()
			})
			.await
	}

	fn ssrc(&self) -> u32 {
		self.running
			.session()
			.voice_connection()
			.and_then(|connection| connection.ssrc())
			.expect("a running voice connection")
	}

	fn feed_tone(&self) {
		let connection = self
			.running
			.session()
			.voice_connection()
			.expect("a running voice connection");
		for index in 0..TONE_FRAMES {
			connection.feed_input(&tone_frame(index));
		}
	}

	async fn stays_silent(&self, ssrc: u32, why: &str) {
		tokio::time::sleep(SETTLE).await;
		let quiet_from = Instant::now();
		self.feed_tone();
		tokio::time::sleep(QUIET_WINDOW).await;
		assert_eq!(
			spoken_after(&self.running.voice().recording(), ssrc, quiet_from),
			0,
			"{why}"
		);
	}

	async fn speaks(&self, ssrc: u32, what: &str) {
		let from = Instant::now();
		self.feed_tone();
		self.voice_until(what, |recording| spoken_after(recording, ssrc, from) > 0)
			.await;
	}

	fn own_voice_state(&self, mute: bool, deaf: bool) {
		let (me, session_id) = {
			let state = self.running.session().state().read();
			(
				state.current_user_id().expect("signed in"),
				state.session().session_id.clone(),
			)
		};
		self.running.mock().dispatch(
			"VOICE_STATE_UPDATE",
			&json!({
				"guild_id": self.guild_id.to_string(),
				"channel_id": self.lounge.to_string(),
				"user_id": me.to_string(),
				"session_id": session_id,
				"deaf": deaf,
				"mute": mute,
				"self_deaf": false,
				"self_mute": false,
				"self_video": false,
				"suppress": false,
				"request_to_speak_timestamp": null,
			}),
		);
	}

	async fn stop(self) {
		self.running.stop().await;
	}
}

fn tone_frame(index: usize) -> Vec<i16> {
	(0..FRAME_SAMPLES)
		.map(|sample| {
			let at = (index * FRAME_SAMPLES + sample) as f32 / SAMPLE_RATE;
			((at * TONE_HZ * std::f32::consts::TAU).sin() * TONE_AMPLITUDE * f32::from(i16::MAX)) as i16
		})
		.collect()
}

fn for_channel(frame: &GatewayFrame, channel_id: Option<Snowflake>) -> bool {
	frame.data["channel_id"] == channel_id.map_or(Value::Null, |id| json!(id.to_string()))
}

fn identifies(recording: &VoiceRecording) -> Vec<&VoiceFrame> {
	recording.frames_with(VOICE_IDENTIFY).collect()
}

fn spoken_after(recording: &VoiceRecording, ssrc: u32, after: Instant) -> usize {
	recording
		.packets_from(ssrc)
		.filter(|packet| packet.at > after && packet.payload != SILENCE_FRAME)
		.count()
}

fn self_flags(state: &State) -> Option<(bool, bool)> {
	state.voice().map(|voice| (voice.self_mute, voice.self_deaf))
}

#[tokio::test(flavor = "multi_thread")]
async fn joining_moving_and_leaving_a_guild_voice_channel_speak_op_4_and_reconnect() {
	let voice = start("voice-join-move-leave").await;
	let sent = voice.join(voice.lounge);
	let join = voice
		.op_4(sent, "op 4 for the Lounge", |frame| {
			for_channel(frame, Some(voice.lounge))
		})
		.await;
	assert_eq!(join.data["guild_id"], json!(voice.guild_id.to_string()));
	assert_eq!(join.keys(&[]), OP_4_KEYS);
	assert_eq!(join.data["self_mute"], json!(false));
	assert_eq!(join.data["self_deaf"], json!(false));
	assert_eq!(join.data["self_video"], json!(false));
	assert_eq!(join.data["preferred_region"], json!("mock-east"));
	assert_eq!(
		join.data["preferred_regions"],
		json!(["mock-east", "mock-central", "mock-west"])
	);
	assert_eq!(join.data["flags"], json!(ALLOW_VOICE_RECORDING));
	voice.connected_to(voice.lounge).await;
	let lounge_token = format!("{VOICE_TOKEN}-{}", voice.lounge);
	let recording = voice
		.voice_until("the voice identify", |recording| {
			!identifies(recording).is_empty()
		})
		.await;
	let identify = identifies(&recording)[0];
	assert_eq!(identify.data["server_id"], json!(voice.guild_id.to_string()));
	assert_eq!(identify.data["channel_id"], json!(voice.lounge.to_string()));
	assert_eq!(identify.data["token"], json!(lounge_token));
	let session_id = voice
		.running
		.session()
		.state()
		.read()
		.session()
		.session_id
		.clone();
	assert_eq!(identify.data["session_id"], json!(session_id));
	assert!(voice.running.session().voice_connection().is_some());

	let sent = voice.join(voice.gaming);
	voice
		.op_4(sent, "op 4 for Gaming", |frame| {
			for_channel(frame, Some(voice.gaming))
		})
		.await;
	voice.connected_to(voice.gaming).await;
	let gaming_token = format!("{VOICE_TOKEN}-{}", voice.gaming);
	let recording = voice
		.voice_until("a second voice identify with the new token", |recording| {
			identifies(recording)
				.iter()
				.any(|frame| frame.data["token"] == json!(gaming_token))
		})
		.await;
	let moved = identifies(&recording)
		.into_iter()
		.find(|frame| frame.data["token"] == json!(gaming_token))
		.expect("the move identifies again");
	assert_ne!(
		moved.connection, identify.connection,
		"the move opens a new voice socket"
	);
	assert_eq!(moved.data["channel_id"], json!(voice.gaming.to_string()));

	let sent = voice.command(Command::LeaveVoice);
	let leave = voice
		.op_4(sent, "op 4 leaving voice", |frame| for_channel(frame, None))
		.await;
	assert_eq!(leave.data["guild_id"], json!(voice.guild_id.to_string()));
	assert_eq!(leave.data["channel_id"], Value::Null);
	voice
		.running
		.until("voice to be left", |state| state.voice().is_none())
		.await;
	assert!(voice.running.session().voice_connection().is_none());
	let every_op_4: Vec<GatewayFrame> = voice
		.running
		.mock()
		.inspect(|recording| recording.frames_with(Opcode::VoiceStateUpdate).cloned().collect());
	assert!(
		every_op_4
			.iter()
			.all(|frame| frame.data["guild_id"] == json!(voice.guild_id.to_string())),
		"op 4 never goes out as a DM call"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mute_stops_sending_and_deafen_implies_mute_and_silences_playback() {
	let voice = start("voice-mute-deafen").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	voice.speaks(ssrc, "speech to reach the voice server").await;

	let sent = voice.command(Command::SetSelfMute(true));
	let muted = voice
		.op_4(sent, "op 4 with self_mute", |frame| {
			frame.data["self_mute"] == json!(true)
		})
		.await;
	assert_eq!(muted.data["channel_id"], json!(voice.lounge.to_string()));
	assert_eq!(muted.data["self_deaf"], json!(false));
	voice
		.self_flags_become("the session to show muted", (true, false))
		.await;
	voice.stays_silent(ssrc, "nothing is sent while muted").await;

	voice.command(Command::SetSelfMute(false));
	voice
		.self_flags_become("the session to show unmuted", (false, false))
		.await;
	voice.speaks(ssrc, "speech to resume after unmuting").await;

	let sent = voice.command(Command::SetSelfDeaf(true));
	let deafened = voice
		.op_4(sent, "op 4 with self_deaf", |frame| {
			frame.data["self_deaf"] == json!(true)
		})
		.await;
	assert_eq!(
		deafened.data["self_mute"],
		json!(true),
		"deafen implies mute on the wire"
	);
	voice
		.self_flags_become("the session to show deafened and muted", (true, true))
		.await;
	voice.stays_silent(ssrc, "nothing is sent while deafened").await;

	let sent = voice.command(Command::SetSelfMute(false));
	let undeafened = voice
		.op_4(sent, "op 4 after unmuting a deafened user", |frame| {
			frame.data["self_deaf"] == json!(false) && frame.data["self_mute"] == json!(false)
		})
		.await;
	assert_eq!(undeafened.data["channel_id"], json!(voice.lounge.to_string()));
	voice
		.self_flags_become("unmuting to undeafen too", (false, false))
		.await;

	let control = voice.running.voice();
	control.hold_echo();
	let held_from = Instant::now();
	voice.feed_tone();
	voice
		.voice_until("speech to be sent while the echo is held", |recording| {
			spoken_after(recording, ssrc, held_from) >= HELD_PACKETS
		})
		.await;
	voice.command(Command::SetSelfDeaf(true));
	voice
		.self_flags_become("the session to show deafened again", (true, true))
		.await;
	let connection = voice
		.running
		.session()
		.voice_connection()
		.expect("still connected");
	tokio::time::sleep(SETTLE).await;
	let _ = connection.take_output();
	control.release_echo();
	let heard = RefCell::new(Vec::new());
	voice
		.running
		.eventually("the held echo to reach the mixer", || {
			let mut heard = heard.borrow_mut();
			heard.extend(connection.take_output());
			heard.iter().any(|frame| !frame.sources.is_empty())
		})
		.await;
	assert!(
		heard
			.into_inner()
			.iter()
			.filter(|frame| !frame.sources.is_empty())
			.all(|frame| frame.pcm.iter().all(|sample| *sample == 0)),
		"a deafened user hears nothing"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn server_mute_and_deafen_show_and_stop_sending_until_lifted() {
	let voice = start("voice-server-mute").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let ssrc = voice.ssrc();
	voice.own_voice_state(true, true);
	voice
		.running
		.until("the server mute and deafen to show", |state| {
			state.own_voice_state().is_some_and(|own| own.mute && own.deaf)
		})
		.await;
	assert_eq!(
		voice
			.running
			.session()
			.state()
			.read()
			.voice()
			.map(|voice| voice.status.clone()),
		Some(VoiceStatus::Connected)
	);
	voice
		.stays_silent(ssrc, "a server-muted user sends nothing")
		.await;

	voice.own_voice_state(false, false);
	voice
		.running
		.until("the server mute and deafen to lift", |state| {
			state.own_voice_state().is_some_and(|own| !own.mute && !own.deaf)
		})
		.await;
	voice
		.speaks(ssrc, "speech to resume once the server lifts it")
		.await;
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_channel_times_out_with_a_toast_and_leaves_voice() {
	let voice = start("voice-full-channel").await;
	voice.running.mock().fill_voice_channel(voice.gaming.0);
	let sent = voice.join(voice.gaming);
	voice
		.op_4(sent, "op 4 for the full channel", |frame| {
			for_channel(frame, Some(voice.gaming))
		})
		.await;
	voice
		.running
		.until("the join to time out", |state| {
			state.voice().is_none()
				&& state
					.last_rejection()
					.is_some_and(|rejection| rejection.reason.contains("full"))
		})
		.await;
	voice
		.op_4(sent, "op 4 giving up on the channel", |frame| {
			for_channel(frame, None)
		})
		.await;
	assert!(
		voice.running.voice().recording().frames.is_empty(),
		"no voice socket was opened"
	);
	voice.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_gateway_session_sends_op_4_again_and_reconnects_voice() {
	let voice = start("voice-gateway-reconnect").await;
	voice.join(voice.lounge);
	voice.connected_to(voice.lounge).await;
	let invalidated = Instant::now();
	voice.running.mock().invalidate_session(false);
	let identified = voice
		.running
		.until_wire("the second IDENTIFY", |recording| {
			recording
				.frames_with(Opcode::Identify)
				.find(|frame| frame.at > invalidated)
				.map(|frame| frame.at)
		})
		.await;
	let again = voice
		.op_4(identified, "op 4 after the new IDENTIFY", |frame| {
			for_channel(frame, Some(voice.lounge))
		})
		.await;
	assert_eq!(again.data["guild_id"], json!(voice.guild_id.to_string()));
	voice
		.voice_until("a second voice identify", |recording| {
			identifies(recording).len() >= 2
		})
		.await;
	voice.connected_to(voice.lounge).await;
	voice.stop().await;
}
