use bridge::{
	AuditTone, ChannelKind, GuildAccess, MemberSort, ModerationItem, OpenChannel, PickedImage, ServerPage,
	audit_rows, first_channel, member_rows, moderation_items, prepared_image, replacement_channel,
	role_move_to, role_rows,
};
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
	assert_eq!(
		role_move_to(state, guild.id, muted, rows[2].id),
		Some(vec![(muted, 2), (rows[2].id, 1)]),
		"Muted moves up past Members"
	);
	roles_move_where_they_are_dropped(state, guild);
}

fn roles_move_where_they_are_dropped(state: &State, guild: &Guild) {
	let rows = role_rows(state, guild.id);
	let (admin, mods, members, muted, everyone) =
		(rows[0].id, rows[1].id, rows[2].id, rows[3].id, rows[4].id);
	assert_eq!(
		role_move_to(state, guild.id, muted, mods),
		Some(vec![(muted, 3), (mods, 2), (members, 1)]),
		"Muted dropped on Mods takes its place and the roles between move down"
	);
	assert_eq!(
		role_move_to(state, guild.id, mods, muted),
		Some(vec![(members, 3), (muted, 2), (mods, 1)]),
		"Mods dropped on Muted goes below the roles between"
	);
	assert_eq!(
		role_move_to(state, guild.id, mods, admin),
		None,
		"a role above the user's stays put"
	);
	assert_eq!(
		role_move_to(state, guild.id, muted, everyone),
		None,
		"@everyone stays last"
	);
	assert_eq!(role_move_to(state, guild.id, muted, muted), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn members_sort_by_when_they_joined_or_by_name() {
	let (running, guild) = started("admin-member-sort").await;
	members_follow_the_sort(&running.session().state().read(), &guild);
	running.stop().await;
}

fn members_follow_the_sort(state: &State, guild: &Guild) {
	let (newest, total) = member_rows(state, guild.id, "", MemberSort::Newest);
	assert_eq!(newest.len(), total);
	assert!(
		newest.windows(2).all(|pair| pair[0].joined >= pair[1].joined),
		"Member Since lists the newest members first"
	);
	assert!(newest.iter().any(|row| row.joined.is_some()));
	let (oldest, _) = member_rows(state, guild.id, "", MemberSort::Oldest);
	let dated: Vec<_> = oldest.iter().filter_map(|row| row.joined).collect();
	assert!(dated.windows(2).all(|pair| pair[0] <= pair[1]));
	let (named, _) = member_rows(state, guild.id, "", MemberSort::Name);
	let names: Vec<String> = named.iter().map(|row| row.name.to_lowercase()).collect();
	let mut sorted = names.clone();
	sorted.sort();
	assert_eq!(names, sorted);
	assert_eq!(MemberSort::at(0), MemberSort::Newest);
	assert_eq!(MemberSort::at(99), MemberSort::Newest);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audit_log_reads_as_sentences_with_reasons() {
	let (mut running, guild) = started("admin-audit").await;
	running.session().send(Command::LoadAuditLog {
		guild_id: guild.id,
		before: None,
		action_type: None,
		user_id: None,
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
	assert_eq!(banned.tone, AuditTone::Delete);
	assert_eq!(banned.icon, "gavel");
	let strong: Vec<&str> = banned
		.parts
		.iter()
		.filter(|(_, strong)| *strong)
		.map(|(text, _)| text.as_str())
		.collect();
	assert_eq!(
		strong,
		[
			banned.actor.as_str(),
			banned.sentence.rsplit(" banned ").next().unwrap_or_default()
		]
	);
	let joined: String = banned.parts.iter().map(|(text, _)| text.as_str()).collect();
	assert_eq!(joined, banned.sentence, "the parts read as the sentence");
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deleting_the_open_channel_moves_the_view_to_the_servers_first_channel() {
	let mut running = Running::start("admin-delete-open").await;
	let open = first_channel(&running.session().state().read()).expect("a channel to open");
	let guild_id = open.guild_id.expect("a server channel");
	if let Some(command) = running.select_guild(guild_id) {
		running.session().send(command);
	}
	let command = running.open_channel(open.channel_id).expect("the channel opens");
	running.session().send(command);
	assert_eq!(
		replacement_channel(&running.session().state().read(), open),
		None,
		"a channel that is still there stays open"
	);
	running.session().send(Command::DeleteChannel {
		channel_id: open.channel_id,
	});
	running
		.until("the deleted channel to leave the sidebar", |view| {
			view.sidebar.channel(open.channel_id).is_none()
		})
		.await;
	assert_eq!(running.view.timeline.open(), None, "the timeline closes");
	let first = running
		.view
		.sidebar
		.channels()
		.find(|channel| {
			!channel.locked && matches!(channel.kind, ChannelKind::Text | ChannelKind::Announcement)
		})
		.map(|channel| channel.id)
		.expect("another text channel");
	let replacement = replacement_channel(&running.session().state().read(), open);
	assert_eq!(
		replacement,
		Some(OpenChannel {
			guild_id: Some(guild_id),
			channel_id: first,
		}),
		"the server's first text channel in sidebar order replaces it"
	);
	assert!(running.open_channel(first).is_some(), "the replacement opens");
	running.stop().await;
}

fn gif(side: u32, frames: u8, noisy: bool) -> Vec<u8> {
	let mut encoded = Vec::new();
	{
		let mut encoder = image::codecs::gif::GifEncoder::new(&mut encoded);
		encoder
			.set_repeat(image::codecs::gif::Repeat::Infinite)
			.expect("a looping gif");
		let mut seed = 7u32;
		for frame in 0..frames {
			let picture = image::RgbaImage::from_fn(side, side, |x, y| {
				seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
				let noise = if noisy { (seed >> 16) as u8 } else { 0 };
				image::Rgba([(x as u8) ^ noise, (y as u8).wrapping_add(frame * 40), noise, 255])
			});
			encoder
				.encode_frame(image::Frame::new(picture))
				.expect("a gif frame");
		}
	}
	encoded
}

fn png(width: u32, height: u32) -> Vec<u8> {
	let picture = image::RgbaImage::from_pixel(width, height, image::Rgba([90, 120, 200, 255]));
	let mut encoded = std::io::Cursor::new(Vec::new());
	picture
		.write_to(&mut encoded, image::ImageFormat::Png)
		.expect("a png");
	encoded.into_inner()
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
	assert!(bytes.starts_with(b"\x89PNG"), "not a png");
	let picture = image::load_from_memory(bytes).expect("a readable png");
	(picture.width(), picture.height())
}

#[test]
fn picked_emoji_and_stickers_keep_their_animation_and_stills_are_fitted_as_png() {
	let animated = gif(200, 3, false);
	assert!(animated.len() <= state::EMOJI_MAX_BYTES);
	assert_eq!(
		prepared_image(animated.clone(), PickedImage::Emoji),
		Some(animated.clone())
	);
	assert_eq!(
		prepared_image(animated.clone(), PickedImage::Sticker),
		Some(animated.clone())
	);
	assert_eq!(
		png_size(&prepared_image(animated, PickedImage::Icon).expect("an icon")),
		(200, 200)
	);
	let still = gif(200, 1, false);
	assert_eq!(
		png_size(&prepared_image(still, PickedImage::Emoji).expect("an emoji")),
		(128, 128)
	);
	let heavy = gif(600, 2, true);
	assert!(heavy.len() > state::STICKER_MAX_BYTES, "{} bytes", heavy.len());
	assert_eq!(
		png_size(&prepared_image(heavy.clone(), PickedImage::Emoji).expect("a flattened emoji")),
		(128, 128)
	);
	assert_eq!(
		png_size(&prepared_image(heavy, PickedImage::Sticker).expect("a flattened sticker")),
		(320, 320)
	);
	assert_eq!(
		png_size(&prepared_image(png(600, 300), PickedImage::Sticker).expect("a sticker")),
		(320, 160)
	);
	assert_eq!(
		png_size(&prepared_image(png(1024, 512), PickedImage::Icon).expect("an icon")),
		(512, 256)
	);
	assert_eq!(
		prepared_image(b"not a picture".to_vec(), PickedImage::Emoji),
		None
	);
}
