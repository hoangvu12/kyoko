use std::time::Instant;

use bridge::{DmStatus, MemberEntry, MemberListEntry, MemberPanel, MemberRowKind, OpenMembers};
use mockcord::MockConfig;
use protocol::{Opcode, Snowflake};
use serde_json::{Value, json};
use session::Command;

use crate::harness::Running;

const LARGE_GUILD: usize = 300;
const BURST: usize = 24;
const PUBLISH_EVERY: std::time::Duration = std::time::Duration::from_millis(150);
const BURST_SPACING: std::time::Duration = std::time::Duration::from_millis(20);

async fn opened(name: &str, config: MockConfig) -> (Running, OpenMembers) {
	let mut running = Running::start_with(name, config).await;
	let open = {
		let state = running.session().state().clone();
		bridge::first_channel(&state.read()).expect("a channel to open")
	};
	let guild_id = open.guild_id.expect("the first channel is in a guild");
	let select = running.select_guild(guild_id);
	if let Some(command) = select {
		running.session().send(command);
	}
	let command = running.open_channel(open.channel_id).expect("the channel opens");
	running.session().send(command);
	let members = OpenMembers {
		guild_id,
		channel_id: open.channel_id,
	};
	running.view.members.show(Some(members), Instant::now());
	running
		.until("the member list to load", |view| loaded(&view.members))
		.await;
	(running, members)
}

fn loaded(panel: &MemberPanel) -> bool {
	panel.members().next().is_some() && !panel.entries().contains(&MemberListEntry::Placeholder)
}

fn member(panel: &MemberPanel, user_id: Snowflake) -> Option<MemberEntry> {
	panel.members().find(|member| member.user_id == user_id).cloned()
}

fn groups(panel: &MemberPanel) -> Vec<(usize, String, u32)> {
	panel
		.entries()
		.iter()
		.enumerate()
		.filter_map(|(offset, entry)| match entry {
			MemberListEntry::Group(group) => Some((panel.first() + offset, group.title(), group.count)),
			_ => None,
		})
		.collect()
}

fn viewport_ranges(running: &Running, guild_id: Snowflake) -> Vec<Value> {
	let key = guild_id.to_string();
	running.server.control().inspect(|recording| {
		recording
			.frames_with(Opcode::GuildSubscriptionsBulk)
			.filter_map(|frame| {
				frame.data["subscriptions"][key.as_str()]["channels"]
					.as_object()
					.cloned()
			})
			.flat_map(|channels| channels.into_values())
			.collect()
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn members_are_grouped_under_headers_with_their_counts() {
	let (running, open) = opened("members-groups", MockConfig::default()).await;
	let panel = &running.view.members;
	let groups = groups(panel);
	let layout = panel.layout();
	assert_eq!(groups[0].0, 0);
	let online = groups[0].2 as usize;
	assert_eq!(groups[0].1, format!("ONLINE — {online}"));
	let headers: Vec<usize> = (0..layout.len())
		.filter(|index| layout[*index] == MemberRowKind::Group)
		.collect();
	assert_eq!(headers, vec![0, online + 1]);
	let offline = layout.len() - online - 2;
	let fixture = running
		.server
		.fixtures()
		.guild(open.guild_id.0)
		.expect("the guild is a fixture");
	assert_eq!(online + offline, fixture.members.len());
	for (offset, entry) in panel.entries().iter().enumerate() {
		if let MemberListEntry::Member(member) = entry {
			let online_row = panel.first() + offset <= online;
			assert_eq!(member.status != DmStatus::Offline, online_row, "{member:?}");
			assert!(!member.name.is_empty());
		}
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scrolling_requests_a_later_range_and_fills_its_placeholders() {
	let config = MockConfig {
		members_per_guild: LARGE_GUILD,
		..MockConfig::default()
	};
	let (mut running, open) = opened("members-scroll", config).await;
	let rows = running.view.members.layout().len();
	assert_eq!(rows, LARGE_GUILD + 2);
	let heights = bridge::MemberHeights::default();
	let target = 250usize;
	let offset = heights.group + (target - 1) as f64 * heights.member;
	let command = running
		.view
		.members
		.scroll(offset, 15. * heights.member, Instant::now())
		.expect("a new viewport is reported");
	let Command::SetMemberViewport {
		guild_id,
		channel_id,
		first,
		last,
	} = command
	else {
		panic!("expected a member viewport, got {command:?}");
	};
	assert_eq!((guild_id, channel_id), (open.guild_id, open.channel_id));
	assert!(
		(first as usize).abs_diff(target) <= 2,
		"first visible row {first}"
	);
	assert!(last > first);
	assert!(
		running
			.view
			.members
			.scroll(offset, 15. * heights.member, Instant::now())
			.is_none(),
		"an unchanged viewport is not reported again"
	);
	running.flush_members();
	while running.view.members.due().is_some() {
		tokio::time::sleep(bridge::MEMBER_PUBLISH_GAP).await;
		running.flush_members();
	}
	let members = &running.view.members;
	assert!(members.first() > 100);
	assert!(
		members
			.entries()
			.iter()
			.all(|entry| matches!(entry, MemberListEntry::Placeholder | MemberListEntry::Group(_))),
		"rows beyond the first range are placeholders before it arrives"
	);
	running.session().send(command);
	running
		.until("the later range to fill in", |view| loaded(&view.members))
		.await;
	assert!(running.view.members.first() > 100);
	let ranges = viewport_ranges(&running, open.guild_id);
	assert!(
		ranges.iter().any(|range| range
			.as_array()
			.is_some_and(|ranges| ranges.iter().any(|range| range == &json!([200, 299])))),
		"the bucket under the viewport was subscribed: {ranges:?}"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_presence_change_reaches_the_visible_row() {
	let (mut running, open) = opened("members-presence", MockConfig::default()).await;
	let user_id = running
		.view
		.members
		.members()
		.find(|member| member.status != DmStatus::Offline)
		.map(|member| member.user_id)
		.expect("an online member is visible");
	running.server.control().dispatch(
		"PRESENCE_UPDATE",
		&json!({
			"user": { "id": user_id.to_string() },
			"guild_id": open.guild_id.to_string(),
			"status": "idle",
			"activities": [{ "name": "Chess", "type": 0 }],
		}),
	);
	running
		.until("the presence to reach the row", |view| {
			member(&view.members, user_id).is_some_and(|member| {
				member.status == DmStatus::Idle && member.activity.as_deref() == Some("Playing Chess")
			})
		})
		.await;
	running.server.control().dispatch(
		"PRESENCE_UPDATE",
		&json!({
			"user": { "id": user_id.to_string() },
			"guild_id": open.guild_id.to_string(),
			"status": "dnd",
			"activities": [{ "name": "Custom Status", "type": 4, "state": "deep work" }],
		}),
	);
	running
		.until("the custom status to reach the row", |view| {
			member(&view.members, user_id).is_some_and(|member| {
				member.status == DmStatus::Dnd && member.activity.as_deref() == Some("deep work")
			})
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_burst_of_list_updates_is_published_a_few_times() {
	let (mut running, open) = opened("members-burst", MockConfig::default()).await;
	let target = running
		.view
		.members
		.members()
		.next()
		.cloned()
		.expect("a visible member");
	let index = running
		.view
		.members
		.entries()
		.iter()
		.position(
			|entry| matches!(entry, MemberListEntry::Member(member) if member.user_id == target.user_id),
		)
		.expect("the member is in the window")
		+ running.view.members.first();
	running.view.member_publishes = 0;
	let started = Instant::now();
	let last_nick = format!("burst {}", BURST - 1);
	let control = running.server.control();
	let user_id = target.user_id;
	let guild_id = open.guild_id;
	let burst = tokio::spawn(async move {
		for step in 0..BURST {
			control.dispatch(
				"GUILD_MEMBER_LIST_UPDATE",
				&json!({
					"guild_id": guild_id.to_string(),
					"id": "everyone",
					"groups": [],
					"ops": [{
						"op": "UPDATE",
						"index": index,
						"item": { "member": {
							"user": { "id": user_id.to_string(), "username": "burst" },
							"roles": [],
							"nick": format!("burst {step}"),
							"presence": {
								"user": { "id": user_id.to_string() },
								"status": "online",
								"activities": [],
							},
						}},
					}],
				}),
			);
			tokio::time::sleep(BURST_SPACING).await;
		}
	});
	running
		.until("the last update to be shown", |view| {
			member(&view.members, target.user_id).is_some_and(|member| member.name == last_nick)
		})
		.await;
	burst.await.expect("the burst is sent");
	let elapsed = started.elapsed();
	let allowed = 2 + (elapsed.as_millis() / PUBLISH_EVERY.as_millis()) as usize;
	assert!(
		running.view.member_publishes <= allowed,
		"{} publishes for {BURST} updates in {elapsed:?}",
		running.view.member_publishes
	);
	running.stop().await;
}
