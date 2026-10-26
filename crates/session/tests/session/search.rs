use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mockcord::{Reply, RestRequest};
use protocol::Snowflake;
use rest::api::SearchQuery;
use serde_json::json;
use session::{Command, RestTuning, SearchFailure, SearchResults, SearchStatus};
use state::SendFailure;

use crate::harness::{Options, Running};

const SEARCH_ROUTE: &str = "/guilds/*/messages/search";
const QUIET: Duration = Duration::from_millis(400);
const PAGE: usize = 25;

fn tuning() -> RestTuning {
	RestTuning {
		search_delay: Duration::from_millis(50),
		..RestTuning::default()
	}
}

fn results(running: &Running) -> Option<Arc<SearchResults>> {
	running.session().search_results()
}

fn searches(running: &Running) -> Vec<RestRequest> {
	running
		.mock()
		.recording()
		.requests_to("GET", SEARCH_ROUTE)
		.cloned()
		.collect()
}

fn params(request: &RestRequest) -> Vec<(String, String)> {
	url::form_urlencoded::parse(request.query.as_deref().unwrap_or_default().as_bytes())
		.map(|(key, value)| (key.into_owned(), value.into_owned()))
		.collect()
}

fn param<'a>(pairs: &'a [(String, String)], key: &str) -> Vec<&'a str> {
	pairs
		.iter()
		.filter(|(name, _)| name == key)
		.map(|(_, value)| value.as_str())
		.collect()
}

fn busiest_author(running: &Running, guild: usize) -> Snowflake {
	let fixtures = running.fixtures();
	let mut counts: HashMap<u64, usize> = HashMap::new();
	for channel_id in &fixtures.guilds[guild].message_channel_ids {
		for message in fixtures
			.messages
			.get(channel_id)
			.into_iter()
			.flat_map(|page| page.values())
		{
			if let Some(author) = message["author"]["id"].as_str().and_then(|id| id.parse().ok()) {
				*counts.entry(author).or_default() += 1;
			}
		}
	}
	let (author, _) = counts
		.into_iter()
		.max_by_key(|(author, count)| (*count, *author))
		.expect("a guild with messages");
	Snowflake(author)
}

async fn settled(running: &Running, what: &str) -> Arc<SearchResults> {
	running
		.eventually(what, || {
			results(running).is_some_and(|results| !results.is_busy())
		})
		.await;
	results(running).expect("search results")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_guild_search_sends_its_filters_and_pages_through_the_results() {
	let running = Running::start_with(Options::new("search-pages").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	let author = busiest_author(&running, 0);
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			author_id: Some(author),
			..SearchQuery::default()
		},
	});
	let first = settled(&running, "the first page").await;
	assert_eq!(first.status, SearchStatus::Ready);
	assert!(
		first.total > PAGE as u64,
		"the busiest author has more than a page of messages, got {}",
		first.total
	);
	assert_eq!(first.messages.len(), PAGE);
	assert!(first.messages.iter().all(|message| message.author.id == author));
	assert!(first.messages.windows(2).all(|pair| pair[0].id > pair[1].id));
	assert!(first.has_more());

	running.session().send(Command::SearchMore);
	running
		.eventually("the second page", || {
			results(&running).is_some_and(|results| results.messages.len() > PAGE && !results.is_busy())
		})
		.await;
	let second = results(&running).expect("search results");
	assert_eq!(second.serial, first.serial);
	assert_eq!(second.messages.len(), (2 * PAGE).min(second.total as usize));
	assert_eq!(second.messages[..PAGE], first.messages[..]);
	assert!(second.messages.iter().all(|message| message.author.id == author));

	let wire = searches(&running);
	assert_eq!(wire.len(), 2, "one request per page: {wire:?}");
	let (page_one, page_two) = (params(&wire[0]), params(&wire[1]));
	assert_eq!(param(&page_one, "author_id"), [author.to_string()]);
	assert!(
		param(&page_one, "offset").is_empty(),
		"the first page sends no offset"
	);
	assert_eq!(param(&page_two, "offset"), ["25"]);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oldest_first_search_asks_for_ascending_order_and_gets_it() {
	let running = Running::start_with(Options::new("search-oldest").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	let author = busiest_author(&running, 0);
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			author_id: Some(author),
			oldest_first: true,
			..SearchQuery::default()
		},
	});
	let first = settled(&running, "the oldest page").await;
	assert_eq!(first.status, SearchStatus::Ready);
	assert!(first.messages.len() > 1);
	assert!(first.messages.windows(2).all(|pair| pair[0].id < pair[1].id));
	let wire = searches(&running);
	let sent = params(&wire[0]);
	assert_eq!(param(&sent, "sort_by"), ["timestamp"]);
	assert_eq!(param(&sent, "sort_order"), ["asc"]);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_filter_reaches_the_wire_and_narrows_the_results() {
	let running = Running::start_with(Options::new("search-filters").rest(tuning())).await;
	running.connected().await;
	let guild = &running.fixtures().guilds[0];
	let guild_id = Snowflake(guild.id);
	let channel_id = Snowflake(guild.message_channel_ids[0]);
	let author = busiest_author(&running, 0);
	let newest = running.fixtures().messages[&channel_id.get()]
		.keys()
		.next_back()
		.copied()
		.expect("a message");
	let before = Snowflake(newest);
	let after = Snowflake(1);
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			content: Some("the".to_owned()),
			author_id: Some(author),
			mentions: Some(Snowflake(running.fixtures().current_user.id.get())),
			has: vec!["link".to_owned(), "embed".to_owned()],
			before: Some(before),
			after: Some(after),
			channel_id: Some(channel_id),
			pinned: Some(false),
			oldest_first: false,
			offset: None,
		},
	});
	let found = settled(&running, "the filtered search").await;
	assert_eq!(found.status, SearchStatus::Ready);
	let wire = searches(&running);
	assert_eq!(wire.len(), 1);
	let pairs = params(&wire[0]);
	assert_eq!(param(&pairs, "content"), ["the"]);
	assert_eq!(param(&pairs, "author_id"), [author.to_string()]);
	assert_eq!(
		param(&pairs, "mentions"),
		[running.fixtures().current_user.id.to_string()]
	);
	assert_eq!(param(&pairs, "has"), ["link", "embed"]);
	assert_eq!(param(&pairs, "max_id"), [before.to_string()]);
	assert_eq!(param(&pairs, "min_id"), [after.to_string()]);
	assert_eq!(param(&pairs, "channel_id"), [channel_id.to_string()]);
	assert_eq!(param(&pairs, "pinned"), ["false"]);

	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			channel_id: Some(channel_id),
			before: Some(before),
			..SearchQuery::default()
		},
	});
	running
		.eventually("the channel search", || {
			results(&running).is_some_and(|results| results.serial > found.serial && !results.is_busy())
		})
		.await;
	let narrowed = results(&running).expect("search results");
	assert!(!narrowed.messages.is_empty());
	assert!(
		narrowed
			.messages
			.iter()
			.all(|message| message.channel_id == channel_id && message.id < before)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_burst_of_searches_sends_only_the_last_one() {
	let running = Running::start_with(Options::new("search-burst").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	for word in ["t", "th", "the"] {
		running.session().send(Command::Search {
			guild_id: Some(guild_id),
			channel_id: None,
			query: SearchQuery {
				content: Some(word.to_owned()),
				..SearchQuery::default()
			},
		});
	}
	settled(&running, "the last search").await;
	tokio::time::sleep(QUIET).await;
	let wire = searches(&running);
	assert_eq!(wire.len(), 1, "the burst was one request: {wire:?}");
	assert_eq!(param(&params(&wire[0]), "content"), ["the"]);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_index_that_is_not_ready_is_asked_again_after_its_wait() {
	let running = Running::start_with(Options::new("search-indexing").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running.mock().respond_times(
		"GET",
		SEARCH_ROUTE,
		Reply::json(
			202,
			json!({
				"message": "Index not yet available. Try again later",
				"code": 110000,
				"documents_indexed": 0,
				"retry_after": 0.3,
			}),
		),
		2,
	);
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			content: Some("the".to_owned()),
			..SearchQuery::default()
		},
	});
	running
		.eventually("the indexing notice", || {
			results(&running).is_some_and(|results| results.status == SearchStatus::Indexing)
		})
		.await;
	let found = settled(&running, "the results once indexed").await;
	assert_eq!(found.status, SearchStatus::Ready);
	assert!(!found.messages.is_empty());
	let wire = searches(&running);
	assert_eq!(wire.len(), 3, "two indexing replies, then the results: {wire:?}");
	assert_eq!((wire[0].status, wire[1].status, wire[2].status), (202, 202, 200));
	let waits = [
		wire[1].at.saturating_duration_since(wire[0].at),
		wire[2].at.saturating_duration_since(wire[1].at),
	];
	assert!(
		waits.iter().all(|wait| *wait >= Duration::from_millis(300)),
		"each retry waits for retry_after: {waits:?}"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_index_that_never_becomes_ready_gives_up() {
	let running = Running::start_with(Options::new("search-index-gives-up").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running.mock().respond_always(
		"GET",
		SEARCH_ROUTE,
		Reply::json(202, json!({ "code": 110000, "retry_after": 0.05 })),
	);
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery::default(),
	});
	let gave_up = settled(&running, "the search to give up").await;
	assert_eq!(gave_up.status, SearchStatus::Failed(SearchFailure::StillIndexing));
	tokio::time::sleep(QUIET).await;
	assert_eq!(searches(&running).len(), 5, "a bounded number of tries");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_search_reports_why_and_is_not_retried() {
	let running = Running::start_with(Options::new("search-refused").rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.mock()
		.respond_once("GET", SEARCH_ROUTE, Reply::forbidden());
	running.session().send(Command::Search {
		guild_id: Some(guild_id),
		channel_id: None,
		query: SearchQuery {
			content: Some("the".to_owned()),
			..SearchQuery::default()
		},
	});
	let refused = settled(&running, "the refusal").await;
	assert_eq!(
		refused.status,
		SearchStatus::Failed(SearchFailure::Refused(SendFailure::Forbidden))
	);
	assert!(refused.messages.is_empty());
	assert!(!refused.has_more());
	running.session().send(Command::SearchMore);
	tokio::time::sleep(QUIET).await;
	assert_eq!(searches(&running).len(), 1, "a refusal is never retried");

	running.session().send(Command::CloseSearch);
	running
		.eventually("the search to close", || results(&running).is_none())
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dm_search_uses_the_channel_route() {
	let running = Running::start_with(Options::new("search-dm").rest(tuning())).await;
	running.connected().await;
	let channel_id = Snowflake(running.fixtures().private_channel_ids[0]);
	running.session().send(Command::Search {
		guild_id: None,
		channel_id: Some(channel_id),
		query: SearchQuery::default(),
	});
	let found = settled(&running, "the DM search").await;
	assert_eq!(found.status, SearchStatus::Ready);
	assert!(
		found
			.messages
			.iter()
			.all(|message| message.channel_id == channel_id)
	);
	let wire: Vec<RestRequest> = running
		.mock()
		.recording()
		.requests_to("GET", "/channels/*/messages/search")
		.cloned()
		.collect();
	assert_eq!(wire.len(), 1);
	assert!(wire[0].path.contains(&channel_id.to_string()));
	assert!(param(&params(&wire[0]), "channel_id").is_empty());
	running.stop().await;
}
