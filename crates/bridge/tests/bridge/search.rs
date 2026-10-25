use std::time::Duration;

use bridge::{SearchFilter, SearchPanel, parse_search, today};
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(15);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filters_in_the_text_become_query_fields_and_chips() {
	let running = Running::start("search-parse").await;
	let state = running.session().state().read();
	let guild_id = state.guild_summaries()[0].id;
	let guild = state.guild(guild_id).expect("the guild");
	let member = *guild.members.keys().next().expect("a member");
	let username = state.user(member).expect("the member's user").username.clone();
	let channel = guild
		.channels
		.values()
		.find(|channel| channel.kind == protocol::ChannelType::GuildText)
		.expect("a text channel");
	let channel_name = channel.name.clone().expect("a name");
	let text = format!(
		"parser from:{username} has:image has:nonsense in:#{channel_name} before:2026-01-02 pinned:true mentions:<@42> works"
	);
	let parsed = parse_search(&text, &state, Some(guild_id), None);
	assert_eq!(parsed.query.author_id, Some(member));
	assert_eq!(parsed.query.mentions, Some(Snowflake(42)));
	assert_eq!(parsed.query.has, ["image"]);
	assert_eq!(parsed.query.channel_id, Some(channel.id));
	assert_eq!(parsed.query.pinned, Some(true));
	assert_eq!(
		parsed
			.query
			.before
			.map(|id| id.timestamp().date_naive().to_string()),
		Some("2026-01-02".to_owned())
	);
	assert_eq!(
		parsed.query.content.as_deref(),
		Some("parser has:nonsense works"),
		"an unknown value stays in the text"
	);
	let filters: Vec<SearchFilter> = parsed.chips.iter().map(|chip| chip.filter).collect();
	assert_eq!(
		filters,
		[
			SearchFilter::From,
			SearchFilter::Has,
			SearchFilter::In,
			SearchFilter::Before,
			SearchFilter::Pinned,
			SearchFilter::Mentions,
		]
	);
	assert_eq!(parsed.chips[0].label(), format!("from: {username}"));

	let during = parse_search("during:2026-03-04", &state, Some(guild_id), None).query;
	let (after, before) = (during.after.expect("a start"), during.before.expect("an end"));
	assert!(after < before);
	assert_eq!(
		Snowflake(after.get() + 1).timestamp().date_naive().to_string(),
		"2026-03-04"
	);
	assert_eq!(before.timestamp().date_naive().to_string(), "2026-03-05");
	assert_eq!(
		parse_search("in:general", &state, None, Some(Snowflake(1)))
			.query
			.channel_id,
		None,
		"a conversation has no channels to narrow to"
	);
	drop(state);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_results_are_lowered_into_entries_the_menu_can_act_on() {
	let running = Running::start("search-panel").await;
	let (guild_id, query) = {
		let state = running.session().state().read();
		let guild_id = state.guild_summaries()[0].id;
		(guild_id, parse_search("the", &state, Some(guild_id), None).query)
	};
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query,
	});
	let deadline = tokio::time::Instant::now() + WAIT;
	let results = loop {
		if let Some(results) = running.session().search_results()
			&& !results.is_busy()
		{
			break results;
		}
		assert!(tokio::time::Instant::now() < deadline, "the search finished");
		tokio::time::sleep(Duration::from_millis(50)).await;
	};
	let state = running.session().state().read();
	let panel = SearchPanel::of(&results, &state, today());
	assert_eq!(panel.title, format!("{} Results", results.total));
	assert_eq!(panel.hits.len(), results.messages.len());
	assert!(panel.notice.is_none());
	for (hit, message) in panel.hits.iter().zip(&results.messages) {
		assert_eq!(hit.entry.id, message.id);
		assert_eq!(hit.channel_id, message.channel_id);
		assert_eq!(hit.guild_id, Some(guild_id));
		assert!(hit.place.starts_with('#'));
		assert!(hit.text.to_lowercase().contains("the"));
		assert!(!hit.author.is_empty());
		assert!(bridge::has_actions(&hit.entry));
	}
	drop(state);
	running.stop().await;
}
