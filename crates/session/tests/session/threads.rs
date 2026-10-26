use std::time::Duration;

use bytes::Bytes;
use mockcord::{Recording, RestRequest};
use protocol::{ChannelType, Snowflake};
use rest::UploadFile;
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const SEND_GAP: Duration = Duration::from_millis(400);
const PACED_AT_LEAST: Duration = Duration::from_millis(350);

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn guild_id(running: &Running) -> Snowflake {
	Snowflake(running.fixtures().guilds[0].id)
}

fn channel_of_kind(running: &Running, kind: ChannelType) -> Snowflake {
	let guild = &running.fixtures().guilds[0];
	guild
		.channel_ids
		.iter()
		.copied()
		.find(|id| running.fixtures().channels[id].kind == kind)
		.map(Snowflake)
		.expect("a channel of that kind in the first guild")
}

fn threads_of(running: &Running, parent: Snowflake) -> Vec<Snowflake> {
	running.fixtures().guilds[0]
		.thread_ids
		.iter()
		.copied()
		.filter(|id| running.fixtures().channels[id].parent_id == Some(parent))
		.map(Snowflake)
		.collect()
}

fn first_text(running: &Running) -> Snowflake {
	Snowflake(running.fixtures().guilds[0].message_channel_ids[0])
}

fn selected(state: &State) -> Option<Snowflake> {
	state.selected().map(|selection| selection.channel_id)
}

fn named_thread(state: &State, parent: Snowflake, name: &str) -> Option<Snowflake> {
	let guild_id = state.channel_guild(parent)?;
	state
		.guild(guild_id)?
		.threads
		.values()
		.find(|thread| thread.parent_id == Some(parent) && thread.name.as_deref() == Some(name))
		.map(|thread| thread.id)
}

fn joined(state: &State, thread: Snowflake) -> Option<bool> {
	state.channel(thread).map(|channel| channel.member.is_some())
}

fn requests<'a>(recording: &'a Recording, method: &str, path: &str) -> Vec<&'a RestRequest> {
	recording
		.requests
		.iter()
		.filter(|request| request.is(method, path))
		.collect()
}

fn body(request: &RestRequest) -> Value {
	request.json().expect("a json body")
}

async fn select(running: &Running, channel_id: Snowflake) {
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id(running)),
		channel_id,
	});
	running
		.until("the channel history to load", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| !cache.ordered.is_empty())
		})
		.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opening_a_thread_loads_it_and_keeps_its_parent_warm() {
	let running = Running::start_with(Options::new("threads-open").rest(tuning())).await;
	running.connected().await;
	let parent = first_text(&running);
	let thread = threads_of(&running, parent)[0];
	select(&running, parent).await;
	select(&running, thread).await;
	{
		let state = running.session().state().read();
		assert_eq!(selected(&state), Some(thread));
		assert!(
			state
				.messages(parent)
				.is_some_and(|cache| !cache.ordered.is_empty()),
			"the parent channel keeps its messages beside the open thread"
		);
	}
	let recording = running.mock().recording();
	assert_eq!(
		requests(&recording, "GET", &format!("/channels/{thread}/messages")).len(),
		1
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn following_and_unfollowing_a_thread_update_its_membership() {
	let running = Running::start_with(Options::new("threads-follow").rest(tuning())).await;
	running.connected().await;
	let thread = threads_of(&running, first_text(&running))[0];
	assert_eq!(joined(&running.session().state().read(), thread), Some(false));
	running.session().send(Command::JoinThread { thread_id: thread });
	running
		.until("the join to echo back", |state| {
			joined(state, thread) == Some(true)
		})
		.await;
	running.session().send(Command::LeaveThread { thread_id: thread });
	running
		.until("the leave to echo back", |state| {
			joined(state, thread) == Some(false)
		})
		.await;
	let recording = running.mock().recording();
	let route = format!("/channels/{thread}/thread-members/@me");
	assert_eq!(requests(&recording, "PUT", &route).len(), 1);
	assert_eq!(requests(&recording, "DELETE", &route).len(), 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_thread_from_a_message_opens_once_discord_creates_it() {
	let running = Running::start_with(Options::new("threads-from-message").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let message_id = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.ordered.last().map(|message| message.id))
		.expect("a message");
	running.session().send(Command::CreateThread {
		channel_id,
		message_id: Some(message_id),
		name: "about this".to_owned(),
		auto_archive_minutes: 1440,
		private: false,
	});
	running
		.until("the new thread to open", |state| {
			selected(state) == Some(message_id) && joined(state, message_id) == Some(true)
		})
		.await;
	let recording = running.mock().recording();
	let created = requests(
		&recording,
		"POST",
		&format!("/channels/{channel_id}/messages/{message_id}/threads"),
	);
	assert_eq!(created.len(), 1);
	assert_eq!(
		body(created[0]),
		json!({ "name": "about this", "auto_archive_duration": 1440 })
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_thread_opens_and_its_first_message_goes_through_the_outbox() {
	let running = Running::start_with(Options::new("threads-new").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::CreateThread {
		channel_id,
		message_id: None,
		name: "fresh start".to_owned(),
		auto_archive_minutes: 60,
		private: false,
	});
	running
		.until("the new thread to open", |state| {
			named_thread(state, channel_id, "fresh start")
				.is_some_and(|thread| selected(state) == Some(thread))
		})
		.await;
	let thread =
		named_thread(&running.session().state().read(), channel_id, "fresh start").expect("the thread");
	running.send_and_wait(thread, "first words", "first words").await;
	let recording = running.mock().recording();
	let created = requests(&recording, "POST", &format!("/channels/{channel_id}/threads"));
	assert_eq!(created.len(), 1);
	assert_eq!(
		body(created[0]),
		json!({ "name": "fresh start", "auto_archive_duration": 60, "type": 11 })
	);
	let sent = requests(&recording, "POST", &format!("/channels/{thread}/messages"));
	assert_eq!(sent.len(), 1);
	assert!(
		body(sent[0])["nonce"].is_string(),
		"the first message is a nonce'd send"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_private_thread_is_created_as_one_and_opens() {
	let running = Running::start_with(Options::new("threads-private").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::CreateThread {
		channel_id,
		message_id: None,
		name: "just us".to_owned(),
		auto_archive_minutes: 1440,
		private: true,
	});
	running
		.until("the private thread to open", |state| {
			named_thread(state, channel_id, "just us").is_some_and(|thread| {
				selected(state) == Some(thread)
					&& state
						.channel(thread)
						.is_some_and(|channel| channel.kind == ChannelType::PrivateThread)
			})
		})
		.await;
	let recording = running.mock().recording();
	let created = requests(&recording, "POST", &format!("/channels/{channel_id}/threads"));
	assert_eq!(created.len(), 1);
	assert_eq!(
		body(created[0]),
		json!({ "name": "just us", "auto_archive_duration": 1440, "type": 12 })
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forum_post_uploads_its_files_carries_its_tags_and_opens() {
	let running = Running::start_with(Options::new("threads-forum-post").rest(tuning())).await;
	running.connected().await;
	let forum = channel_of_kind(&running, ChannelType::GuildForum);
	let tags: Vec<Snowflake> = running.fixtures().channels[&forum.0]
		.available_tags
		.iter()
		.map(|tag| tag.id)
		.take(2)
		.collect();
	running.session().send(Command::CreateForumPost {
		channel_id: forum,
		name: "screen goes dark".to_owned(),
		content: "it happens after login".to_owned(),
		tags: tags.clone(),
		files: vec![UploadFile::new(
			"log.txt",
			Bytes::from_static(b"trace"),
			"text/plain",
		)],
	});
	running
		.until("the post to open with its first message", |state| {
			named_thread(state, forum, "screen goes dark").is_some_and(|post| {
				selected(state) == Some(post)
					&& state.messages(post).is_some_and(|cache| {
						cache
							.ordered
							.iter()
							.any(|message| message.content == "it happens after login")
					})
			})
		})
		.await;
	let recording = running.mock().recording();
	let slots = requests(&recording, "POST", &format!("/channels/{forum}/attachments"));
	assert_eq!(slots.len(), 1);
	assert_eq!(requests(&recording, "PUT", "/upload/**").len(), 1);
	let created = requests(&recording, "POST", &format!("/channels/{forum}/threads"));
	assert_eq!(created.len(), 1);
	let created = body(created[0]);
	assert_eq!(created["name"], json!("screen goes dark"));
	assert_eq!(created["applied_tags"], json!(tags));
	assert_eq!(created["message"]["content"], json!("it happens after login"));
	assert_eq!(created["message"]["attachments"][0]["filename"], json!("log.txt"));
	assert!(created["message"]["attachments"][0]["uploaded_filename"].is_string());
	assert!(
		created_with_json(&recording, forum),
		"the post is json, not multipart"
	);
	running.stop().await;
}

fn created_with_json(recording: &Recording, forum: Snowflake) -> bool {
	recording
		.requests_to("POST", &format!("/channels/{forum}/threads"))
		.all(|request| {
			request
				.header("content-type")
				.is_some_and(|kind| kind.starts_with("application/json"))
		})
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forum_post_waits_its_turn_behind_a_send() {
	let running = Running::start_with(Options::new("threads-forum-paced").rest(RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: SEND_GAP,
		..RestTuning::default()
	}))
	.await;
	let channel_id = running.open_text_channel().await;
	let forum = channel_of_kind(&running, ChannelType::GuildForum);
	running.session().send(Command::SendMessage {
		channel_id,
		content: "just before".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	running.session().send(Command::CreateForumPost {
		channel_id: forum,
		name: "right after".to_owned(),
		content: "a post".to_owned(),
		tags: Vec::new(),
		files: Vec::new(),
	});
	let (sent, posted) = running
		.until_wire("both requests", |recording| {
			let sent = recording
				.requests_to("POST", &format!("/channels/{channel_id}/messages"))
				.next()?
				.at;
			let posted = recording
				.requests_to("POST", &format!("/channels/{forum}/threads"))
				.next()?
				.at;
			Some((sent, posted))
		})
		.await;
	let gap = if posted > sent {
		posted - sent
	} else {
		sent - posted
	};
	assert!(
		gap >= PACED_AT_LEAST,
		"the post was paced as a send, {gap:?} apart"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_gate_holds_new_threads_posts_and_joins() {
	let running = Running::start_with(Options::new("threads-gate").rest(tuning())).await;
	running.connected().await;
	let guild = &running.fixtures().guilds[0];
	let parent = first_text(&running);
	let thread = threads_of(&running, parent)[0];
	let forum = channel_of_kind(&running, ChannelType::GuildForum);
	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({
			"guild_id": guild.id.to_string(),
			"user": running.fixtures().current_user,
			"roles": guild.current_member().roles,
			"joined_at": "2025-01-01T00:00:00+00:00",
			"pending": true,
			"flags": 0,
		}),
	);
	running
		.until("the screening gate", |state| {
			state.channel_lock(forum, chrono::Utc::now()).is_some()
		})
		.await;
	for command in [
		Command::CreateForumPost {
			channel_id: forum,
			name: "held".to_owned(),
			content: "held".to_owned(),
			tags: Vec::new(),
			files: Vec::new(),
		},
		Command::CreateThread {
			channel_id: parent,
			message_id: None,
			name: "held".to_owned(),
			auto_archive_minutes: 60,
			private: false,
		},
		Command::JoinThread { thread_id: thread },
	] {
		let before = rejection_id(&running.session().state().read());
		running.session().send(command);
		running
			.until("the action to be held", |state| rejection_id(state) > before)
			.await;
	}
	let recording = running.mock().recording();
	assert_eq!(requests(&recording, "POST", "/channels/*/threads").len(), 0);
	assert_eq!(
		requests(&recording, "PUT", "/channels/*/thread-members/@me").len(),
		0
	);
	running.stop().await;
}

fn rejection_id(state: &State) -> u64 {
	state.last_rejection().map_or(0, |rejection| rejection.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn forum_posts_load_with_their_first_messages() {
	let running = Running::start_with(Options::new("threads-forum-list").rest(tuning())).await;
	running.connected().await;
	let forum = channel_of_kind(&running, ChannelType::GuildForum);
	let posts = threads_of(&running, forum);
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id(&running)),
		channel_id: forum,
	});
	running.session().send(Command::LoadForumPosts {
		channel_id: forum,
		more: false,
	});
	running
		.until("every post to have its first message", |state| {
			state.forum_posts(forum).len() == posts.len()
				&& posts.iter().all(|post| state.forum_starter(*post).is_some())
		})
		.await;
	let recording = running.mock().recording();
	let searched = requests(&recording, "GET", &format!("/channels/{forum}/threads/search"));
	assert_eq!(searched.len(), 1);
	let query = searched[0].query.clone().unwrap_or_default();
	assert!(query.contains("sort_by=last_message_time"), "{query}");
	assert!(
		!query.contains("archived"),
		"active and archived posts together: {query}"
	);
	assert_eq!(requests(&recording, "GET", "/guilds/*/threads/active").len(), 0);
	assert_eq!(
		requests(&recording, "GET", &format!("/channels/{forum}/messages")).len(),
		0,
		"a forum has no message history to load"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn archived_threads_come_from_the_thread_search() {
	let running = Running::start_with(Options::new("threads-archived").rest(tuning())).await;
	running.connected().await;
	let parent = first_text(&running);
	let thread = threads_of(&running, parent)[1];
	running.session().send(Command::ArchiveThread {
		thread_id: thread,
		archived: true,
	});
	running
		.until("the thread to be archived", |state| {
			state
				.channel(thread)
				.and_then(|channel| channel.thread_metadata.as_ref())
				.is_some_and(|metadata| metadata.archived)
		})
		.await;
	running.session().send(Command::LoadArchivedThreads {
		channel_id: parent,
		more: false,
	});
	running
		.until("the archived list", |state| {
			state
				.archived_threads(parent)
				.iter()
				.any(|channel| channel.id == thread)
		})
		.await;
	let recording = running.mock().recording();
	let searched = requests(&recording, "GET", &format!("/channels/{parent}/threads/search"));
	assert_eq!(searched.len(), 1);
	assert!(
		searched[0]
			.query
			.as_deref()
			.is_some_and(|query| query.contains("archived=true"))
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pins_list_loads_from_the_message_pins_route() {
	let running = Running::start_with(Options::new("threads-pins").rest(tuning())).await;
	let channel_id = running.open_text_channel().await;
	let pinned_in_fixtures: Vec<Snowflake> = running.fixtures().messages[&channel_id.0]
		.values()
		.filter(|message| message["pinned"].as_bool() == Some(true))
		.filter_map(|message| message["id"].as_str()?.parse().ok().map(Snowflake))
		.collect();
	assert!(!pinned_in_fixtures.is_empty(), "the fixtures pin a message");
	running.session().send(Command::LoadPins { channel_id });
	running
		.until("the pins to load", |state| {
			state.pins(channel_id).is_some_and(|pins| {
				pins.len() == pinned_in_fixtures.len()
					&& pins
						.iter()
						.all(|message| pinned_in_fixtures.contains(&message.id))
			})
		})
		.await;
	let unpinned = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| {
			cache
				.ordered
				.iter()
				.rev()
				.find(|message| !message.pinned)
				.map(|message| message.id)
		})
		.expect("an unpinned message");
	running.session().send(Command::PinMessage {
		channel_id,
		message_id: unpinned,
	});
	running
		.until("the pins to reload with the new pin", |state| {
			state
				.pins(channel_id)
				.is_some_and(|pins| pins.iter().any(|message| message.id == unpinned))
		})
		.await;
	let recording = running.mock().recording();
	assert!(
		!requests(
			&recording,
			"GET",
			&format!("/channels/{channel_id}/messages/pins")
		)
		.is_empty()
	);
	assert_eq!(
		requests(
			&recording,
			"PUT",
			&format!("/channels/{channel_id}/messages/pins/{unpinned}")
		)
		.len(),
		1
	);
	assert_eq!(
		requests(&recording, "GET", &format!("/channels/{channel_id}/pins")).len(),
		0
	);
	running.stop().await;
}
