use std::time::Duration;

use plugins::{Limits, PluginHost, Status};
use protocol::Snowflake;
use serde_json::json;
use session::{
	Accessory, DeletedLook, DisplayHooks, Formatting, HistoryLook, KeptMessage, KeptVersion, MessageLook,
};

use crate::support::{Lab, approve, config, scratch, start, write_plugin};

const ID: &str = "message-logger";
const REMOVE: &str = "Remove from log";

fn kept(id: u64, deleted: bool, versions: usize) -> KeptMessage {
	KeptMessage {
		id: Snowflake(id),
		channel_id: Snowflake(7),
		guild_id: Some(Snowflake(3)),
		author_id: Snowflake(41),
		bot: false,
		mine: false,
		content: "now".to_owned(),
		deleted_at: deleted.then_some(1_700_000_100),
		edited_at: (versions > 0).then_some(1_700_000_050),
		versions: (0..versions)
			.map(|index| KeptVersion {
				content: format!("version {index}"),
				edited_at: (index > 0).then_some(1_700_000_000 + index as i64),
				replaced_at: Some(1_700_000_010 + index as i64),
				attachments: 0,
				embeds: 0,
			})
			.collect(),
	}
}

fn remove_link() -> Vec<Accessory> {
	vec![Accessory {
		plugin: ID.to_owned(),
		text: REMOVE.to_owned(),
		action: Some("remove".to_owned()),
	}]
}

fn look(
	deleted: Option<DeletedLook>,
	history: Option<HistoryLook>,
	accessories: Vec<Accessory>,
) -> Option<MessageLook> {
	Some(MessageLook {
		deleted,
		history,
		accessories,
	})
}

fn host(root: &std::path::Path, limits: Limits, bundled: bool) -> PluginHost {
	PluginHost::new(config(root, limits, bundled))
}

fn shows_deleted(lab: &Lab) -> bool {
	lab.running.session().state().read().shows_deleted_messages()
}

async fn until_shown(lab: &Lab, show: bool, what: &str) {
	lab.running
		.until(what, |state| state.shows_deleted_messages() == show)
		.await;
}

#[test]
fn the_render_hook_runs_only_with_its_permission_and_draws_accessories_only_with_the_slot() {
	let root = scratch("message-render-permissions");
	let logic = r#"
return {
	render_message = function(message)
		return { deleted = "tint", accessory = { text = message.content .. " " .. #message.versions, action = "poke" } }
	end,
}
"#;
	write_plugin(&root, "no-grants", &[], "", logic);
	write_plugin(&root, "render-only", &["hook:message_render"], "", logic);
	let host = host(&root, Limits::default(), true);
	assert_eq!(host.status(ID), Some(Status::Disabled), "it ships disabled");
	approve(&host, "no-grants");
	assert_eq!(host.render_message(&kept(1, true, 0)), None);
	assert!(DisplayHooks::formatters(&host, Formatting::Messages).is_empty());

	approve(&host, "render-only");
	let formatters = DisplayHooks::formatters(&host, Formatting::Messages);
	assert_eq!(formatters.len(), 1, "{formatters:?}");
	assert!(formatters[0].starts_with("render-only/"), "{formatters:?}");
	assert_eq!(
		host.render_message(&kept(1, true, 2)),
		look(Some(DeletedLook::Tint), None, Vec::new()),
		"without ui:message.accessory its accessory is dropped"
	);

	write_plugin(
		&root,
		"with-slot",
		&["hook:message_render", "ui:message.accessory"],
		"",
		logic,
	);
	host.scan();
	approve(&host, "with-slot");
	let shown = host.render_message(&kept(1, true, 2)).expect("a look");
	assert_eq!(
		shown.accessories,
		vec![Accessory {
			plugin: "with-slot".to_owned(),
			text: "now 2".to_owned(),
			action: Some("poke".to_owned()),
		}]
	);
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn message_logger_styles_deleted_and_edited_messages_by_its_settings() {
	let root = scratch("message-logger-settings");
	let host = host(&root, Limits::default(), true);
	assert_eq!(host.render_message(&kept(1, true, 0)), None, "nothing while off");
	approve(&host, ID);

	assert_eq!(
		host.render_message(&kept(1, true, 0)),
		look(Some(DeletedLook::Text), None, remove_link())
	);
	assert_eq!(
		host.render_message(&kept(2, false, 2)),
		look(None, Some(HistoryLook::Inline), remove_link())
	);
	assert_eq!(
		host.render_message(&kept(3, true, 1)),
		look(Some(DeletedLook::Text), Some(HistoryLook::Inline), remove_link())
	);
	assert_eq!(host.render_message(&kept(4, false, 0)), None);

	host.set_setting(ID, "deleted_style", &json!("tint"))
		.expect("a setting");
	host.set_setting(ID, "inline_edits", &json!(false))
		.expect("a setting");
	host.set_setting(ID, "remove_button", &json!(false))
		.expect("a setting");
	assert_eq!(
		host.render_message(&kept(1, true, 0)),
		look(Some(DeletedLook::Tint), None, Vec::new())
	);
	assert_eq!(
		host.render_message(&kept(2, false, 2)),
		look(None, Some(HistoryLook::Collapsed), Vec::new())
	);

	host.set_setting(ID, "show", &json!("deleted"))
		.expect("a setting");
	assert_eq!(host.render_message(&kept(2, false, 2)), None);
	assert_eq!(
		host.render_message(&kept(3, true, 1)),
		look(Some(DeletedLook::Tint), None, Vec::new())
	);

	host.set_setting(ID, "show", &json!("edits")).expect("a setting");
	assert_eq!(
		host.render_message(&kept(1, true, 0)),
		look(Some(DeletedLook::Hidden), None, Vec::new())
	);
	assert_eq!(
		host.render_message(&kept(2, false, 2)),
		look(None, Some(HistoryLook::Collapsed), Vec::new())
	);

	host.set_setting(ID, "show", &json!("both")).expect("a setting");
	let mut bot = kept(5, true, 0);
	bot.bot = true;
	let mut mine = kept(6, false, 1);
	mine.mine = true;
	assert!(host.render_message(&bot).is_some());
	assert!(host.render_message(&mine).is_some());
	host.set_setting(ID, "ignore_bots", &json!(true))
		.expect("a setting");
	host.set_setting(ID, "ignore_self", &json!(true))
		.expect("a setting");
	assert_eq!(
		host.render_message(&bot),
		look(Some(DeletedLook::Hidden), None, Vec::new())
	);
	assert_eq!(host.render_message(&mine), None);

	host.disable(ID);
	assert_eq!(host.render_message(&kept(1, true, 0)), None);
	assert!(DisplayHooks::formatters(&host, Formatting::Messages).is_empty());
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn remove_from_log_stops_showing_a_message_and_is_remembered() {
	let root = scratch("message-logger-remove");
	let host = host(&root, Limits::default(), true);
	approve(&host, ID);
	let before = DisplayHooks::formatters(&host, Formatting::Messages);
	assert!(host.render_message(&kept(9, true, 0)).is_some());

	let context = [
		("source".to_owned(), "slot".to_owned()),
		("slot".to_owned(), "message.accessory".to_owned()),
		("channel_id".to_owned(), "7".to_owned()),
		("message_id".to_owned(), "9".to_owned()),
	];
	host.gesture(ID, "remove", &context).expect("the click runs");
	assert_ne!(
		DisplayHooks::formatters(&host, Formatting::Messages),
		before,
		"an action asks the plugin again"
	);
	assert_eq!(
		host.render_message(&kept(9, true, 0)),
		look(Some(DeletedLook::Hidden), None, Vec::new())
	);
	assert_eq!(host.render_message(&kept(9, false, 3)), None);
	assert!(host.render_message(&kept(10, true, 0)).is_some());
	drop(host);

	let again = PluginHost::new(config(&root, Limits::default(), true));
	assert_eq!(again.status(ID), Some(Status::Running));
	assert_eq!(
		again.render_message(&kept(9, true, 0)),
		look(Some(DeletedLook::Hidden), None, Vec::new()),
		"removed messages stay removed after a restart"
	);
	let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deleted_messages_show_while_message_logger_runs_and_on_every_start() {
	let root = scratch("message-logger-show");
	let lab = start("message-logger-show", root.clone(), Limits::default(), true).await;
	assert!(!shows_deleted(&lab), "off at start while the plugin is off");

	approve(&lab.host, ID);
	until_shown(&lab, true, "enabling to show deleted messages").await;
	assert!(lab.host.shows_deleted());

	lab.host
		.set_setting(ID, "show", &json!("edits"))
		.expect("a setting");
	until_shown(&lab, false, "edit history only to hide deleted messages").await;
	lab.host
		.set_setting(ID, "show", &json!("deleted"))
		.expect("a setting");
	until_shown(&lab, true, "deleted messages only to show them").await;

	lab.host.disable(ID);
	until_shown(&lab, false, "disabling to hide deleted messages").await;
	lab.host.enable(ID).expect("it enables again");
	until_shown(&lab, true, "enabling again to show deleted messages").await;
	lab.running.stop().await;

	let lab = start("message-logger-restart", root, Limits::default(), true).await;
	assert_eq!(lab.host.status(ID), Some(Status::Running));
	until_shown(
		&lab,
		true,
		"a new start with the plugin on to show deleted messages",
	)
	.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_renderer_fails_open_and_is_suspended() {
	let root = scratch("message-render-faults");
	write_plugin(
		&root,
		"broken-render",
		&["hook:message_render"],
		"",
		r#"
return {
	show_deleted = function() return true end,
	render_message = function(message) error("no looks today") end,
}
"#,
	);
	write_plugin(
		&root,
		"slow-render",
		&["hook:message_render"],
		"",
		"return { render_message = function(message) while true do end end }",
	);
	let limits = Limits {
		render_deadline: Duration::from_millis(30),
		..Limits::default()
	};
	let lab = start("message-render-faults", root, limits, false).await;
	approve(&lab.host, "broken-render");
	approve(&lab.host, "slow-render");
	until_shown(&lab, true, "a running plugin to show deleted messages").await;

	for _ in 0..3 {
		let started = std::time::Instant::now();
		assert_eq!(lab.host.render_message(&kept(1, true, 1)), None);
		assert!(
			started.elapsed() < Duration::from_secs(1),
			"{:?}",
			started.elapsed()
		);
	}
	assert!(matches!(
		lab.host.status("broken-render"),
		Some(Status::Suspended { .. })
	));
	assert!(matches!(
		lab.host.status("slow-render"),
		Some(Status::Suspended { .. })
	));
	assert!(DisplayHooks::formatters(&lab.host, Formatting::Messages).is_empty());
	until_shown(&lab, false, "a suspended plugin to hide deleted messages again").await;
	lab.stop().await;
}
