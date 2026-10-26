use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use mockcord::Recording;
use parking_lot::Mutex;
use protocol::{
	FavoriteGif, ProtoStatusSettings, Snowflake, decode_favorite_emojis, decode_favorite_gifs,
	encode_status_settings,
};
use serde_json::json;
use session::{Command, EmojiRankRequest, RestTuning, SessionHooks};
use state::{EmojiCandidate, State};

use crate::harness::{Options, Running};

const FRECENCY: &str = "/users/@me/settings-proto/2";
const QUIET: Duration = Duration::from_millis(400);

fn frecency_reads(recording: &Recording) -> usize {
	recording.requests_to("GET", FRECENCY).count()
}

fn unicode(name: &str, glyph: &str) -> EmojiCandidate {
	EmojiCandidate {
		key: name.to_owned(),
		name: name.to_owned(),
		id: None,
		animated: false,
		guild_id: None,
		unicode: Some(glyph.to_owned()),
	}
}

fn smiles() -> Vec<EmojiCandidate> {
	vec![
		unicode("smile", "😄"),
		unicode("smiley", "😃"),
		unicode("smirk", "😏"),
		unicode("heart", "❤️"),
	]
}

fn ask(running: &Running, query: &str, candidates: Vec<EmojiCandidate>) {
	running.session().send(Command::RankEmoji {
		guild_id: None,
		query: query.to_owned(),
		candidates,
	});
}

fn wave_id(running: &Running) -> Snowflake {
	let favorite = running.mock().favorite_emojis()[0].clone();
	Snowflake(
		favorite
			.parse()
			.expect("the mock's first favourite is a custom emoji"),
	)
}

#[derive(Default)]
struct Ranker {
	asked: Mutex<Vec<EmojiRankRequest>>,
}

impl SessionHooks for Ranker {
	fn ranks_emoji(&self) -> bool {
		true
	}

	fn rank_emoji(&self, request: &EmojiRankRequest) -> Result<Option<Vec<String>>> {
		self.asked.lock().push(request.clone());
		if request.query == "boom" {
			bail!("the ranking failed");
		}
		let mut favorites: Vec<_> = request
			.emojis
			.iter()
			.filter_map(|ranked| Some((ranked.favorite?, ranked.emoji.key.clone())))
			.collect();
		favorites.sort();
		Ok(Some(favorites.into_iter().map(|(_, key)| key).collect()))
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn favourite_emoji_are_read_once_from_the_frecency_proto_when_emoji_are_first_suggested() {
	let running = Running::start_with(Options::new("favourites-read")).await;
	running.connected().await;
	tokio::time::sleep(QUIET).await;
	assert_eq!(
		frecency_reads(&running.mock().recording()),
		0,
		"the frecency proto is lazy, as in the web client"
	);
	ask(&running, "he", Vec::new());
	let expected = running.mock().favorite_emojis();
	running
		.until("the favourites", |state: &State| {
			state.favorites_loaded() && state.favorite_emojis() == expected.as_slice()
		})
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(state.favorite_position(Some(wave_id(&running)), None), Some(0));
		assert_eq!(state.favorite_position(None, Some("❤️")), Some(1));
		assert_eq!(state.favorite_position(None, Some("❤")), Some(1));
		assert_eq!(state.favorite_position(None, Some("😄")), None);
	}
	ask(&running, "hea", Vec::new());
	ask(&running, "heart", Vec::new());
	tokio::time::sleep(QUIET).await;
	let recording = running.mock().recording();
	let reads: Vec<_> = recording.requests_to("GET", FRECENCY).collect();
	assert_eq!(reads.len(), 1, "read once per session");
	assert_eq!(reads[0].query, None);
	assert!(reads[0].body.is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn favourites_follow_frecency_updates_from_other_devices() {
	let running = Running::start_with(Options::new("favourites-update")).await;
	running.connected().await;
	ask(&running, "he", Vec::new());
	running
		.until("the favourites", |state: &State| state.favorites_loaded())
		.await;
	let before = running.session().state().read().favorites_generation();
	running.mock().update_favorite_emojis(&["smile", "heart"]);
	running
		.until("the updated favourites", |state: &State| {
			state.favorite_emojis() == ["smile".to_owned(), "heart".to_owned()]
		})
		.await;
	assert!(running.session().state().read().favorites_generation() > before);
	let status = ProtoStatusSettings {
		status: Some("idle".to_owned()),
		..ProtoStatusSettings::default()
	};
	running.mock().dispatch(
		"USER_SETTINGS_PROTO_UPDATE",
		&json!({ "settings": { "type": 1, "proto": encode_status_settings(&status) }, "partial": true }),
	);
	running.mock().dispatch(
		"USER_SETTINGS_PROTO_UPDATE",
		&json!({ "settings": { "type": 2, "proto": "" }, "partial": true }),
	);
	tokio::time::sleep(QUIET).await;
	assert_eq!(
		running.session().state().read().favorite_emojis(),
		["smile".to_owned(), "heart".to_owned()],
		"other settings and partial updates without favourites leave them"
	);
	running.stop().await;
}

fn cat_dance() -> FavoriteGif {
	FavoriteGif {
		url: "https://klipy.com/gifs/cat-dance".to_owned(),
		src: "https://static.klipy.com/cat-dance.mp4".to_owned(),
		width: 498,
		height: 280,
		video: true,
		order: 0,
	}
}

fn written_gifs(recording: &Recording) -> Vec<Vec<FavoriteGif>> {
	recording
		.requests_to("PATCH", FRECENCY)
		.map(|request| {
			let body: serde_json::Value = serde_json::from_slice(&request.body).expect("a JSON body");
			let encoded = body["settings"].as_str().expect("the settings").to_owned();
			assert_eq!(
				decode_favorite_emojis(&encoded),
				None,
				"only the GIF field is sent"
			);
			decode_favorite_gifs(&encoded).expect("the GIF favourites field")
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_favourited_gif_is_read_then_written_to_the_frecency_proto_and_unfavouriting_removes_it() {
	let running = Running::start_with(Options::new("gif-favourites").rest(RestTuning {
		settings_delay: Duration::from_millis(50),
		mutation_gap: Duration::ZERO,
		..RestTuning::default()
	}))
	.await;
	running.connected().await;
	running.session().send(Command::SetGifFavorite {
		gif: cat_dance(),
		favorite: true,
	});
	running
		.until("the favourite GIF", |state: &State| {
			state.is_favorite_gif(&cat_dance().url)
		})
		.await;
	running
		.eventually("the favourite stored by Discord", || {
			running.mock().favorite_gifs().len() == 1
		})
		.await;
	let stored = running.mock().favorite_gifs();
	assert_eq!(stored[0].src, cat_dance().src);
	assert!(stored[0].video);
	assert_eq!(stored[0].order, 1);
	assert_eq!(
		frecency_reads(&running.mock().recording()),
		1,
		"the favourites are read before the first write, so other favourites survive"
	);
	assert!(
		!running.mock().favorite_emojis().is_empty(),
		"the favourite emoji are left alone"
	);
	running.session().send(Command::SetGifFavorite {
		gif: cat_dance(),
		favorite: false,
	});
	running
		.eventually("the favourite removed at Discord", || {
			running.mock().favorite_gifs().is_empty()
		})
		.await;
	tokio::time::sleep(QUIET).await;
	let writes = written_gifs(&running.mock().recording());
	assert_eq!(writes.len(), 2, "one write per change: {writes:?}");
	assert_eq!(writes[0].len(), 1);
	assert!(writes[1].is_empty());
	assert!(running.session().state().read().favorite_gifs().is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emoji_ranking_waits_for_a_pause_in_typing_and_a_failing_ranking_keeps_kyokos_order() {
	let ranker = Arc::new(Ranker::default());
	let running = Running::start_with(Options::new("emoji-rank").hooks(ranker.clone())).await;
	running.connected().await;
	ask(&running, "he", Vec::new());
	running
		.until("the favourites", |state: &State| state.favorites_loaded())
		.await;
	for query in ["s", "sm", "smi", "smil"] {
		ask(&running, query, smiles());
	}
	running
		.until("the ranking", |state: &State| {
			state.emoji_ranking(None, "smil").is_some()
		})
		.await;
	assert_eq!(
		running.session().state().read().emoji_ranking(None, "smil"),
		Some(["heart".to_owned()].as_slice())
	);
	{
		let asked = ranker.asked.lock();
		assert_eq!(asked.len(), 1, "one call for a burst of keystrokes: {asked:?}");
		assert_eq!(asked[0].query, "smil");
		assert_eq!(asked[0].emojis.len(), 4);
		assert_eq!(asked[0].emojis[3].favorite, Some(1));
		assert_eq!(asked[0].emojis[0].favorite, None);
	}
	ask(&running, "boom", smiles());
	running
		.eventually("the failing ranking", || ranker.asked.lock().len() == 2)
		.await;
	tokio::time::sleep(QUIET).await;
	assert_eq!(running.session().state().read().emoji_ranking(None, "boom"), None);
	ask(&running, "smirk", smiles());
	running
		.until("a ranking after the failure", |state: &State| {
			state.emoji_ranking(None, "smirk").is_some()
		})
		.await;
	ask(&running, "smirk", Vec::new());
	tokio::time::sleep(QUIET).await;
	assert_eq!(ranker.asked.lock().len(), 3, "no candidates, no call");
	running.stop().await;
}
