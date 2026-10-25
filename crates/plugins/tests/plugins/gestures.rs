use std::time::Duration;

use plugins::Limits;

use crate::support::{approve, scratch, start, write_plugin};

const MARKER: &str = r#"
local outcomes = {}

local function try(where)
	local ok = pcall(kyoko.actions.mark_all_read)
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

fn acks(lab: &crate::support::Lab) -> usize {
	lab.running
		.mock()
		.recording()
		.requests_to("POST", "/read-states/ack-bulk")
		.count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_run_only_inside_a_gesture_and_are_rate_limited() {
	let root = scratch("gestures");
	write_plugin(&root, "marker", &["hook:send", "action:mark_read"], "", MARKER);
	let limits = Limits {
		actions: 1,
		action_window: Duration::from_secs(60),
		..Limits::default()
	};
	let lab = start("gestures", root, limits, false).await;
	approve(&lab.host, "marker");
	let channel_id = lab.running.open_text_channel().await;
	lab.running
		.send_and_wait(channel_id, "one", "one send=refused")
		.await;
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert_eq!(
		acks(&lab),
		0,
		"a hook is not a gesture, so nothing was marked read"
	);

	lab.host
		.gesture("marker", "first", &[])
		.expect("the click reaches the plugin");
	lab.running
		.until_wire("the bulk ack from the click", |recording| {
			(recording.requests_to("POST", "/read-states/ack-bulk").count() > 0).then_some(())
		})
		.await;
	lab.host
		.gesture("marker", "second", &[])
		.expect("the click reaches the plugin");
	lab.running
		.send_and_wait(channel_id, "two", "two first=ok second=refused send=refused")
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_action_without_its_permission_is_refused_even_in_a_gesture() {
	let root = scratch("gesture-ungranted");
	write_plugin(&root, "marker", &["hook:send"], "", MARKER);
	let lab = start("gesture-ungranted", root, Limits::default(), false).await;
	approve(&lab.host, "marker");
	let channel_id = lab.running.open_text_channel().await;
	lab.host
		.gesture("marker", "click", &[])
		.expect("the click reaches the plugin");
	lab.running
		.send_and_wait(channel_id, "check", "check click=refused send=refused")
		.await;
	assert_eq!(acks(&lab), 0);
	lab.stop().await;
}
