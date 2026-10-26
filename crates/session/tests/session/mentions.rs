use std::time::Duration;

use mockcord::{Reply, RestRequest};
use protocol::Snowflake;
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const MENTIONS: &str = "/users/@me/mentions";
const ONE_MENTION: &str = "/users/@me/mentions/*";
const MUTATION_GAP: Duration = Duration::from_millis(200);
const SETTLE: Duration = Duration::from_millis(400);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: MUTATION_GAP,
		..RestTuning::default()
	}
}

fn everything() -> Command {
	Command::LoadRecentMentions {
		everyone: true,
		roles: true,
		guild_id: None,
	}
}

fn mention_ids(state: &State) -> Vec<Snowflake> {
	state
		.recent_mentions()
		.map(|messages| messages.iter().map(|message| message.id).collect())
		.unwrap_or_default()
}

fn fetches(running: &Running) -> Vec<RestRequest> {
	running
		.mock()
		.recording()
		.requests_to("GET", MENTIONS)
		.cloned()
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recent_mentions_load_once_when_asked_and_removals_are_paced() {
	let running = Running::start_with(Options::new("mentions-load").rest(tuning())).await;
	running.connected().await;
	tokio::time::sleep(SETTLE).await;
	assert!(
		fetches(&running).is_empty(),
		"nothing fetches recent mentions before the Mentions tab opens"
	);
	running.session().send(everything());
	running.session().send(everything());
	running
		.until("the recent mentions to load", |state| {
			state.recent_mentions().is_some()
		})
		.await;
	tokio::time::sleep(SETTLE).await;
	let sent = fetches(&running);
	assert_eq!(sent.len(), 1, "opening the tab twice in a row fetches once");
	let query = sent[0].query.clone().unwrap_or_default();
	for pair in ["limit=25", "roles=true", "everyone=true"] {
		assert!(
			query.contains(pair),
			"the fetch asks as the web client does: {query}"
		);
	}
	let ids = mention_ids(&running.session().state().read());
	assert!(ids.len() >= 2, "the fixtures mention the current user: {ids:?}");
	assert!(
		ids.windows(2).all(|pair| pair[0] > pair[1]),
		"newest first: {ids:?}"
	);
	{
		let state = running.session().state().read();
		let me = state.current_user_id().expect("signed in");
		for message in state.recent_mentions().expect("loaded") {
			assert_ne!(message.author.id, me, "your own messages are not mentions");
			assert!(
				state.channel_guild(message.channel_id).is_some() || message.guild_id.is_some(),
				"each mention carries its server and channel"
			);
		}
	}
	let (first, second) = (ids[0], ids[1]);
	running
		.session()
		.send(Command::RemoveRecentMention { message_id: first });
	running
		.session()
		.send(Command::RemoveRecentMention { message_id: second });
	running
		.until("both mentions to leave the list", |state| {
			let left = mention_ids(state);
			!left.contains(&first) && !left.contains(&second)
		})
		.await;
	let removed: Vec<RestRequest> = running
		.until_wire("both removals", |recording| {
			let removed: Vec<RestRequest> = recording.requests_to("DELETE", ONE_MENTION).cloned().collect();
			(removed.len() == 2).then_some(removed)
		})
		.await;
	let paths: Vec<&str> = removed.iter().map(|request| request.path.as_str()).collect();
	assert!(paths.contains(&format!("{MENTIONS}/{first}").as_str()));
	assert!(paths.contains(&format!("{MENTIONS}/{second}").as_str()));
	running.assert_released_apart(&removed, MUTATION_GAP, "mention removal");
	if let Some(&elsewhere) = ids.get(2) {
		running.mock().dispatch(
			"RECENT_MENTION_DELETE",
			&json!({ "message_id": elsewhere.to_string() }),
		);
		running
			.until("a mention removed on another client to leave the list", |state| {
				!mention_ids(state).contains(&elsewhere)
			})
			.await;
	}
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running.session().send(Command::LoadRecentMentions {
		everyone: false,
		roles: false,
		guild_id: Some(guild_id),
	});
	let narrowed = running
		.until_wire("the filtered fetch", |recording| {
			let sent: Vec<RestRequest> = recording.requests_to("GET", MENTIONS).cloned().collect();
			(sent.len() == 2).then(|| sent[1].clone())
		})
		.await;
	let query = narrowed.query.unwrap_or_default();
	for pair in [
		"everyone=false".to_owned(),
		"roles=false".to_owned(),
		format!("guild_id={guild_id}"),
	] {
		assert!(
			query.contains(&pair),
			"a changed filter fetches again at once: {query}"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bad_mention_never_fails_the_whole_list() {
	let running = Running::start_with(Options::new("mentions-lenient").rest(tuning())).await;
	running.connected().await;
	let channel_id = running.open_text_channel().await;
	let good = running.fixtures().messages[&channel_id.0]
		.values()
		.next_back()
		.cloned()
		.expect("a fixture message");
	let good_id: Snowflake = good["id"]
		.as_str()
		.and_then(|id| id.parse().ok())
		.map(Snowflake)
		.expect("a message id");
	let mut odd = good.clone();
	odd["type"] = json!(9999);
	odd["id"] = json!("1");
	odd["surprise"] = json!({ "nested": true });
	running.mock().respond_once(
		"GET",
		MENTIONS,
		Reply::json(
			200,
			Value::Array(vec![json!({ "id": "not a snowflake" }), good, json!(7), odd]),
		),
	);
	running.session().send(everything());
	running
		.until("the decodable mentions to load", |state| {
			state.recent_mentions().is_some()
		})
		.await;
	let ids = mention_ids(&running.session().state().read());
	assert!(ids.contains(&good_id), "the good mention survives: {ids:?}");
	assert!(
		ids.contains(&Snowflake(1)),
		"an unknown message type and extra fields are tolerated: {ids:?}"
	);
	assert_eq!(ids.len(), 2, "the broken elements are dropped: {ids:?}");
	running.stop().await;
}
