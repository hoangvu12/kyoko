use std::time::Duration;

use plugins::{Limits, PluginHost, Status};
use protocol::Snowflake;
use serde_json::json;
use session::{Badge, DecoratedUser, Decorations, DisplayHooks, Formatting, Platforms};

use crate::support::{approve, config, scratch, start, write_plugin};

const PLATFORMS: &str = "platform-indicators";
const CROWN: &str = "force-owner-crown";
const PROBE_ICON: &str =
	r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/></svg>"#;

fn user(owner: bool, platforms: Platforms) -> DecoratedUser {
	DecoratedUser {
		user_id: Snowflake(41),
		guild_id: Some(Snowflake(3)),
		bot: false,
		mine: false,
		owner,
		status: "online",
		platforms,
	}
}

fn desktop_and_mobile() -> Platforms {
	Platforms {
		desktop: Some("dnd"),
		mobile: Some("idle"),
		web: None,
	}
}

fn host(root: &std::path::Path, limits: Limits, bundled: bool) -> PluginHost {
	PluginHost::new(config(root, limits, bundled))
}

fn shown(badges: &[Badge]) -> Vec<(String, String, String)> {
	badges
		.iter()
		.map(|badge| (badge.key.clone(), badge.color.clone(), badge.tooltip.clone()))
		.collect()
}

fn indicator(key: &str, color: &str, tooltip: &str) -> (String, String, String) {
	(key.to_owned(), color.to_owned(), tooltip.to_owned())
}

const PROBE: &str = r#"
return {
	decorate_user = function(user)
		local seen = (user.status or "hidden") .. " " .. tostring(user.platforms and user.platforms.mobile) .. " " .. tostring(user.owner)
		local mark = { icon = "probe.svg", color = "$kyoko.status_dnd", tooltip = seen }
		return {
			members = { mark, { icon = "../escape.svg" }, { icon = "probe.svg", color = "Not A Token" } },
			messages = { mark },
			profiles = { mark },
			crown = true,
		}
	end,
}
"#;

fn write_probe(root: &std::path::Path, id: &str, permissions: &[&str]) {
	let folder = write_plugin(root, id, permissions, "", PROBE);
	std::fs::write(folder.join("probe.svg"), PROBE_ICON).expect("an icon");
}

#[test]
fn decorations_follow_the_slots_and_read_scope_a_plugin_was_granted() {
	let root = scratch("decoration-grants");
	write_probe(&root, "no-slot", &["read:users"]);
	write_probe(&root, "members-only", &["ui:member.decoration"]);
	let host = host(&root, Limits::default(), false);
	approve(&host, "no-slot");
	assert!(DisplayHooks::formatters(&host, Formatting::Decorations).is_empty());
	assert_eq!(host.decorate_user(&user(true, desktop_and_mobile())), None);

	approve(&host, "members-only");
	let formatters = DisplayHooks::formatters(&host, Formatting::Decorations);
	assert_eq!(formatters.len(), 1, "{formatters:?}");
	let decorations = host
		.decorate_user(&user(true, desktop_and_mobile()))
		.expect("decorations");
	assert!(decorations.crown, "the owner's crown with the member slot");
	assert!(decorations.messages.is_empty() && decorations.profiles.is_empty());
	assert_eq!(
		shown(&decorations.members),
		vec![
			indicator("probe", "status_dnd", "hidden nil true"),
			indicator("probe", "text_4", ""),
		],
		"no presence without read:users, the escaping icon left out, a bad colour muted"
	);
	assert_eq!(
		decorations.members[0].icon.as_deref(),
		Some(PROBE_ICON),
		"the icon comes from the plugin's own folder"
	);
	let not_owner = host
		.decorate_user(&user(false, desktop_and_mobile()))
		.expect("decorations");
	assert!(
		!not_owner.crown,
		"never a crown for someone who does not own the server"
	);

	write_probe(&root, "everywhere", &["read:users", "ui:user.decoration"]);
	host.scan();
	approve(&host, "everywhere");
	host.disable("members-only");
	let decorations = host
		.decorate_user(&user(true, desktop_and_mobile()))
		.expect("decorations");
	assert!(!decorations.crown, "the crown needs the member slot");
	assert!(decorations.members.is_empty());
	assert_eq!(
		shown(&decorations.messages),
		vec![indicator("probe", "status_dnd", "online idle true")]
	);
	assert_eq!(decorations.profiles, decorations.messages);
	assert_eq!(decorations.messages[0].plugin, "everywhere");
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn platform_indicators_show_each_platform_in_its_status_colour_by_its_settings() {
	let root = scratch("platform-indicators");
	let host = host(&root, Limits::default(), true);
	assert_eq!(
		host.status(PLATFORMS),
		Some(Status::Disabled),
		"it ships disabled"
	);
	assert_eq!(host.decorate_user(&user(false, desktop_and_mobile())), None);
	approve(&host, PLATFORMS);

	let decorations = host
		.decorate_user(&user(false, desktop_and_mobile()))
		.expect("decorations");
	let expected = vec![
		indicator("monitor", "status_dnd", "Do Not Disturb on Desktop"),
		indicator("smartphone", "status_idle", "Idle on Mobile"),
	];
	assert_eq!(shown(&decorations.members), expected);
	assert_eq!(shown(&decorations.messages), expected);
	assert_eq!(shown(&decorations.profiles), expected);
	assert!(!decorations.crown);
	assert!(decorations.members.iter().all(|badge| badge.icon.is_some()));

	let web = Platforms {
		web: Some("online"),
		..Platforms::default()
	};
	assert_eq!(
		shown(
			&host
				.decorate_user(&user(false, web.clone()))
				.expect("decorations")
				.members
		),
		vec![indicator("globe", "status_online", "Online on Web")]
	);
	assert_eq!(host.decorate_user(&user(false, Platforms::default())), None);
	let mut bot = user(false, web);
	bot.bot = true;
	assert_eq!(host.decorate_user(&bot), None, "no indicators on bots");

	host.set_setting(PLATFORMS, "messages", &json!(false))
		.expect("a setting");
	host.set_setting(PLATFORMS, "color_mobile", &json!(false))
		.expect("a setting");
	let decorations = host
		.decorate_user(&user(false, desktop_and_mobile()))
		.expect("decorations");
	assert!(decorations.messages.is_empty());
	assert_eq!(
		shown(&decorations.members)[1],
		indicator("smartphone", "text_4", "Idle on Mobile")
	);
	host.set_setting(PLATFORMS, "members", &json!(false))
		.expect("a setting");
	host.set_setting(PLATFORMS, "profiles", &json!(false))
		.expect("a setting");
	assert_eq!(host.decorate_user(&user(false, desktop_and_mobile())), None);

	host.disable(PLATFORMS);
	assert!(DisplayHooks::formatters(&host, Formatting::Decorations).is_empty());
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn force_owner_crown_crowns_only_the_owner_while_it_runs() {
	let root = scratch("force-owner-crown");
	let host = host(&root, Limits::default(), true);
	assert_eq!(host.status(CROWN), Some(Status::Disabled), "it ships disabled");
	assert_eq!(host.decorate_user(&user(true, Platforms::default())), None);
	approve(&host, CROWN);
	assert_eq!(
		host.decorate_user(&user(true, Platforms::default())),
		Some(Decorations {
			crown: true,
			..Decorations::default()
		})
	);
	assert_eq!(host.decorate_user(&user(false, Platforms::default())), None);
	host.disable(CROWN);
	assert_eq!(host.decorate_user(&user(true, Platforms::default())), None);
	let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_decorator_leaves_names_plain_and_is_suspended() {
	let root = scratch("decoration-faults");
	write_plugin(
		&root,
		"broken-decor",
		&["ui:member.decoration"],
		"",
		r#"return { decorate_user = function(user) error("no icons today") end }"#,
	);
	write_plugin(
		&root,
		"slow-decor",
		&["ui:user.decoration"],
		"",
		"return { decorate_user = function(user) while true do end end }",
	);
	let limits = Limits {
		decoration_deadline: Duration::from_millis(30),
		..Limits::default()
	};
	let lab = start("decoration-faults", root, limits, false).await;
	approve(&lab.host, "broken-decor");
	approve(&lab.host, "slow-decor");
	assert_eq!(
		DisplayHooks::formatters(&lab.host, Formatting::Decorations).len(),
		2
	);
	for _ in 0..3 {
		let started = std::time::Instant::now();
		assert_eq!(lab.host.decorate_user(&user(true, desktop_and_mobile())), None);
		assert!(
			started.elapsed() < Duration::from_secs(1),
			"{:?}",
			started.elapsed()
		);
	}
	for id in ["broken-decor", "slow-decor"] {
		assert!(
			matches!(lab.host.status(id), Some(Status::Suspended { .. })),
			"{id}: {:?}",
			lab.host.status(id)
		);
	}
	assert!(DisplayHooks::formatters(&lab.host, Formatting::Decorations).is_empty());
	lab.stop().await;
}
