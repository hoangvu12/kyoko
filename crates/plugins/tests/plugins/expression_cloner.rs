use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use plugins::{Limits, PluginHost, PluginRequest, Slot, Status};
use protocol::{Snowflake, StickerFormat};
use state::Expression;

use crate::support::{Lab, approve, scratch, start, write_plugin};

const CLONER: &str = "expression-cloner";
const QUIET: Duration = Duration::from_millis(300);

const SNEAKY: &str = r#"
local outcomes = {}

local function try(where)
	local ok = pcall(kyoko.actions.clone_expression)
	outcomes[where] = if ok then "ok" else "refused"
end

return {
	on_action = function(key)
		try(key)
	end,
	before_send = function(message)
		try("send")
		local keys = {}
		for key in outcomes do
			table.insert(keys, key)
		end
		table.sort(keys)
		local parts = {}
		for _, key in keys do
			table.insert(parts, key .. "=" .. outcomes[key])
		end
		return message.content .. " " .. table.concat(parts, " ")
	end,
}
"#;

const SNEAKY_MENU: &str = r#"
[[menu]]
slot = "message.menu"
key = "grab"
label = "Grab"
when = ["custom_emoji"]
"#;

fn emoji() -> Expression {
	Expression::Emoji {
		id: Snowflake(4242),
		name: "blobwave".to_owned(),
		animated: true,
	}
}

fn sticker(format: StickerFormat) -> Expression {
	Expression::Sticker {
		id: Snowflake(5151),
		name: "wave".to_owned(),
		format,
	}
}

fn listen(host: &PluginHost) -> Arc<Mutex<Vec<PluginRequest>>> {
	let heard = Arc::new(Mutex::new(Vec::new()));
	let sink = heard.clone();
	host.on_request(move |request| sink.lock().push(request));
	heard
}

async fn bundled(name: &'static str, limits: Limits) -> Lab {
	let lab = start(name, scratch(name), limits, true).await;
	assert_eq!(
		lab.host.status(CLONER),
		Some(Status::Disabled),
		"it ships disabled"
	);
	lab
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_clone_items_join_the_message_menu_only_once_approved() {
	let lab = bundled("cloner-items", Limits::default()).await;
	assert!(lab.host.menu_items(Slot::MessageMenu).is_empty());
	assert!(
		lab.host
			.menu_gesture(CLONER, "clone_emoji", Slot::MessageMenu, Some(emoji()), &[])
			.is_err(),
		"a plugin that is off offers nothing"
	);
	approve(&lab.host, CLONER);
	let items = lab.host.menu_items(Slot::MessageMenu);
	let labels: Vec<(&str, &str, Vec<&str>)> = items
		.iter()
		.map(|item| {
			(
				item.key.as_str(),
				item.label.as_str(),
				item.when.iter().map(String::as_str).collect(),
			)
		})
		.collect();
	assert_eq!(
		labels,
		[
			(
				"clone_emoji",
				"Clone Emoji",
				vec!["custom_emoji", "can_create_expressions"]
			),
			(
				"clone_sticker",
				"Clone Sticker",
				vec!["image_sticker", "can_create_expressions"]
			),
		]
	);
	assert!(
		items
			.iter()
			.all(|item| item.plugin == CLONER && item.icon.as_deref().is_some_and(|svg| svg.contains("<svg")))
	);
	lab.host.disable(CLONER);
	assert!(lab.host.menu_items(Slot::MessageMenu).is_empty());
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choosing_clone_asks_for_exactly_the_expression_that_was_clicked() {
	let lab = bundled("cloner-request", Limits::default()).await;
	approve(&lab.host, CLONER);
	let heard = listen(&lab.host);
	lab.host
		.menu_gesture(CLONER, "clone_emoji", Slot::MessageMenu, Some(emoji()), &[])
		.expect("the menu click reaches the plugin");
	lab.host
		.menu_gesture(
			CLONER,
			"clone_sticker",
			Slot::MessageMenu,
			Some(sticker(StickerFormat::Apng)),
			&[],
		)
		.expect("the menu click reaches the plugin");
	assert_eq!(
		*heard.lock(),
		[
			PluginRequest::Clone {
				plugin: CLONER.to_owned(),
				expression: emoji(),
			},
			PluginRequest::Clone {
				plugin: CLONER.to_owned(),
				expression: sticker(StickerFormat::Apng),
			},
		]
	);
	heard.lock().clear();
	lab.host
		.menu_gesture(
			CLONER,
			"clone_sticker",
			Slot::MessageMenu,
			Some(sticker(StickerFormat::Lottie)),
			&[],
		)
		.expect("a click on a Lottie sticker reaches the plugin, which cannot clone it");
	lab.host
		.menu_gesture(CLONER, "clone_emoji", Slot::MessageMenu, None, &[])
		.expect("a click with nothing under the pointer reaches the plugin");
	lab.host
		.gesture(
			CLONER,
			"clone_emoji",
			&[("source".to_owned(), "settings".to_owned())],
		)
		.expect("a settings click reaches the plugin");
	assert!(
		lab.host
			.menu_gesture(CLONER, "unknown", Slot::MessageMenu, Some(emoji()), &[])
			.is_err(),
		"only a key the manifest declares is a menu item"
	);
	assert!(heard.lock().is_empty(), "nothing to clone was clicked");
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clone_runs_only_from_a_menu_click_and_is_rate_limited() {
	let root = scratch("cloner-gesture");
	write_plugin(
		&root,
		"sneaky",
		&["hook:send", "ui:message.menu", "action:upload_expression"],
		SNEAKY_MENU,
		SNEAKY,
	);
	let limits = Limits {
		actions: 2,
		action_window: Duration::from_secs(60),
		..Limits::default()
	};
	let lab = start("cloner-gesture", root, limits, false).await;
	approve(&lab.host, "sneaky");
	let heard = listen(&lab.host);
	let channel_id = lab.running.open_text_channel().await;
	lab.running
		.send_and_wait(channel_id, "one", "one send=refused")
		.await;
	lab.host
		.gesture("sneaky", "button", &[])
		.expect("the click reaches the plugin");
	for _ in 0..3 {
		lab.host
			.menu_gesture("sneaky", "grab", Slot::MessageMenu, Some(emoji()), &[])
			.expect("the menu click reaches the plugin");
	}
	lab.running
		.send_and_wait(channel_id, "two", "two button=refused grab=refused send=refused")
		.await;
	tokio::time::sleep(QUIET).await;
	assert_eq!(heard.lock().len(), 2, "two clones, then the per-plugin limit");
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clone_needs_the_upload_permission() {
	let root = scratch("cloner-permission");
	write_plugin(
		&root,
		"unpermitted",
		&["hook:send", "ui:message.menu"],
		SNEAKY_MENU,
		SNEAKY,
	);
	let lab = start("cloner-permission", root, Limits::default(), false).await;
	approve(&lab.host, "unpermitted");
	let heard = listen(&lab.host);
	let channel_id = lab.running.open_text_channel().await;
	lab.host
		.menu_gesture("unpermitted", "grab", Slot::MessageMenu, Some(emoji()), &[])
		.expect("the menu click reaches the plugin");
	lab.running
		.send_and_wait(channel_id, "one", "one grab=refused send=refused")
		.await;
	assert!(heard.lock().is_empty());
	lab.stop().await;
}
