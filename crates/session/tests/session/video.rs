use std::time::Duration;

use mockcord::{CAMERA_HEIGHT, CAMERA_WIDTH, ScriptedCamera, VoiceControl};
use protocol::{ChannelType, Snowflake};
use serde_json::json;
use session::{CameraIo, Command, Trust, VideoUpdate, VideoView, VoiceSetup};
use state::{State, VoiceStatus};
use voice::AudioIo;

use crate::harness::{Options, Running};

const JOIN_TIMEOUT: Duration = Duration::from_secs(5);
const CAMERA_FRAME: Duration = Duration::from_millis(66);
const MEDIA_SINK_WANTS: i64 = 15;
const DAVE_MARKER: [u8; 2] = [0xfa, 0xfa];
const MEMBER: u64 = 222_801_430_442_967_040;
const MEMBER_AUDIO_SSRC: u32 = 9_000;
const MEMBER_VIDEO_SSRC: u32 = 9_002;
const FOCUSED: u64 = 555_101_220_330_440_550;
const THUMBNAIL: u64 = 555_101_220_330_440_660;
const OFF_SCREEN: u64 = 555_101_220_330_440_770;
const FOCUSED_SSRC: u32 = 9_102;
const THUMBNAIL_SSRC: u32 = 9_202;
const OFF_SCREEN_SSRC: u32 = 9_302;
const FIRST_FRAMES: usize = 10;
const THUMBNAIL_WIDTH: u32 = 320;
const THUMBNAIL_HEIGHT: u32 = 180;
const FOCUSED_WIDTH: u32 = 1_280;
const FOCUSED_HEIGHT: u32 = 720;

struct Call {
	running: Running,
	lounge: Snowflake,
}

async fn join_lounge(name: &'static str, dave: bool) -> Call {
	let running = Running::start_with(Options::new(name)).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	let lounge = {
		let state = running.session().state().read();
		let guild = state.guild(guild_id).expect("the guild is loaded");
		guild
			.channels
			.values()
			.find(|channel| {
				channel.kind == ChannelType::GuildVoice && channel.name.as_deref() == Some("Lounge")
			})
			.map(|channel| channel.id)
			.expect("the lounge")
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
		camera: CameraIo::Pattern,
		..VoiceSetup::default()
	});
	let control = running.voice();
	if dave {
		control.set_dave_version(1);
		control.add_member(lounge.get(), MEMBER);
	}
	running.session().send(Command::JoinVoice {
		guild_id,
		channel_id: lounge,
	});
	running
		.until("the call to connect", |state| {
			state
				.voice()
				.is_some_and(|voice| voice.channel_id == lounge && voice.status == VoiceStatus::Connected)
		})
		.await;
	Call { running, lounge }
}

fn has_video(state: &State, user_id: u64) -> bool {
	state
		.voice()
		.is_some_and(|voice| voice.remote_video.contains_key(&Snowflake(user_id)))
}

async fn play(camera: &mut ScriptedCamera, frames: usize) {
	for _ in 0..frames {
		camera.send_frame();
		tokio::time::sleep(CAMERA_FRAME).await;
	}
}

#[tokio::test(flavor = "multi_thread")]
async fn a_members_camera_decodes_through_dave_asks_for_a_keyframe_on_loss_and_op_13_ends_it() {
	let call = join_lounge("video-camera", true).await;
	let control: VoiceControl = call.running.voice();
	let session = call.running.session();
	call.running
		.eventually("DAVE to start with the member", || {
			control.dave_group(call.lounge.get()).len() == 2
				&& session
					.voice_connection()
					.is_some_and(|connection| connection.dave().enabled())
		})
		.await;
	control.video_as(MEMBER, MEMBER_AUDIO_SSRC, MEMBER_VIDEO_SSRC);
	call.running
		.until("op 12 to give the member a video tile", |state| {
			has_video(state, MEMBER)
		})
		.await;
	let member = Snowflake(MEMBER);
	let feed = session.video_feed();
	session.send(Command::SetVideoViews(vec![VideoView {
		user_id: member,
		focused: false,
		width: THUMBNAIL_WIDTH,
		height: THUMBNAIL_HEIGHT,
	}]));

	let mut camera = ScriptedCamera::new(control.clone(), call.lounge.get(), MEMBER, MEMBER_VIDEO_SSRC);
	let sealed = control
		.seal_video(call.lounge.get(), MEMBER, &mockcord::camera_frames()[0])
		.expect("the member seals its first frame");
	assert!(
		sealed.ends_with(&DAVE_MARKER),
		"the member's frames are DAVE frames"
	);
	play(&mut camera, FIRST_FRAMES).await;
	call.running
		.eventually("the member's frames to decode", || {
			feed.decoded(member) >= FIRST_FRAMES as u64 - 2
		})
		.await;
	assert_eq!(
		feed.source_size(member),
		Some((CAMERA_WIDTH, CAMERA_HEIGHT)),
		"frames decode at the camera's size"
	);
	let connection = session.voice_connection().expect("a running voice connection");
	assert!(
		connection.dave().decrypted() > 0,
		"video frames were opened with DAVE"
	);
	let shown = feed.drain();
	let picture = shown
		.iter()
		.find_map(|update| match update {
			VideoUpdate::Show { user_id, picture } if *user_id == member => Some(picture),
			_ => None,
		})
		.expect("a decoded frame reaches the feed as a picture");
	assert_eq!(
		(picture.width, picture.height),
		(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT),
		"pictures are scaled to the tile"
	);
	assert_eq!(
		picture.rgba.len(),
		(THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT * 4) as usize
	);
	assert_eq!(
		control.recording().picture_losses(MEMBER_VIDEO_SSRC),
		0,
		"a clean stream asks for no keyframe"
	);

	let before_loss = feed.decoded(member);
	camera.lose_next_packet();
	play(&mut camera, 2).await;
	call.running
		.eventually("a PLI for the member's stream after a lost packet", || {
			control.recording().picture_losses(MEMBER_VIDEO_SSRC) > 0
		})
		.await;
	let rtcp = control.recording().rtcp;
	let pli = rtcp
		.iter()
		.find(|rtcp| rtcp.is_picture_loss())
		.expect("the PLI was recorded");
	assert_eq!(pli.media_ssrc(), Some(MEMBER_VIDEO_SSRC));
	assert_eq!(
		Some(pli.sender_ssrc),
		connection.ssrc(),
		"the PLI comes from kyoko's own SSRC"
	);
	let stalled = feed.decoded(member);
	let cycle = camera.frames();
	play(&mut camera, cycle).await;
	call.running
		.eventually("decoding to resume at the next keyframe", || {
			feed.decoded(member) > stalled
		})
		.await;
	assert!(stalled >= before_loss);

	control.disconnect_as(MEMBER);
	call.running
		.until("op 13 to end the member's video tile", |state| {
			!has_video(state, MEMBER)
		})
		.await;
	call.running
		.eventually("the feed to clear the tile", || {
			feed.drain().contains(&VideoUpdate::Clear { user_id: member })
		})
		.await;
	assert_eq!(feed.decoded(member), 0);
	assert!(connection.video_users().is_empty());
	call.running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn wants_ask_100_for_a_focused_tile_50_for_a_thumbnail_and_0_off_screen() {
	let call = join_lounge("video-wants", false).await;
	let control = call.running.voice();
	for (user_id, ssrc) in [
		(FOCUSED, FOCUSED_SSRC),
		(THUMBNAIL, THUMBNAIL_SSRC),
		(OFF_SCREEN, OFF_SCREEN_SSRC),
	] {
		control.video_as(user_id, ssrc - 2, ssrc);
	}
	call.running
		.until("three members with video", |state| {
			[FOCUSED, THUMBNAIL, OFF_SCREEN]
				.into_iter()
				.all(|user_id| has_video(state, user_id))
		})
		.await;
	call.running.session().send(Command::SetVideoViews(vec![
		VideoView {
			user_id: Snowflake(FOCUSED),
			focused: true,
			width: FOCUSED_WIDTH,
			height: FOCUSED_HEIGHT,
		},
		VideoView {
			user_id: Snowflake(THUMBNAIL),
			focused: false,
			width: THUMBNAIL_WIDTH,
			height: THUMBNAIL_HEIGHT,
		},
	]));
	let wanted = json!({
		FOCUSED_SSRC.to_string(): 100,
		THUMBNAIL_SSRC.to_string(): 50,
		OFF_SCREEN_SSRC.to_string(): 0,
		"any": 100,
		"pixelCounts": {
			FOCUSED_SSRC.to_string(): FOCUSED_WIDTH * FOCUSED_HEIGHT,
			THUMBNAIL_SSRC.to_string(): THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT,
		},
	});
	call.running
		.eventually("op 15 with per-SSRC wants and pixel counts", || {
			control
				.recording()
				.frames_with(MEDIA_SINK_WANTS)
				.any(|frame| frame.data == wanted)
		})
		.await;

	call.running.session().send(Command::SetVideoViews(Vec::new()));
	let closed = json!({
		FOCUSED_SSRC.to_string(): 0,
		THUMBNAIL_SSRC.to_string(): 0,
		OFF_SCREEN_SSRC.to_string(): 0,
		"any": 100,
	});
	call.running
		.eventually("op 15 turning every stream off once nothing is on screen", || {
			control
				.recording()
				.frames_with(MEDIA_SINK_WANTS)
				.any(|frame| frame.data == closed)
		})
		.await;
	call.running.stop().await;
}
