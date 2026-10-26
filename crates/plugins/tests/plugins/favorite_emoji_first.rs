use std::time::Duration;

use plugins::{EnableError, Limits, Permission, Status};
use session::Command;
use state::{EmojiCandidate, State};

use crate::support::{Lab, approve, scratch, start, write_plugin};

const ID: &str = "favorite-emoji-first";
const QUIET: Duration = Duration::from_millis(500);

const REVERSE: &str = r#"
return {
	rank_emoji = function(request)
		local order = {}
		for index = #request.emojis, 1, -1 do
			table.insert(order, request.emojis[index].key)
		end
		return order
	end,
}
"#;

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

fn wave(lab: &Lab) -> EmojiCandidate {
	let id = lab.running.fixtures().guilds[0]
		.emojis
		.iter()
		.find(|emoji| emoji.name.as_deref() == Some("blob_wave"))
		.and_then(|emoji| emoji.id)
		.expect("the first mock server has blob_wave");
	EmojiCandidate {
		key: id.to_string(),
		name: "blob_wave".to_owned(),
		id: Some(id),
		animated: false,
		guild_id: None,
		unicode: None,
	}
}

fn candidates(lab: &Lab) -> Vec<EmojiCandidate> {
	vec![
		unicode("smile", "😄"),
		unicode("heart", "❤️"),
		unicode("smirk", "😏"),
		wave(lab),
	]
}

fn ask(lab: &Lab, query: &str) {
	lab.running.session().send(Command::RankEmoji {
		guild_id: None,
		query: query.to_owned(),
		candidates: candidates(lab),
	});
}

async fn ready(lab: &Lab) {
	lab.running.connected().await;
	lab.running.session().send(Command::RankEmoji {
		guild_id: None,
		query: "warm".to_owned(),
		candidates: Vec::new(),
	});
	lab.running
		.until("the favourites", |state: &State| state.favorites_loaded())
		.await;
}

async fn ranked(lab: &Lab, query: &str) -> Vec<String> {
	ask(lab, query);
	lab.running
		.until("the ranking", |state: &State| {
			state.emoji_ranking(None, query).is_some()
		})
		.await;
	lab.running
		.session()
		.state()
		.read()
		.emoji_ranking(None, query)
		.map(<[String]>::to_vec)
		.unwrap_or_default()
}

async fn unranked(lab: &Lab, query: &str) -> bool {
	ask(lab, query);
	tokio::time::sleep(QUIET).await;
	lab.running
		.session()
		.state()
		.read()
		.emoji_ranking(None, query)
		.is_none()
}

fn enable(lab: &Lab) {
	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let Err(EnableError::NeedsApproval(asked)) = lab.host.enable(ID) else {
		panic!("enabling asks for approval first");
	};
	assert_eq!(asked, vec![Permission::parse("hook:emoji_rank")]);
	lab.host.approve(ID, &asked).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn favourite_emoji_rank_first_in_their_favourite_order() {
	let lab = start("fef-order", scratch("fef-order"), Limits::default(), true).await;
	ready(&lab).await;
	enable(&lab);
	let wave = wave(&lab).key;
	assert_eq!(
		ranked(&lab, "order").await,
		vec![wave, "heart".to_owned(), "smile".to_owned(), "smirk".to_owned()]
	);
	lab.running.mock().update_favorite_emojis(&["smirk", "smile"]);
	lab.running
		.until("the new favourites", |state: &State| {
			state.favorite_emojis().first().map(String::as_str) == Some("smirk")
		})
		.await;
	assert_eq!(
		ranked(&lab, "again").await[..2],
		["smirk".to_owned(), "smile".to_owned()]
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_ranking_hook_runs_only_with_its_permission() {
	let root = scratch("fef-permission");
	write_plugin(&root, "unpermitted", &[], "", REVERSE);
	let lab = start("fef-permission", root, Limits::default(), false).await;
	ready(&lab).await;
	approve(&lab.host, "unpermitted");
	assert_eq!(lab.host.status("unpermitted"), Some(Status::Running));
	assert!(unranked(&lab, "denied").await, "no hook:emoji_rank, no ranking");
	write_plugin(&lab.root, "permitted", &["hook:emoji_rank"], "", REVERSE);
	lab.host.scan();
	approve(&lab.host, "permitted");
	let wave = wave(&lab).key;
	assert_eq!(
		ranked(&lab, "allowed").await,
		vec![wave, "smirk".to_owned(), "heart".to_owned(), "smile".to_owned()]
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_or_slow_ranking_leaves_kyokos_order() {
	let root = scratch("fef-faults");
	write_plugin(
		&root,
		"broken",
		&["hook:emoji_rank"],
		"",
		"return { rank_emoji = function(request) error(\"no ranking today\") end }",
	);
	write_plugin(
		&root,
		"slow",
		&["hook:emoji_rank"],
		"",
		"return { rank_emoji = function(request) while true do end end }",
	);
	let lab = start("fef-faults", root, Limits::default(), true).await;
	ready(&lab).await;
	approve(&lab.host, "broken");
	approve(&lab.host, "slow");
	assert!(unranked(&lab, "faults").await, "kyoko's own order stays");
	enable(&lab);
	let wave = wave(&lab).key;
	assert_eq!(
		ranked(&lab, "chained").await[..2],
		[wave, "heart".to_owned()],
		"the working plugin still ranks beside the failing ones"
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_is_ranked_while_the_plugin_is_off() {
	let lab = start("fef-off", scratch("fef-off"), Limits::default(), true).await;
	ready(&lab).await;
	assert!(unranked(&lab, "off").await);
	enable(&lab);
	assert!(!ranked(&lab, "on").await.is_empty());
	lab.host.disable(ID);
	assert!(unranked(&lab, "off-again").await);
	let recording = lab.running.mock().recording();
	assert_eq!(
		recording
			.requests_to("GET", "/users/@me/settings-proto/2")
			.count(),
		1,
		"favourites are read once whether or not the plugin runs"
	);
	lab.stop().await;
}
