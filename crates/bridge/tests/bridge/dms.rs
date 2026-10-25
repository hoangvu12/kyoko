use bridge::{DmEntry, DmList};
use protocol::Snowflake;
use serde_json::{Value, json};
use session::Command;

use crate::harness::Running;

const FIXTURE_DMS: usize = 5;

async fn listed(name: &str) -> Running {
	let mut running = Running::start(name).await;
	running
		.until("the direct messages to be listed", |view| {
			view.dms.entries().len() == FIXTURE_DMS
		})
		.await;
	running
}

fn ids(list: &DmList) -> Vec<Snowflake> {
	list.entries().iter().map(|entry| entry.id).collect()
}

fn entry(list: &DmList, id: Snowflake) -> DmEntry {
	list.entry(id).cloned().expect("the conversation is listed")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversations_are_listed_by_recent_activity_with_group_names() {
	let running = listed("dms-order").await;
	let list = &running.view.dms;
	let activity: Vec<Snowflake> = list.entries().iter().map(|entry| entry.activity).collect();
	let mut sorted = activity.clone();
	sorted.sort_by(|left, right| right.cmp(left));
	assert_eq!(activity, sorted);
	{
		let state = running.session().state().read();
		for dm in list.entries() {
			let channel = state.channel(dm.id).expect("the channel is known");
			let recipients = channel.recipient_ids.clone();
			match dm.members {
				Some(members) => {
					assert_eq!(members, recipients.len() + 1);
					assert!(dm.recipient.is_none());
					let expected = match channel.name.as_deref().filter(|name| !name.is_empty()) {
						Some(name) => name.to_owned(),
						None => recipients
							.iter()
							.map(|id| state.display_name(None, *id))
							.collect::<Vec<_>>()
							.join(", "),
					};
					assert_eq!(dm.name, expected);
				}
				None => assert_eq!(dm.recipient, recipients.first().copied()),
			}
		}
	}
	let groups: Vec<&DmEntry> = list.entries().iter().filter(|dm| dm.is_group()).collect();
	assert_eq!(groups.len(), 2);
	assert!(groups.iter().any(|group| group.name == "weekend crew"));
	assert!(groups.iter().any(|group| group.name.contains(", ")));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_message_moves_its_conversation_to_the_top() {
	let mut running = listed("dms-reorder").await;
	let oldest = *ids(&running.view.dms).last().expect("a conversation");
	let before = entry(&running.view.dms, oldest).mentions;
	running
		.server
		.post_message(oldest, "are you around?")
		.expect("mockcord posts into the conversation");
	running
		.until("the conversation to move to the top", |view| {
			view.dms.most_recent() == Some(oldest)
		})
		.await;
	let moved = entry(&running.view.dms, oldest);
	assert!(moved.unread);
	assert_eq!(moved.mentions, before + 1);
	assert_eq!(running.view.dms.entries().len(), FIXTURE_DMS);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_conversation_created_later_appears_at_the_top() {
	let mut running = listed("dms-late").await;
	let known: Vec<Snowflake> = {
		let state = running.session().state().read();
		running
			.view
			.dms
			.entries()
			.iter()
			.filter_map(|dm| state.channel(dm.id))
			.flat_map(|channel| channel.recipient_ids.clone())
			.collect()
	};
	let fixtures = running.server.fixtures();
	let me = fixtures.current_user.id;
	let stranger = fixtures
		.users
		.values()
		.find(|user| user.id != me && !known.contains(&user.id))
		.cloned()
		.expect("a user without a conversation");
	let channel_id = Snowflake(running.view.dms.entries()[0].activity.0 + (1 << 30));
	let payload: Value = json!({
		"id": channel_id.to_string(),
		"type": 1,
		"last_message_id": null,
		"recipients": [stranger],
	});
	running.server.control().dispatch("CHANNEL_CREATE", &payload);
	running
		.until("the late conversation to be listed first", |view| {
			view.dms.most_recent() == Some(channel_id)
		})
		.await;
	let late = entry(&running.view.dms, channel_id);
	assert_eq!(late.recipient, Some(stranger.id));
	assert!(!late.unread);
	assert_eq!(running.view.dms.entries().len(), FIXTURE_DMS + 1);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_requests_and_spam_stay_out_of_the_list() {
	let mut running = listed("dms-requests").await;
	let marked: Vec<Snowflake> = ids(&running.view.dms).into_iter().take(2).collect();
	let flags: Vec<(String, &'static str)> = marked
		.iter()
		.zip(["is_message_request", "is_spam"])
		.map(|(id, flag)| (id.to_string(), flag))
		.collect();
	running.server.control().transform_ready(move |_, ready| {
		let Some(channels) = ready["private_channels"].as_array_mut() else {
			return;
		};
		for channel in channels {
			let id = channel["id"].as_str().unwrap_or_default().to_owned();
			if let Some((_, flag)) = flags.iter().find(|(marked, _)| *marked == id) {
				channel[*flag] = json!(true);
			}
		}
	});
	running.restart().await;
	running
		.until("the requests to leave the list", |view| {
			view.dms.requests() == 2 && view.dms.entries().len() == FIXTURE_DMS - 2
		})
		.await;
	for id in marked {
		assert!(running.view.dms.entry(id).is_none());
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reading_a_conversation_clears_its_unread_and_mentions() {
	let mut running = listed("dms-read").await;
	let unread = running
		.view
		.dms
		.entries()
		.iter()
		.find(|dm| dm.unread && dm.mentions > 0)
		.map(|dm| dm.id)
		.expect("a fixture conversation with mentions");
	running
		.session()
		.send(Command::MarkChannelRead { channel_id: unread });
	running
		.until("the conversation to read as read", |view| {
			view.dms
				.entry(unread)
				.is_some_and(|dm| !dm.unread && dm.mentions == 0)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn home_keeps_guilds_unselected_and_opens_conversations_without_a_guild() {
	let mut running = listed("dms-home").await;
	let guild_id = running
		.view
		.sidebar
		.selected()
		.expect("a guild is selected first");
	let dm = running.view.dms.most_recent().expect("a conversation");
	let state = running.session().state().clone();
	assert!(running.view.sidebar.show_home(&state.read()));
	assert_eq!(running.view.sidebar.selected(), None);
	assert!(running.view.sidebar.rows().is_empty());
	let command = running.view.timeline.select(&state.read(), None, dm);
	assert!(matches!(
		command,
		Some(Command::SelectChannel { guild_id: None, channel_id }) if channel_id == dm
	));
	running.session().send(command.expect("a select command"));
	running
		.until("the conversation's history", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	running
		.server
		.post_message(dm, "still home")
		.expect("mockcord posts into the conversation");
	running
		.until("the new message in the open conversation", |view| {
			view.timeline
				.messages()
				.any(|message| message.content == "still home")
		})
		.await;
	assert!(running.view.sidebar.is_home());
	assert_eq!(running.view.sidebar.selected(), None);
	assert!(running.select_guild(guild_id).is_some());
	assert!(!running.view.sidebar.is_home());
	running.stop().await;
}
