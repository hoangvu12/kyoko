use std::collections::HashSet;

use bridge::{ForumSort, ThreadTab, today};
use protocol::{ChannelType, Snowflake};
use session::Command;
use state::State;

use crate::harness::Running;

fn first_guild(state: &State) -> Snowflake {
	state.guild_summaries()[0].id
}

fn channel_of(state: &State, kind: ChannelType) -> Snowflake {
	let guild_id = first_guild(state);
	state
		.guild_channel_tree(guild_id)
		.into_iter()
		.flat_map(|node| node.children)
		.find(|row| row.kind == kind && !row.locked)
		.map(|row| row.id)
		.expect("a channel of that kind")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_thread_list_shows_a_channels_active_threads_newest_first() {
	let running = Running::start("threads-list").await;
	{
		let state = running.session().state().read();
		let guild_id = first_guild(&state);
		let parent = channel_of(&state, ChannelType::GuildText);
		let mut expected: Vec<(Snowflake, Snowflake)> = state
			.guild(guild_id)
			.expect("the guild")
			.threads
			.values()
			.filter(|thread| thread.parent_id == Some(parent))
			.map(|thread| (thread.last_message_id.unwrap_or(thread.id), thread.id))
			.collect();
		expected.sort_by(|left, right| right.cmp(left));
		let entries = bridge::thread_list(&state, parent, ThreadTab::Active, today());
		assert!(!entries.is_empty(), "the fixtures give the channel threads");
		assert_eq!(
			entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
			expected.iter().map(|(_, id)| *id).collect::<Vec<_>>()
		);
		for entry in &entries {
			let thread = state.channel(entry.id).expect("the thread");
			let owner = state.display_name(Some(guild_id), thread.owner_id.expect("an owner"));
			assert!(
				entry.detail.starts_with(&format!("{owner} - ")),
				"{}",
				entry.detail
			);
			assert!(entry.detail.contains(" messages - "), "{}", entry.detail);
			assert!(!entry.archived);
		}
		assert!(bridge::thread_list(&state, parent, ThreadTab::Archived, today()).is_empty());
		assert!(bridge::can_create_threads(&state, parent));
		let header = bridge::thread_header(&state, entries[0].id).expect("a thread header");
		assert_eq!(header.parent_id, Some(parent));
		assert_eq!(header.name, entries[0].name);
		assert!(
			bridge::thread_header(&state, parent).is_none(),
			"a channel has no thread header"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forum_cards_carry_excerpts_and_follow_the_tag_filter_and_sort() {
	let mut running = Running::start("threads-forum").await;
	let forum = channel_of(&running.session().state().read(), ChannelType::GuildForum);
	running.session().send(Command::LoadForumPosts {
		channel_id: forum,
		more: false,
	});
	running
		.until_state("the forum posts", |state| state.forum_page(forum).is_some())
		.await;
	{
		let state = running.session().state().read();
		let cards = bridge::forum_cards(&state, forum, ForumSort::Activity, &HashSet::new(), today());
		assert!(cards.len() >= 2, "the fixtures have forum posts");
		assert!(
			cards.iter().all(|card| !card.excerpt.is_empty()),
			"every post shows its first message"
		);
		assert!(
			cards
				.iter()
				.all(|card| card.replies.ends_with("replies") || card.replies == "1 reply")
		);
		assert!(
			cards
				.iter()
				.all(|card| card.tags.len() == 1 && !card.author.is_empty())
		);
		let tags = bridge::forum_tags(&state, forum);
		assert_eq!(tags.len(), 3);
		assert!(tags[0].label.ends_with(" bug"), "{}", tags[0].label);
		let filter: HashSet<Snowflake> = [cards[0].tags[0].id].into_iter().collect();
		let filtered = bridge::forum_cards(&state, forum, ForumSort::Activity, &filter, today());
		assert!(!filtered.is_empty());
		assert!(
			filtered
				.iter()
				.all(|card| card.tags.iter().any(|tag| filter.contains(&tag.id)))
		);
		assert!(
			filtered.len() < cards.len(),
			"the filter hides posts without the tag"
		);
		let by_creation = bridge::forum_cards(&state, forum, ForumSort::Creation, &HashSet::new(), today());
		let ids: Vec<Snowflake> = by_creation.iter().map(|card| card.id).collect();
		let newest_first = {
			let mut sorted = ids.clone();
			sorted.sort_by(|left, right| right.cmp(left));
			sorted
		};
		assert_eq!(ids, newest_first, "date posted puts the newest post first");
		let form = bridge::new_post_form(&state, forum);
		assert_eq!(form.tags, tags);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_pins_list_names_each_author_and_shows_a_snippet() {
	let mut running = Running::start("threads-pins").await;
	let parent = channel_of(&running.session().state().read(), ChannelType::GuildText);
	assert!(bridge::pin_entries(&running.session().state().read(), parent, today()).is_none());
	running.session().send(Command::LoadPins { channel_id: parent });
	running
		.until_state("the pins", |state| state.pins(parent).is_some())
		.await;
	{
		let state = running.session().state().read();
		let entries = bridge::pin_entries(&state, parent, today()).expect("the pins loaded");
		assert!(!entries.is_empty(), "the fixtures pin a message");
		let guild_id = state.channel_guild(parent);
		for (entry, message) in entries.iter().zip(state.pins(parent).expect("pins")) {
			assert_eq!(entry.id, message.id);
			assert_eq!(entry.author, state.display_name(guild_id, message.author.id));
			assert!(!entry.text.is_empty());
			assert!(entry.text.chars().count() <= bridge::PIN_SNIPPET);
		}
	}
	running.stop().await;
}
