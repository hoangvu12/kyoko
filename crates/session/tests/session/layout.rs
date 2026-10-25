use std::time::{Duration, Instant};

use mockcord::{Recording, Reply};
use protocol::{ProtoGuildFolder, ProtoGuildFolders, Snowflake, decode_guild_folders};
use serde_json::json;
use session::{Command, RestTuning};
use state::{RailEntry, State};

use crate::harness::{Options, Running};

const SETTINGS: &str = "/users/@me/settings-proto/1";
const DELAY: Duration = Duration::from_millis(150);
const GAP: Duration = Duration::from_millis(900);
const GUILDS: usize = 6;
const NEW_FOLDER: u64 = 1_761_000_000_123;

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		settings_delay: DELAY,
		settings_gap: GAP,
		..RestTuning::default()
	}
}

async fn start(name: &'static str) -> Running {
	let running = Running::start_with(Options::new(name).guilds(GUILDS).rest(tuning())).await;
	running.connected().await;
	running
		.until("the guild layout", |state| {
			state.guild_layout().len() == GUILDS - 1
		})
		.await;
	running
}

fn layout(running: &Running) -> Vec<RailEntry> {
	running.session().state().read().guild_layout()
}

fn writes(recording: &Recording) -> Vec<(ProtoGuildFolders, Instant)> {
	recording
		.requests_to("PATCH", SETTINGS)
		.filter_map(|request| {
			let body = request.json()?;
			let folders = decode_guild_folders(body["settings"].as_str()?)?;
			Some((folders, request.at))
		})
		.collect()
}

fn proto_of(layout: &[RailEntry]) -> ProtoGuildFolders {
	state::guild_folders_proto(layout)
}

fn top_level_guilds(layout: &[RailEntry]) -> Vec<Snowflake> {
	layout
		.iter()
		.filter_map(|entry| match entry {
			RailEntry::Guild(id) => Some(*id),
			RailEntry::Folder { .. } => None,
		})
		.collect()
}

fn moved_to_front(layout: &[RailEntry], guild: Snowflake) -> Vec<RailEntry> {
	let mut next: Vec<RailEntry> = layout
		.iter()
		.filter(|entry| **entry != RailEntry::Guild(guild))
		.cloned()
		.collect();
	next.insert(0, RailEntry::Guild(guild));
	next
}

async fn settle() {
	tokio::time::sleep(GAP * 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reorder_shows_at_once_and_is_saved_in_one_settings_proto_request() {
	let running = start("layout-reorder").await;
	let before = layout(&running);
	let last = *top_level_guilds(&before).last().expect("a guild outside folders");
	let next = moved_to_front(&before, last);
	running.session().send(Command::SetGuildLayout(next.clone()));
	running
		.until("the local order", |state: &State| state.guild_layout() == next)
		.await;
	let (sent, _) = running
		.until_wire("the settings write", |recording| {
			writes(recording).into_iter().next()
		})
		.await;
	assert_eq!(sent, proto_of(&next));
	assert!(sent.folders.iter().all(|folder| !folder.guild_ids.is_empty()));
	assert!(
		sent.folders
			.iter()
			.filter(|folder| folder.id.is_none())
			.all(|folder| folder.guild_ids.len() == 1 && folder.name.is_none() && folder.color.is_none())
	);
	settle().await;
	assert_eq!(writes(&running.mock().recording()).len(), 1);
	assert_eq!(layout(&running), next);
	assert!(
		running
			.mock()
			.recording()
			.requests_to("PATCH", "/users/@me/settings")
			.next()
			.is_none()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quick_drops_are_coalesced_and_later_ones_wait_for_the_gap() {
	let running = start("layout-coalesce").await;
	let start_layout = layout(&running);
	let guilds = top_level_guilds(&start_layout);
	let first = moved_to_front(&start_layout, guilds[guilds.len() - 1]);
	let second = moved_to_front(&first, guilds[guilds.len() - 2]);
	running.session().send(Command::SetGuildLayout(first));
	running.session().send(Command::SetGuildLayout(second.clone()));
	let (sent, first_at) = running
		.until_wire("the first write", |recording| {
			writes(recording).into_iter().next()
		})
		.await;
	assert_eq!(sent, proto_of(&second));
	let third = moved_to_front(&second, guilds[guilds.len() - 3]);
	running.session().send(Command::SetGuildLayout(third.clone()));
	let (sent, second_at) = running
		.until_wire("the second write", |recording| {
			writes(recording).into_iter().nth(1)
		})
		.await;
	assert_eq!(sent, proto_of(&third));
	assert!(
		second_at.duration_since(first_at) >= GAP - Duration::from_millis(50),
		"the second write came {:?} after the first",
		second_at.duration_since(first_at)
	);
	settle().await;
	assert_eq!(writes(&running.mock().recording()).len(), 2);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_folder_and_a_move_into_it_survive_a_restart() {
	let mut running = start("layout-folder").await;
	let before = layout(&running);
	let guilds = top_level_guilds(&before);
	let (target, moved) = (guilds[0], guilds[1]);
	let next: Vec<RailEntry> = before
		.iter()
		.filter(|entry| **entry != RailEntry::Guild(moved))
		.map(|entry| match entry {
			RailEntry::Guild(id) if *id == target => RailEntry::Folder {
				id: NEW_FOLDER,
				name: None,
				color: None,
				guilds: vec![target, moved],
			},
			other => other.clone(),
		})
		.collect();
	running.session().send(Command::SetGuildLayout(next.clone()));
	let (sent, _) = running
		.until_wire("the folder write", |recording| {
			writes(recording).into_iter().next()
		})
		.await;
	assert!(sent.folders.contains(&ProtoGuildFolder {
		id: Some(NEW_FOLDER as i64),
		name: None,
		color: None,
		guild_ids: vec![target, moved],
	}));
	settle().await;
	running.restart().await;
	running.connected().await;
	running
		.until("the saved layout after a restart", |state: &State| {
			state.guild_layout() == next
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_write_puts_the_old_order_back() {
	let running = start("layout-refused").await;
	let before = layout(&running);
	let last = *top_level_guilds(&before).last().expect("a guild outside folders");
	let next = moved_to_front(&before, last);
	running.mock().respond_once(
		"PATCH",
		SETTINGS,
		Reply::json(400, json!({ "code": 50035, "message": "Invalid Form Body" })),
	);
	running.session().send(Command::SetGuildLayout(next.clone()));
	running
		.until_wire("the refused write", |recording| {
			writes(recording).into_iter().next()
		})
		.await;
	running
		.until("the old order back", |state: &State| {
			state.guild_layout() == before && state.last_rejection().is_some()
		})
		.await;
	settle().await;
	assert_eq!(writes(&running.mock().recording()).len(), 1);
	running.stop().await;
}
