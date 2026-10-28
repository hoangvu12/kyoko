use std::time::{Duration, Instant};

use bridge::{Pace, SPEAKING_PACE, SpeakingPace, VoiceVideo, voice_call, voice_speaking};
use protocol::Snowflake;
use state::{Event, LocalEvent, State, UserAudio, VideoKind, VoiceStatus};

const CHANNEL: Snowflake = Snowflake(800_000_000_000_000_001);
const GRACE: Snowflake = Snowflake(800_000_000_000_000_002);
const LINUS: Snowflake = Snowflake(800_000_000_000_000_003);

fn local(state: &mut State, event: LocalEvent) {
	state.apply(&Event::Local(event));
}

fn in_call() -> State {
	let mut state = State::new();
	local(
		&mut state,
		LocalEvent::VoiceConnection {
			status: VoiceStatus::Connected,
			channel_id: CHANNEL,
			guild_id: None,
		},
	);
	state
}

fn tile_ids(state: &State) -> Vec<Snowflake> {
	voice_call(state)
		.map(|call| call.tiles.iter().map(|tile| tile.user_id).collect())
		.unwrap_or_default()
}

#[test]
fn peers_from_op_11_and_13_come_and_go_as_tiles_with_their_volume_and_speaking() {
	let mut state = in_call();
	assert!(tile_ids(&state).is_empty());
	for user_id in [GRACE, LINUS] {
		local(
			&mut state,
			LocalEvent::VoicePeer {
				user_id,
				present: true,
			},
		);
	}
	assert_eq!(tile_ids(&state).len(), 2);
	local(
		&mut state,
		LocalEvent::UserAudio {
			user_id: GRACE,
			audio: UserAudio {
				volume: 150,
				muted: true,
			},
		},
	);
	local(
		&mut state,
		LocalEvent::Speaking {
			user_id: GRACE,
			speaking: true,
		},
	);
	let call = voice_call(&state).expect("a call");
	let grace = call
		.tiles
		.iter()
		.find(|tile| tile.user_id == GRACE)
		.expect("grace's tile");
	assert_eq!((grace.volume, grace.local_muted), (150, true));
	assert_eq!(voice_speaking(&state), vec![GRACE]);

	local(
		&mut state,
		LocalEvent::VoicePeer {
			user_id: GRACE,
			present: false,
		},
	);
	assert_eq!(tile_ids(&state), vec![LINUS], "op 13 removes the tile");
	assert!(
		voice_speaking(&state).is_empty(),
		"someone who left stops speaking"
	);
	local(
		&mut state,
		LocalEvent::VoiceConnection {
			status: VoiceStatus::Disconnected { reason: None },
			channel_id: CHANNEL,
			guild_id: None,
		},
	);
	assert!(voice_call(&state).is_none(), "leaving ends the call view");
}

#[test]
fn speaking_is_published_on_change_at_most_every_100_ms() {
	let start = Instant::now();
	let mut pace = SpeakingPace::default();
	assert_eq!(pace.offer(vec![GRACE], start), Pace::Publish(vec![GRACE]));
	assert_eq!(pace.offer(vec![GRACE], start), Pace::Same);
	let soon = start + Duration::from_millis(30);
	assert_eq!(
		pace.offer(Vec::new(), soon),
		Pace::Wait(start + SPEAKING_PACE),
		"a change within 100 ms waits"
	);
	assert_eq!(pace.shown(), [GRACE]);
	let later = start + SPEAKING_PACE;
	assert_eq!(pace.offer(Vec::new(), later), Pace::Publish(Vec::new()));
	assert_eq!(pace.offer(Vec::new(), later), Pace::Same);
}

fn tile_video(state: &State, user_id: Snowflake) -> Option<VoiceVideo> {
	voice_call(state)?
		.tiles
		.into_iter()
		.find(|tile| tile.user_id == user_id)?
		.video
}

#[test]
fn a_members_op_12_camera_makes_their_tile_a_camera_tile_until_it_stops() {
	let mut state = in_call();
	local(
		&mut state,
		LocalEvent::VoicePeer {
			user_id: GRACE,
			present: true,
		},
	);
	assert_eq!(tile_video(&state, GRACE), None, "an audio tile");
	let camera = |active: bool| LocalEvent::RemoteVideo {
		user_id: GRACE,
		kind: VideoKind::Camera,
		active,
	};
	local(&mut state, camera(true));
	assert_eq!(tile_video(&state, GRACE), Some(VoiceVideo::Camera));
	local(&mut state, camera(false));
	assert_eq!(
		tile_video(&state, GRACE),
		None,
		"the camera tile ends with the video"
	);
}
