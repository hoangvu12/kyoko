use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use mockcord::Recording;
use parking_lot::Mutex;
use protocol::{ProtoStatusSettings, Snowflake, encode_status_settings};
use serde_json::json;
use session::{Command, EmojiRankRequest, SessionHooks};
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
