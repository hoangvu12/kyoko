use bridge::{
	Leading, MAX_RESULTS, QuickSwitcher, RecentTargets, SwitcherCategory, SwitcherEntry, SwitcherTarget,
	fuzzy_score, switcher_entries,
};
use protocol::{ChannelType, Snowflake};

use crate::harness::Running;

fn channel(id: u64, name: &str, category: SwitcherCategory) -> SwitcherEntry {
	SwitcherEntry {
		target: SwitcherTarget::Channel {
			guild_id: Some(Snowflake(1)),
			channel_id: Snowflake(id),
		},
		name: name.to_owned(),
		context: Some("Kyoko".to_owned()),
		leading: Leading::Icon("hash"),
		category,
	}
}

fn dm(id: u64, name: &str) -> SwitcherEntry {
	SwitcherEntry {
		target: SwitcherTarget::Channel {
			guild_id: None,
			channel_id: Snowflake(id),
		},
		name: name.to_owned(),
		context: None,
		leading: Leading::Icon("at_sign"),
		category: SwitcherCategory::DirectMessages,
	}
}

fn server(id: u64, name: &str) -> SwitcherEntry {
	SwitcherEntry {
		target: SwitcherTarget::Guild(Snowflake(id)),
		name: name.to_owned(),
		context: None,
		leading: Leading::Icon("home"),
		category: SwitcherCategory::Servers,
	}
}

fn sample() -> Vec<SwitcherEntry> {
	vec![
		channel(10, "general", SwitcherCategory::Channels),
		channel(11, "green-energy", SwitcherCategory::Channels),
		channel(12, "off-topic-general", SwitcherCategory::Channels),
		channel(13, "Lounge", SwitcherCategory::Voice),
		dm(20, "grace"),
		server(30, "Gen Z Gamers"),
	]
}

fn names(switcher: &QuickSwitcher) -> Vec<(String, SwitcherCategory)> {
	switcher
		.results()
		.iter()
		.map(|shown| (switcher.entry(shown).name.clone(), shown.category))
		.collect()
}

fn searching(query: &str, recent: &RecentTargets) -> QuickSwitcher {
	let mut switcher = QuickSwitcher::new(sample(), recent);
	switcher.set_query(query);
	switcher
}

#[test]
fn word_starts_and_runs_of_letters_rank_above_scattered_matches() {
	let query: Vec<char> = "gen".chars().collect();
	let general = fuzzy_score(&query, "general").expect("matches");
	let inside = fuzzy_score(&query, "off-topic-general").expect("matches");
	let scattered = fuzzy_score(&query, "green-energy").expect("matches");
	assert!(general > inside, "a leading match beats a later word start");
	assert!(inside > scattered, "a word start run beats letters spread out");
	assert_eq!(fuzzy_score(&query, "lounge"), None);
	assert!(
		fuzzy_score(&['G', 'E', 'N'], "general").is_some(),
		"matching ignores case"
	);
}

#[test]
fn results_are_grouped_by_category_in_score_order_within_each() {
	let switcher = searching("gen", &RecentTargets::default());
	assert_eq!(
		names(&switcher),
		[
			("general".to_owned(), SwitcherCategory::Channels),
			("off-topic-general".to_owned(), SwitcherCategory::Channels),
			("green-energy".to_owned(), SwitcherCategory::Channels),
			("Gen Z Gamers".to_owned(), SwitcherCategory::Servers),
		]
	);
}

#[test]
fn a_recent_target_breaks_a_tie() {
	let mut recent = RecentTargets::default();
	let lounge = sample()[3].target;
	recent.remember(lounge);
	let switcher = searching("!", &recent);
	assert_eq!(names(&switcher), [("Lounge".to_owned(), SwitcherCategory::Voice)]);
	let mut recent = RecentTargets::default();
	recent.remember(sample()[2].target);
	let switcher = searching("#", &recent);
	assert_eq!(
		switcher.entry(&switcher.results()[0]).name,
		"off-topic-general",
		"with equal scores the most recent comes first"
	);
}

#[test]
fn a_prefix_narrows_to_its_kind() {
	let recent = RecentTargets::default();
	let only = |query: &str| -> Vec<SwitcherCategory> {
		searching(query, &recent)
			.results()
			.iter()
			.map(|shown| shown.category)
			.collect()
	};
	assert!(
		only("#g")
			.iter()
			.all(|category| *category == SwitcherCategory::Channels)
	);
	assert_eq!(only("!l"), [SwitcherCategory::Voice]);
	assert_eq!(only("@gr"), [SwitcherCategory::DirectMessages]);
	assert_eq!(only("*gen"), [SwitcherCategory::Servers]);
	assert!(only("#zzz").is_empty());
}

#[test]
fn an_empty_query_shows_recent_places_first_then_the_rest() {
	let mut recent = RecentTargets::default();
	recent.remember(sample()[4].target);
	recent.remember(sample()[1].target);
	let switcher = QuickSwitcher::new(sample(), &recent);
	let shown = names(&switcher);
	assert_eq!(
		shown[..2],
		[
			("green-energy".to_owned(), SwitcherCategory::Recent),
			("grace".to_owned(), SwitcherCategory::Recent),
		]
	);
	assert_eq!(shown.len(), sample().len());
	let categories: Vec<SwitcherCategory> = shown[2..].iter().map(|(_, category)| *category).collect();
	assert_eq!(
		categories,
		[
			SwitcherCategory::Channels,
			SwitcherCategory::Channels,
			SwitcherCategory::Voice,
			SwitcherCategory::Servers,
		]
	);
}

#[test]
fn results_stop_at_nine_and_the_selection_wraps() {
	let entries: Vec<SwitcherEntry> = (0..20)
		.map(|index| channel(100 + index, &format!("chan-{index}"), SwitcherCategory::Channels))
		.collect();
	let mut switcher = QuickSwitcher::new(entries, &RecentTargets::default());
	switcher.set_query("chan");
	assert_eq!(switcher.results().len(), MAX_RESULTS);
	switcher.step(false);
	assert_eq!(switcher.selected(), MAX_RESULTS - 1);
	switcher.step(true);
	assert_eq!(switcher.selected(), 0);
	switcher.step(true);
	switcher.set_query("chan-1");
	assert_eq!(switcher.selected(), 0, "typing resets the selection");
	assert_eq!(
		switcher.picked(),
		Some(SwitcherTarget::Channel {
			guild_id: Some(Snowflake(1)),
			channel_id: Snowflake(101),
		})
	);
}

#[test]
fn remembering_keeps_the_newest_eight_without_repeats() {
	let mut recent = RecentTargets::default();
	for id in 0..10 {
		recent.remember(SwitcherTarget::Guild(Snowflake(id)));
	}
	recent.remember(SwitcherTarget::Guild(Snowflake(5)));
	let ids: Vec<SwitcherTarget> = recent.targets().to_vec();
	assert_eq!(ids.len(), bridge::RECENT_TARGETS);
	assert_eq!(ids[0], SwitcherTarget::Guild(Snowflake(5)));
	assert_eq!(ids.iter().filter(|target| **target == ids[0]).count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_server_channel_and_conversation_can_be_reached_by_name() {
	let running = Running::start("switcher-entries").await;
	{
		let state = running.session().state().read();
		let entries = switcher_entries(&state);
		for guild in state.guild_summaries() {
			assert!(
				entries
					.iter()
					.any(|entry| entry.target == SwitcherTarget::Guild(guild.id))
			);
			for row in state
				.guild_channel_tree(guild.id)
				.into_iter()
				.flat_map(|node| node.children)
			{
				let listed = entries.iter().any(|entry| {
					entry.target
						== SwitcherTarget::Channel {
							guild_id: Some(guild.id),
							channel_id: row.id,
						}
				});
				let reachable =
					!row.locked && !matches!(row.kind, ChannelType::GuildForum | ChannelType::GuildMedia);
				assert_eq!(listed, reachable, "{} ({:?})", row.name, row.kind);
			}
		}
		for row in state.dm_rows() {
			let entry = entries
				.iter()
				.find(|entry| {
					entry.target
						== SwitcherTarget::Channel {
							guild_id: None,
							channel_id: row.channel_id,
						}
				})
				.expect("the conversation is listed");
			assert_eq!(entry.name, row.name);
			let mut switcher = QuickSwitcher::new(entries.clone(), &RecentTargets::default());
			switcher.set_query(&format!("@{}", row.name));
			assert!(
				switcher
					.results()
					.iter()
					.any(|shown| switcher.entry(shown).target == entry.target),
				"typing the name finds {}",
				row.name
			);
		}
		let voice = entries
			.iter()
			.filter(|entry| entry.category == SwitcherCategory::Voice)
			.count();
		assert!(voice > 0, "voice channels are listed apart");
	}
	running.stop().await;
}
