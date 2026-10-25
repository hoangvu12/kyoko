use std::time::{Duration, Instant};

use mockcord::{Recording, Reply};
use protocol::{ChannelType, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const DELAY: Duration = Duration::from_millis(150);
const GAP: Duration = Duration::from_millis(700);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		settings_delay: DELAY,
		collapse_gap: GAP,
		..RestTuning::default()
	}
}

async fn start(name: &'static str) -> (Running, Snowflake, Vec<Snowflake>) {
	let running = Running::start_with(Options::new(name).rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("the first guild's channels", |state| {
			state.guild(guild_id).is_some()
		})
		.await;
	let categories = {
		let state = running.session().state().read();
		let mut categories: Vec<(i64, Snowflake)> = state
			.guild(guild_id)
			.expect("the guild is loaded")
			.channels
			.values()
			.filter(|channel| channel.kind == ChannelType::GuildCategory)
			.map(|channel| (channel.position.unwrap_or(0), channel.id))
			.collect();
		categories.sort();
		categories.into_iter().map(|(_, id)| id).collect()
	};
	(running, guild_id, categories)
}

fn settings_path(guild_id: Snowflake) -> String {
	format!("/users/@me/guilds/{guild_id}/settings")
}

fn writes(recording: &Recording, guild_id: Snowflake) -> Vec<(Value, Instant)> {
	recording
		.requests_to("PATCH", &settings_path(guild_id))
		.filter_map(|request| Some((request.json()?, request.at)))
		.collect()
}

fn overrides(categories: &[(Snowflake, bool)]) -> Value {
	let entries: serde_json::Map<String, Value> = categories
		.iter()
		.map(|(id, collapsed)| (id.to_string(), json!({ "collapsed": collapsed })))
		.collect();
	json!({ "channel_overrides": entries })
}

fn toggle(running: &Running, guild_id: Snowflake, category_id: Snowflake) {
	running.session().send(Command::ToggleCategory {
		guild_id,
		category_id,
	});
}

async fn settle() {
	tokio::time::sleep(GAP * 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_collapse_shows_at_once_and_is_saved_as_a_channel_override() {
	let (running, guild_id, categories) = start("collapse-save").await;
	let category = categories[0];
	toggle(&running, guild_id, category);
	running
		.until("the local collapse", |state: &State| state.is_collapsed(category))
		.await;
	let (sent, _) = running
		.until_wire("the settings write", |recording| {
			writes(recording, guild_id).into_iter().next()
		})
		.await;
	assert_eq!(sent, overrides(&[(category, true)]));
	settle().await;
	assert_eq!(writes(&running.mock().recording(), guild_id).len(), 1);
	assert!(running.session().state().read().is_collapsed(category));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quick_toggles_are_coalesced_and_later_ones_wait_for_the_gap() {
	let (running, guild_id, categories) = start("collapse-coalesce").await;
	let (first, second) = (categories[0], categories[1]);
	toggle(&running, guild_id, first);
	toggle(&running, guild_id, first);
	toggle(&running, guild_id, first);
	toggle(&running, guild_id, second);
	let (sent, first_at) = running
		.until_wire("the first write", |recording| {
			writes(recording, guild_id).into_iter().next()
		})
		.await;
	assert_eq!(sent, overrides(&[(first, true), (second, true)]));
	toggle(&running, guild_id, second);
	let (sent, second_at) = running
		.until_wire("the second write", |recording| {
			writes(recording, guild_id).into_iter().nth(1)
		})
		.await;
	assert_eq!(sent, overrides(&[(second, false)]));
	assert!(
		second_at.duration_since(first_at) >= GAP - Duration::from_millis(50),
		"the second write came {:?} after the first",
		second_at.duration_since(first_at)
	);
	settle().await;
	assert_eq!(writes(&running.mock().recording(), guild_id).len(), 2);
	let (first_collapsed, second_collapsed) = {
		let state = running.session().state().read();
		(state.is_collapsed(first), state.is_collapsed(second))
	};
	assert!(first_collapsed && !second_collapsed);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_collapsed_category_survives_a_restart() {
	let (mut running, guild_id, categories) = start("collapse-restart").await;
	let category = categories[0];
	toggle(&running, guild_id, category);
	running
		.until_wire("the settings write", |recording| {
			writes(recording, guild_id).into_iter().next()
		})
		.await;
	settle().await;
	running.restart().await;
	running.connected().await;
	running
		.until("the collapse after a restart", |state: &State| {
			state.is_collapsed(category)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_write_expands_the_category_again() {
	let (running, guild_id, categories) = start("collapse-refused").await;
	let category = categories[0];
	running.mock().respond_once(
		"PATCH",
		&settings_path(guild_id),
		Reply::json(400, json!({ "code": 50035, "message": "Invalid Form Body" })),
	);
	toggle(&running, guild_id, category);
	running
		.until_wire("the refused write", |recording| {
			writes(recording, guild_id).into_iter().next()
		})
		.await;
	running
		.until("the category expanded again", |state: &State| {
			!state.is_collapsed(category) && state.last_rejection().is_some()
		})
		.await;
	settle().await;
	assert_eq!(writes(&running.mock().recording(), guild_id).len(), 1);
	running.stop().await;
}
