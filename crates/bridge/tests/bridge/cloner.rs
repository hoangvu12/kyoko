use std::path::PathBuf;
use std::time::Duration;

use bridge::{MenuFacts, PluginMenuItem};
use media::{Limits, MediaService};
use plugins::{HostConfig, Permission, PluginHost, Slot};
use protocol::fingerprint::Fingerprint;
use protocol::{Snowflake, StickerFormat};
use serde_json::json;
use state::{Expression, ExpressionKind, State};

use crate::harness::Running;

const CLONER: &str = "expression-cloner";

fn emoji() -> Expression {
	bridge::emoji_expression("blobwave", Snowflake(4242), false)
}

fn sticker(format: StickerFormat) -> Expression {
	Expression::Sticker {
		id: Snowflake(5151),
		name: "wave".to_owned(),
		format,
	}
}

fn cloner_items(root: &PathBuf) -> Vec<PluginMenuItem> {
	let _ = std::fs::remove_dir_all(root);
	let host = PluginHost::new(HostConfig {
		data_dir: Some(root.clone()),
		user_dir: None,
		development_dirs: Vec::new(),
		bundled: true,
		limits: plugins::Limits {
			load_deadline: Duration::from_secs(15),
			..plugins::Limits::default()
		},
	});
	let declared: Vec<Permission> = host
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == CLONER)
		.map(|plugin| plugin.permissions)
		.expect("the bundled plugin is listed");
	host.approve(CLONER, &declared).expect("the plugin starts");
	host.menu_items(Slot::MessageMenu)
		.into_iter()
		.map(|item| PluginMenuItem {
			plugin: item.plugin,
			key: item.key,
			label: item.label,
			icon: item.icon.map(Into::into),
			when: item.when,
		})
		.collect()
}

fn shown(items: &[PluginMenuItem], state: &State, target: Option<Expression>) -> Vec<String> {
	let facts = MenuFacts::of(state, target);
	bridge::shown_plugin_items(items, &facts)
		.into_iter()
		.map(|index| items[index].label.clone())
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clone_shows_only_for_custom_emoji_and_image_stickers_while_expressions_can_be_added() {
	let mut running = Running::start("cloner-menu").await;
	let root = running.scratch.join("plugins");
	let items = cloner_items(&root);
	let guild_ids: Vec<Snowflake> = running
		.server
		.fixtures()
		.guilds
		.iter()
		.map(|guild| Snowflake(guild.id))
		.collect();
	running
		.until_state(
			"expressions can be added somewhere",
			State::may_create_expressions_anywhere,
		)
		.await;
	{
		let state = running.session().state().read();
		assert_eq!(shown(&items, &state, Some(emoji())), ["Clone Emoji"]);
		assert_eq!(
			shown(&items, &state, Some(sticker(StickerFormat::Png))),
			["Clone Sticker"]
		);
		assert_eq!(
			shown(&items, &state, Some(sticker(StickerFormat::Gif))),
			["Clone Sticker"]
		);
		assert!(shown(&items, &state, Some(sticker(StickerFormat::Lottie))).is_empty());
		assert!(
			shown(&items, &state, None).is_empty(),
			"a Unicode emoji or plain text is no target"
		);
		let targets: Vec<Snowflake> = bridge::clone_targets(&state, &emoji())
			.into_iter()
			.map(|target| target.guild_id)
			.collect();
		let mut allowed: Vec<Snowflake> = guild_ids
			.iter()
			.copied()
			.filter(|guild_id| state.may_create_expressions(*guild_id))
			.collect();
		allowed.sort();
		let mut listed = targets.clone();
		listed.sort();
		assert_eq!(
			listed, allowed,
			"only servers where expressions can be added are offered"
		);
	}
	let user = running.server.fixtures().current_user.clone();
	for guild_id in &guild_ids {
		running.server.control().dispatch(
			"GUILD_MEMBER_UPDATE",
			&json!({ "guild_id": guild_id.to_string(), "user": user, "roles": [], "joined_at": "2025-01-01T00:00:00+00:00" }),
		);
	}
	running
		.until_state("no server takes expressions any more", |state| {
			!state.may_create_expressions_anywhere()
		})
		.await;
	{
		let state = running.session().state().read();
		assert!(shown(&items, &state, Some(emoji())).is_empty());
		assert!(shown(&items, &state, Some(sticker(StickerFormat::Png))).is_empty());
		assert!(bridge::clone_targets(&state, &emoji()).is_empty());
	}
	running.stop().await;
	let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_server_is_listed_with_why_and_cannot_be_picked() {
	let mut running = Running::start("cloner-full").await;
	let guild_id = Snowflake(running.server.fixtures().guilds[0].id);
	running
		.until_state("expressions can be added to the first server", |state| {
			state.may_create_expressions(guild_id)
		})
		.await;
	let max = running
		.session()
		.state()
		.read()
		.expression_slots(guild_id, ExpressionKind::Emoji)
		.expect("the server")
		.max;
	let emojis: Vec<serde_json::Value> = (0..max)
		.map(|index| json!({ "id": (9_000_000 + index).to_string(), "name": format!("still_{index}"), "animated": false }))
		.collect();
	running.server.control().dispatch(
		"GUILD_EMOJIS_UPDATE",
		&json!({ "guild_id": guild_id.to_string(), "emojis": emojis }),
	);
	running
		.until_state("a full server", |state| {
			state
				.expression_slots(guild_id, ExpressionKind::Emoji)
				.is_some_and(state::Slots::full)
		})
		.await;
	{
		let state = running.session().state().read();
		let still = bridge::clone_targets(&state, &emoji());
		let full = still
			.iter()
			.find(|target| target.guild_id == guild_id)
			.expect("the full server is still listed");
		assert_eq!(
			full.blocked.as_deref(),
			Some(format!("No free emoji slots ({max} of {max})").as_str())
		);
		let animated = bridge::emoji_expression("blobdance", Snowflake(4343), true);
		let moving = bridge::clone_targets(&state, &animated);
		let open = moving
			.iter()
			.find(|target| target.guild_id == guild_id)
			.expect("the server is listed for animated emoji");
		assert_eq!(open.blocked, None, "animated emoji have slots of their own");
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloning_fetches_through_the_media_pipeline_and_creates_the_copy() {
	let mut running = Running::start("cloner-copy").await;
	let guild_id = Snowflake(running.server.fixtures().guilds[0].id);
	running
		.until_state("expressions can be added to the first server", |state| {
			state.may_create_expressions(guild_id) && state.guild_stickers(guild_id).is_some()
		})
		.await;
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let port = running.server.address().port();
	let fetcher = bridge::media_fetcher(&fingerprint, Some(port)).expect("the fetcher builds");
	let loader = MediaService::new(fetcher, running.scratch.join("cache"), Limits::default());
	let origin = format!("http://127.0.0.1:{port}");
	let cdn = running.server.cdn_url();
	let animated = bridge::emoji_expression("blobdance", Snowflake(4242), true);
	for (expression, name) in [
		(animated, "blobdance_copy"),
		(sticker(StickerFormat::Png), "wave copy"),
	] {
		let source = bridge::clone_source(&cdn, &expression).expect("an image to fetch");
		let url = bridge::local_media(&source, Some(&origin));
		let bytes = loader.bytes(&url).await.expect("the image downloads").to_vec();
		let command = {
			let state = running.session().state().read();
			assert!(bridge::clone_name_problem(&expression, name).is_none());
			bridge::clone_command(&state, &expression, guild_id, name, bytes)
		};
		running.session().send(command);
		let wanted = name.to_owned();
		let sticker = expression.is_sticker();
		running
			.until_state("the copy in the server", move |state| {
				if sticker {
					state
						.guild_stickers(guild_id)
						.is_some_and(|stickers| stickers.iter().any(|sticker| sticker.name == wanted))
				} else {
					state.guild(guild_id).is_some_and(|guild| {
						guild.emojis.iter().any(|emoji| {
							emoji.name.as_deref() == Some(wanted.as_str()) && emoji.animated == Some(true)
						})
					})
				}
			})
			.await;
	}
	let recording = running.server.control().recording();
	let fetched: Vec<String> = recording
		.media_to("/cdn/emojis/")
		.map(mockcord::MediaRequest::url)
		.collect();
	assert_eq!(fetched.len(), 1, "{fetched:?}");
	assert!(fetched[0].contains("/cdn/emojis/4242.gif"), "{fetched:?}");
	let uploaded = recording
		.requests_to("POST", &format!("/guilds/{guild_id}/emojis"))
		.filter_map(mockcord::RestRequest::json)
		.collect::<Vec<_>>();
	assert_eq!(uploaded.len(), 1);
	assert!(
		uploaded[0]["image"]
			.as_str()
			.is_some_and(|image| image.starts_with("data:image/gif;base64,")),
		"the animation is kept"
	);
	assert_eq!(
		recording
			.requests_to("POST", &format!("/guilds/{guild_id}/stickers"))
			.count(),
		1
	);
	running.stop().await;
}
