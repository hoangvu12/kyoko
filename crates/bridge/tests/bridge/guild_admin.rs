use bridge::{GuildAccess, ModerationItem, ServerPage, audit_rows, moderation_items, role_rows, role_swap};
use chrono::Utc;
use protocol::{Permissions, Snowflake};
use serde_json::json;
use session::Command;
use state::State;

use crate::harness::Running;

struct Guild {
	id: Snowflake,
	me: Snowflake,
	owner: Snowflake,
	below: Snowflake,
}

async fn started(name: &str) -> (Running, Guild) {
	let mut running = Running::start(name).await;
	let guild_id = Snowflake(running.server.fixtures().guilds[0].id);
	let user_ids: Vec<Snowflake> = running.server.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.collect();
	running
		.session()
		.send(Command::RequestMembers { guild_id, user_ids });
	running
		.until_state("the members of the first guild", |state| {
			state.guild(guild_id).is_some_and(|guild| guild.members.len() > 3)
		})
		.await;
	let guild = {
		let state = running.session().state().read();
		let me = state.current_user_id().expect("signed in");
		let owner = state
			.guild(guild_id)
			.and_then(|guild| guild.owner_id)
			.expect("an owner");
		let found = state.guild(guild_id).expect("the guild");
		let mut candidates: Vec<Snowflake> = found
			.members
			.iter()
			.map(|(id, _)| *id)
			.filter(|id| *id != me && *id != owner && state.outranks(guild_id, me, *id))
			.filter(|id| !found.voice_states.contains_key(id))
			.collect();
		candidates.sort();
		let below = candidates
			.first()
			.copied()
			.expect("a member below the current user");
		Guild {
			id: guild_id,
			me,
			owner,
			below,
		}
	};
	(running, guild)
}

fn items(state: &State, guild: &Guild, target: Snowflake) -> Vec<ModerationItem> {
	moderation_items(state, guild.id, target, Utc::now())
}

fn labels(items: &[ModerationItem]) -> Vec<&'static str> {
	items.iter().map(ModerationItem::label).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_member_menu_offers_moderation_only_below_the_current_users_rank() {
	let (mut running, guild) = started("admin-menu").await;
	{
		let state = running.session().state().read();
		let below = items(&state, &guild, guild.below);
		assert_eq!(
			labels(&below),
			["Change Nickname", "Roles", "", "Timeout", "Kick", "Ban"]
		);
		let Some(ModerationItem::Roles(roles)) = below.get(1) else {
			panic!("a roles submenu");
		};
		assert!(
			roles
				.iter()
				.all(|role| role.name != "Admin" && role.name != "@everyone")
		);
		assert!(items(&state, &guild, guild.owner).is_empty());
		assert_eq!(labels(&items(&state, &guild, guild.me)), ["Change Nickname"]);
	}
	let user = running.server.fixtures().current_user.clone();
	running.server.control().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({ "guild_id": guild.id.to_string(), "user": user, "roles": [], "joined_at": "2025-01-01T00:00:00+00:00" }),
	);
	running
		.until_state("the current user without roles", |state| {
			!state
				.guild_permissions(guild.id, guild.me)
				.contains(Permissions::KICK_MEMBERS)
		})
		.await;
	{
		let state = running.session().state().read();
		assert!(items(&state, &guild, guild.below).is_empty());
		let access = GuildAccess::of(&state, guild.id);
		assert_eq!(ServerPage::opening(None, &access), None);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn roles_list_from_the_top_and_only_lower_ones_move() {
	let (running, guild) = started("admin-roles").await;
	roles_follow_the_hierarchy(&running.session().state().read(), &guild);
	running.stop().await;
}

fn roles_follow_the_hierarchy(state: &State, guild: &Guild) {
	let rows = role_rows(state, guild.id);
	let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
	assert_eq!(names, ["Admin", "Mods", "Members", "Muted", "@everyone"]);
	let movable: Vec<bool> = rows.iter().map(|row| row.movable).collect();
	assert_eq!(movable, [false, true, true, true, false]);
	let access = GuildAccess::of(state, guild.id);
	assert_eq!(ServerPage::opening(None, &access), Some(ServerPage::Overview));
	assert!(ServerPage::ALL.iter().all(|page| page.allowed(&access)));
	let muted = rows[3].id;
	assert_eq!(role_swap(state, guild.id, muted, false), None);
	let swapped = role_swap(state, guild.id, muted, true).expect("Muted moves up");
	assert_eq!(swapped, vec![(muted, 2), (rows[2].id, 1)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audit_log_reads_as_sentences_with_reasons() {
	let (mut running, guild) = started("admin-audit").await;
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
	});
	running
		.until_state("the audit log", |state| state.audit_log(guild.id).is_some())
		.await;
	let rows =
		audit_rows(&running.session().state().read(), guild.id, bridge::today()).expect("the log is loaded");
	assert!(
		rows.iter()
			.any(|row| row.sentence.ends_with("created the channel announcements")
				&& row.changes[0].new == "announcements")
	);
	let banned = rows
		.iter()
		.find(|row| row.reason.as_deref() == Some("Spam links in every channel"))
		.expect("a ban with its reason");
	assert!(banned.sentence.contains(" banned "));
	running.stop().await;
}
