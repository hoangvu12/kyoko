use bridge::{DmStatus, FriendKind, FriendSection, FriendsTab, FriendsView};
use protocol::Snowflake;
use serde_json::json;
use session::Command;

use crate::harness::Running;

async fn listed(name: &str) -> Running {
	let mut running = Running::start(name).await;
	running
		.until("the friends and their presences", |view| {
			view.friends
				.everyone()
				.any(|entry| entry.status != DmStatus::Offline)
		})
		.await;
	running
}

fn titles(sections: &[FriendSection]) -> Vec<&str> {
	sections.iter().map(|section| section.title.as_str()).collect()
}

fn ids(section: &FriendSection) -> Vec<Snowflake> {
	section.entries.iter().map(|entry| entry.id).collect()
}

fn offline_friend(view: &FriendsView) -> Snowflake {
	let all = view.sections(FriendsTab::All, "");
	all[0]
		.entries
		.iter()
		.find(|entry| entry.status == DmStatus::Offline)
		.map(|entry| entry.id)
		.expect("an offline friend in the fixtures")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_tab_lists_its_relationships_with_counts() {
	let running = listed("friends-tabs").await;
	let view = &running.view.friends;
	let (friends, incoming) = {
		let state = running.session().state().read();
		(state.friends(), state.pending_incoming()[0].user_id)
	};
	let online = friends
		.iter()
		.filter(|row| DmStatus::from_kind(&row.status) != DmStatus::Offline)
		.count();
	assert!(
		online > 0 && online < friends.len(),
		"the fixtures mix online and offline friends"
	);

	let tab = view.sections(FriendsTab::Online, "");
	assert_eq!(titles(&tab), [format!("ONLINE — {online}")]);
	assert!(
		tab[0]
			.entries
			.iter()
			.all(|entry| entry.status != DmStatus::Offline)
	);
	assert!(
		tab[0]
			.entries
			.iter()
			.all(|entry| entry.kind == FriendKind::Friend)
	);

	let tab = view.sections(FriendsTab::All, "");
	assert_eq!(titles(&tab), [format!("ALL FRIENDS — {}", friends.len())]);
	let ranks: Vec<bool> = tab[0]
		.entries
		.iter()
		.map(|entry| entry.status == DmStatus::Offline)
		.collect();
	assert!(
		ranks.windows(2).all(|pair| pair[0] <= pair[1]),
		"online friends come first"
	);

	let tab = view.sections(FriendsTab::Pending, "");
	assert_eq!(titles(&tab), ["INCOMING — 1", "OUTGOING — 1"]);
	assert_eq!(ids(&tab[0]), [incoming]);
	assert_eq!(tab[0].entries[0].subtitle(), "Incoming Friend Request");
	assert_eq!(tab[1].entries[0].subtitle(), "Outgoing Friend Request");
	assert_eq!(view.pending_count(), 1);

	let tab = view.sections(FriendsTab::Blocked, "");
	assert_eq!(titles(&tab), ["BLOCKED — 1"]);
	assert_eq!(tab[0].entries[0].subtitle(), "Blocked");
	assert!(view.sections(FriendsTab::Add, "").is_empty());

	let someone = &friends[friends.len() - 1];
	let filtered = view.sections(
		FriendsTab::All,
		&format!("  {}  ", someone.username.to_uppercase()),
	);
	assert_eq!(ids(&filtered[0]), [someone.user_id]);
	assert!(
		view.sections(FriendsTab::Pending, "nobody by this name")
			.is_empty()
	);
	assert_eq!(
		FriendsTab::Pending.empty_text(),
		"There are no pending friend requests."
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn presence_and_accepted_requests_move_people_between_tabs() {
	let mut running = listed("friends-moves").await;
	let friend = offline_friend(&running.view.friends);
	let before = running.view.friends.active_now().len();
	running.server.control().dispatch(
		"PRESENCE_UPDATE",
		&json!({
			"user": { "id": friend.to_string() },
			"status": "dnd",
			"client_status": { "desktop": "dnd" },
			"activities": [{ "name": "Hades II", "type": 0 }],
		}),
	);
	running
		.until("the friend to come online", |view| {
			view.friends.sections(FriendsTab::Online, "")[0]
				.entries
				.iter()
				.any(|entry| entry.id == friend)
		})
		.await;
	let entry = running.view.friends.entry(friend).expect("the friend").clone();
	assert_eq!(entry.status, DmStatus::Dnd);
	assert_eq!(entry.subtitle(), "Playing Hades II");
	assert_eq!(running.view.friends.active_now().len(), before + 1);

	let incoming = running.view.friends.sections(FriendsTab::Pending, "")[0].entries[0].id;
	running
		.session()
		.send(Command::AcceptFriend { user_id: incoming });
	running
		.until("the request to become a friend", |view| {
			view.friends.pending_count() == 0
				&& view.friends.entry(incoming).map(|entry| entry.kind) == Some(FriendKind::Friend)
		})
		.await;
	let pending = running.view.friends.sections(FriendsTab::Pending, "");
	assert_eq!(titles(&pending), ["OUTGOING — 1"]);
	running.stop().await;
}
