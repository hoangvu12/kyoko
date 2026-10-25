use plugins::{Limits, PluginHost, Status};
use serde_json::json;

use crate::support::{approve, config, scratch, start, write_plugin};

const SETTINGS: &str = r#"
[[settings]]
key = "suffix"
type = "string"
label = "Suffix"
default = "!"
"#;

const COUNTER: &str = r#"
return {
	before_send = function(message)
		local count = (kyoko.storage.get("count") or 0) + 1
		kyoko.storage.set("count", count)
		return message.content .. kyoko.settings.get("suffix") .. count
	end,
}
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_and_settings_belong_to_the_plugin_and_survive_reloads() {
	let root = scratch("storage");
	write_plugin(&root, "counter", &["hook:send", "storage"], SETTINGS, COUNTER);
	let lab = start("storage", root.clone(), Limits::default(), false).await;
	approve(&lab.host, "counter");
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "a", "a!1").await;
	lab.host
		.set_setting("counter", "suffix", &json!("?"))
		.expect("a valid value");
	assert!(lab.host.set_setting("counter", "suffix", &json!(3)).is_err());
	assert!(lab.host.set_setting("counter", "missing", &json!("x")).is_err());
	lab.running.send_and_wait(channel_id, "b", "b?2").await;
	lab.host.reload("counter");
	lab.running.send_and_wait(channel_id, "c", "c?3").await;

	let again = PluginHost::new(config(&root, Limits::default(), false));
	let plugin = again
		.snapshot()
		.into_iter()
		.find(|plugin| plugin.id == "counter")
		.expect("listed after a restart");
	assert!(plugin.enabled && !plugin.needs_approval);
	assert_eq!(plugin.status, Status::Running);
	assert_eq!(plugin.values.get("suffix"), Some(&json!("?")));
	drop(again);
	lab.stop().await;
}
