use std::time::{Duration, Instant};

use mockcord::{Recording, Reply};
use protocol::{Permissions, Snowflake};
use serde_json::{Value, json};
use session::{Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const GAP: Duration = Duration::from_millis(400);
const SLACK: Duration = Duration::from_millis(50);
const REASON: &str = "Spam links, again: 100% sure";
const ENCODED_REASON: &str = "Spam%20links%2C%20again%3A%20100%25%20sure";
const EXTRA_BANS: u64 = 1005;
const EXTRA_AUDIT: usize = 60;

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		guild_write_gap: GAP,
		..RestTuning::default()
	}
}

struct Guild {
	id: Snowflake,
	me: Snowflake,
	owner: Snowflake,
	targets: Vec<Snowflake>,
	role: Snowflake,
}

async fn start(name: &'static str) -> (Running, Guild) {
	let running = Running::start_with(Options::new(name).rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	let user_ids: Vec<Snowflake> = running.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.collect();
	running
		.until("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	running
		.session()
		.send(Command::RequestMembers { guild_id, user_ids });
	running
		.until("the first guild with its members", |state| {
			state
				.guild(guild_id)
				.is_some_and(|guild| guild.members.len() > 3 && guild.owner_id.is_some())
		})
		.await;
	let guild = {
		let state = running.session().state().read();
		let me = state.current_user_id().expect("signed in");
		let found = state.guild(guild_id).expect("the guild is loaded");
		let owner = found.owner_id.expect("the guild has an owner");
		let mut targets: Vec<Snowflake> = found
			.members
			.iter()
			.map(|(id, _)| *id)
			.filter(|id| *id != me && *id != owner && state.outranks(guild_id, me, *id))
			.collect();
		targets.sort();
		let role = found
			.roles
			.values()
			.find(|role| role.name == "Members")
			.map(|role| role.id)
			.expect("the guild has a Members role");
		Guild {
			id: guild_id,
			me,
			owner,
			targets,
			role,
		}
	};
	assert!(guild.targets.len() >= 3, "three members below the current user");
	(running, guild)
}

fn member_path(guild: &Guild, user_id: Snowflake) -> String {
	format!("/guilds/{}/members/{user_id}", guild.id)
}

fn requests(
	recording: &Recording,
	method: &str,
	path: &str,
) -> Vec<(Option<Value>, Option<String>, Instant)> {
	recording
		.requests_to(method, path)
		.map(|request| {
			(
				request.json(),
				request.header("x-audit-log-reason").map(str::to_owned),
				request.at,
			)
		})
		.collect()
}

fn queries(recording: &Recording, path: &str) -> Vec<String> {
	recording
		.requests_to("GET", path)
		.map(|request| request.query.clone().unwrap_or_default())
		.collect()
}

async fn strip_my_roles(running: &Running, guild: &Guild) {
	let user = running.fixtures().current_user.clone();
	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({ "guild_id": guild.id.to_string(), "user": user, "roles": [], "joined_at": "2025-01-01T00:00:00+00:00" }),
	);
	running
		.until("the current user without roles", |state: &State| {
			!state
				.guild_permissions(guild.id, guild.me)
				.contains(Permissions::KICK_MEMBERS)
		})
		.await;
}

async fn quiet() {
	tokio::time::sleep(GAP * 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn moderation_without_the_permission_or_the_rank_never_reaches_rest() {
	let (running, guild) = start("moderation-refused").await;
	running.session().send(Command::KickMember {
		guild_id: guild.id,
		user_id: guild.owner,
		reason: None,
	});
	running
		.until("the refusal to kick the owner", |state: &State| {
			state.last_rejection().is_some()
		})
		.await;
	strip_my_roles(&running, &guild).await;
	let target = guild.targets[0];
	running.session().send(Command::KickMember {
		guild_id: guild.id,
		user_id: target,
		reason: None,
	});
	running.session().send(Command::BanMember {
		guild_id: guild.id,
		user_id: target,
		delete_message_seconds: 0,
		reason: None,
	});
	running.session().send(Command::EditRole {
		guild_id: guild.id,
		role_id: guild.role,
		name: Some("renamed".to_owned()),
		color: None,
		permissions: None,
		hoist: None,
		mentionable: None,
	});
	running
		.until("the refusals", |state: &State| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.reason.contains("Manage Roles"))
		})
		.await;
	quiet().await;
	let recording = running.mock().recording();
	assert!(requests(&recording, "DELETE", &member_path(&guild, guild.owner)).is_empty());
	assert!(requests(&recording, "DELETE", &member_path(&guild, target)).is_empty());
	assert!(requests(&recording, "PUT", &format!("/guilds/{}/bans/{target}", guild.id)).is_empty());
	assert!(
		requests(
			&recording,
			"PATCH",
			&format!("/guilds/{}/roles/{}", guild.id, guild.role)
		)
		.is_empty()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kicks_bans_and_timeouts_carry_the_audit_log_reason() {
	let (running, guild) = start("moderation-reason").await;
	let (kicked, banned) = (guild.targets[0], guild.targets[1]);
	running.session().send(Command::KickMember {
		guild_id: guild.id,
		user_id: kicked,
		reason: Some(REASON.to_owned()),
	});
	running.session().send(Command::BanMember {
		guild_id: guild.id,
		user_id: banned,
		delete_message_seconds: 3600,
		reason: Some(REASON.to_owned()),
	});
	let (_, kick_reason, _) = running
		.until_wire("the kick", |recording| {
			requests(recording, "DELETE", &member_path(&guild, kicked)).pop()
		})
		.await;
	assert_eq!(kick_reason.as_deref(), Some(ENCODED_REASON));
	let (body, ban_reason, _) = running
		.until_wire("the ban", |recording| {
			requests(recording, "PUT", &format!("/guilds/{}/bans/{banned}", guild.id)).pop()
		})
		.await;
	assert_eq!(ban_reason.as_deref(), Some(ENCODED_REASON));
	assert_eq!(body, Some(json!({ "delete_message_seconds": 3600 })));
	let timed = guild.targets[2];
	running.session().send(Command::TimeoutMember {
		guild_id: guild.id,
		user_id: timed,
		until: Some(chrono::Utc::now() + chrono::TimeDelta::minutes(10)),
		reason: Some(REASON.to_owned()),
	});
	let (body, timeout_reason, _) = running
		.until_wire("the timeout", |recording| {
			requests(recording, "PATCH", &member_path(&guild, timed)).pop()
		})
		.await;
	assert_eq!(timeout_reason.as_deref(), Some(ENCODED_REASON));
	assert!(
		body.as_ref()
			.is_some_and(|body| body["communication_disabled_until"].is_string())
	);
	running
		.until("the kicked member to leave", |state: &State| {
			state
				.guild(guild.id)
				.is_some_and(|found| found.members.get(&kicked).is_none())
		})
		.await;
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
		user_id: None,
	});
	running
		.until("the kick in the audit log", |state: &State| {
			state.audit_log(guild.id).is_some_and(|log| {
				log.entries
					.iter()
					.any(|entry| entry.target_id == Some(kicked) && entry.reason.as_deref() == Some(REASON))
			})
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writes_in_a_guild_go_one_at_a_time_and_edits_of_one_role_coalesce() {
	let (running, guild) = start("moderation-paced").await;
	let edit = |name: Option<&str>, color: Option<u32>, hoist: Option<bool>| Command::EditRole {
		guild_id: guild.id,
		role_id: guild.role,
		name: name.map(str::to_owned),
		color,
		permissions: None,
		hoist,
		mentionable: None,
	};
	let role_path = format!("/guilds/{}/roles/{}", guild.id, guild.role);
	running.session().send(edit(Some("first"), None, None));
	running
		.until_wire("the first role write", |recording| {
			requests(recording, "PATCH", &role_path).pop()
		})
		.await;
	running.session().send(edit(None, Some(0x00ff_0000), None));
	running.session().send(edit(Some("second"), None, Some(true)));
	running
		.until("the local edits", |state: &State| {
			state
				.guild(guild.id)
				.and_then(|found| found.roles.get(&guild.role))
				.is_some_and(|role| role.name == "second" && role.hoist && role.color == 0x00ff_0000)
		})
		.await;
	let writes = running
		.until_wire("both role writes", |recording| {
			let writes = requests(recording, "PATCH", &role_path);
			(writes.len() >= 2).then_some(writes)
		})
		.await;
	assert_eq!(writes[0].0, Some(json!({ "name": "first" })));
	assert_eq!(
		writes[1].0,
		Some(json!({ "name": "second", "color": 0x00ff_0000, "hoist": true }))
	);
	assert!(writes[1].2.duration_since(writes[0].2) >= GAP - SLACK);
	let (first, second) = (guild.targets[0], guild.targets[1]);
	for user_id in [first, second] {
		running.session().send(Command::KickMember {
			guild_id: guild.id,
			user_id,
			reason: None,
		});
	}
	let kicks = running
		.until_wire("both kicks", |recording| {
			let first = requests(recording, "DELETE", &member_path(&guild, first)).pop()?;
			let second = requests(recording, "DELETE", &member_path(&guild, second)).pop()?;
			Some((first.2, second.2))
		})
		.await;
	assert!(kicks.1.duration_since(kicks.0) >= GAP - SLACK);
	quiet().await;
	assert_eq!(
		requests(&running.mock().recording(), "PATCH", &role_path).len(),
		2
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refused_edits_roll_back_and_say_why() {
	let (running, guild) = start("moderation-rollback").await;
	let role_path = format!("/guilds/{}/roles/{}", guild.id, guild.role);
	let target = guild.targets[0];
	let nickname = |state: &State| {
		state
			.guild(guild.id)
			.and_then(|found| found.members.get(&target))
			.and_then(|member| member.nick.clone())
	};
	let before = nickname(&running.session().state().read());
	running.mock().respond_once(
		"PATCH",
		&role_path,
		Reply::json(403, json!({ "code": 50013, "message": "Missing Permissions" })),
	);
	running.mock().respond_once(
		"PATCH",
		&member_path(&guild, target),
		Reply::json(400, json!({ "code": 50035, "message": "Invalid Form Body" })),
	);
	running.session().send(Command::EditRole {
		guild_id: guild.id,
		role_id: guild.role,
		name: Some("refused".to_owned()),
		color: None,
		permissions: None,
		hoist: None,
		mentionable: None,
	});
	running.session().send(Command::SetNickname {
		guild_id: guild.id,
		user_id: target,
		nick: Some("refused nick".to_owned()),
	});
	running
		.until_wire("both refused writes", |recording| {
			let role = requests(recording, "PATCH", &role_path).pop()?;
			let member = requests(recording, "PATCH", &member_path(&guild, target)).pop()?;
			Some((role, member))
		})
		.await;
	running
		.until("the rollback", |state: &State| {
			let role_back = state
				.guild(guild.id)
				.and_then(|found| found.roles.get(&guild.role))
				.is_some_and(|role| role.name == "Members");
			role_back
				&& nickname(state) == before
				&& state
					.last_rejection()
					.is_some_and(|rejection| rejection.reason.starts_with("Nickname"))
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audit_log_loads_and_pages_back_with_before() {
	let (running, guild) = start("moderation-audit").await;
	running.mock().add_audit_entries(guild.id.0, EXTRA_AUDIT);
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
		user_id: None,
	});
	running
		.until("the first page", |state: &State| {
			state
				.audit_log(guild.id)
				.is_some_and(|log| log.entries.len() == 50 && log.has_more)
		})
		.await;
	let last = running
		.session()
		.state()
		.read()
		.audit_log(guild.id)
		.and_then(|log| log.entries.last().map(|entry| entry.id))
		.expect("a last entry");
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: Some(last),
		action_type: None,
		user_id: None,
	});
	running
		.until("the second page", |state: &State| {
			state.audit_log(guild.id).is_some_and(|log| {
				log.entries.len() > 50
					&& !log.has_more
					&& log
						.entries
						.iter()
						.any(|entry| entry.action_type == 22 && entry.reason.is_some())
			})
		})
		.await;
	let path = format!("/guilds/{}/audit-logs", guild.id);
	let sent = queries(&running.mock().recording(), &path);
	assert_eq!(sent[0], "limit=50");
	assert_eq!(sent[1], format!("limit=50&before={last}"));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audit_log_filters_by_the_user_who_acted() {
	let (running, guild) = start("moderation-audit-user").await;
	assert_ne!(guild.me, guild.owner);
	running.mock().add_audit_entries(guild.id.0, 3);
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
		user_id: Some(guild.owner),
	});
	running
		.until("the owner's entries", |state: &State| {
			state.audit_log(guild.id).is_some_and(|log| {
				!log.entries.is_empty() && log.entries.iter().all(|entry| entry.user_id == Some(guild.owner))
			})
		})
		.await;
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
		user_id: Some(guild.me),
	});
	running
		.until("only the current user's entries", |state: &State| {
			state.audit_log(guild.id).is_some_and(|log| {
				log.entries.len() == 3 && log.entries.iter().all(|entry| entry.user_id == Some(guild.me))
			})
		})
		.await;
	let path = format!("/guilds/{}/audit-logs", guild.id);
	let sent = queries(&running.mock().recording(), &path);
	assert_eq!(sent[0], format!("limit=50&user_id={}", guild.owner));
	assert_eq!(sent[1], format!("limit=50&user_id={}", guild.me));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bans_load_page_after_the_last_id_and_an_unban_leaves_the_list() {
	let (running, guild) = start("moderation-bans").await;
	let banned: Vec<u64> = (1..=EXTRA_BANS).map(|step| 1u64 << 60 | step).collect();
	running.mock().ban_users(guild.id.0, &banned);
	running.session().send(Command::LoadBans { guild_id: guild.id });
	running
		.until("the first page of bans", |state: &State| {
			state
				.ban_list(guild.id)
				.is_some_and(|list| list.entries.len() == 1000 && list.has_more)
		})
		.await;
	running
		.session()
		.send(Command::LoadMoreBans { guild_id: guild.id });
	running
		.until("every ban", |state: &State| {
			state
				.ban_list(guild.id)
				.is_some_and(|list| list.entries.len() > 1000 && !list.has_more)
		})
		.await;
	let total = running.session().state().read().bans(guild.id).len();
	assert!(total >= EXTRA_BANS as usize);
	let path = format!("/guilds/{}/bans", guild.id);
	let sent = queries(&running.mock().recording(), &path);
	let last_of_first = running.session().state().read().bans(guild.id)[999].user.id;
	assert_eq!(
		sent,
		vec![
			"limit=1000".to_owned(),
			format!("limit=1000&after={last_of_first}")
		]
	);
	let revoked = Snowflake(banned[0]);
	running.session().send(Command::UnbanMember {
		guild_id: guild.id,
		user_id: revoked,
	});
	running
		.until("the unban to leave the list", |state: &State| {
			state.bans(guild.id).iter().all(|ban| ban.user.id != revoked)
		})
		.await;
	assert_eq!(running.session().state().read().bans(guild.id).len(), total - 1);
	running.stop().await;
}

fn holds(state: &State, guild: &Guild, user_id: Snowflake) -> bool {
	state
		.guild(guild.id)
		.and_then(|found| found.members.get(&user_id))
		.is_some_and(|member| member.roles.contains(&guild.role))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choosing_a_held_role_sends_one_delete_and_the_member_loses_it() {
	let (running, guild) = start("moderation-role-remove").await;
	let target = guild.targets[0];
	let role_path = format!("{}/roles/{}", member_path(&guild, target), guild.role);
	if !holds(&running.session().state().read(), &guild, target) {
		running.session().send(Command::AddMemberRole {
			guild_id: guild.id,
			user_id: target,
			role_id: guild.role,
		});
		running
			.until_wire("the role grant", |recording| {
				requests(recording, "PUT", &role_path).pop()
			})
			.await;
	}
	running
		.until("the member to hold the role", |state: &State| {
			holds(state, &guild, target)
		})
		.await;
	running.session().send(Command::RemoveMemberRole {
		guild_id: guild.id,
		user_id: target,
		role_id: guild.role,
	});
	let (body, reason, _) = running
		.until_wire("the role removal", |recording| {
			requests(recording, "DELETE", &role_path).pop()
		})
		.await;
	assert_eq!(body, None);
	assert_eq!(reason, None);
	running
		.until("the member without the role", |state: &State| {
			!holds(state, &guild, target)
		})
		.await;
	quiet().await;
	let recording = running.mock().recording();
	assert_eq!(requests(&recording, "DELETE", &role_path).len(), 1);
	assert!(
		requests(&recording, "PATCH", &member_path(&guild, target)).is_empty(),
		"a role change goes to the member's role route, not a member edit"
	);
	running.stop().await;
}
