use std::time::Duration;

use plugins::{Limits, Status};
use session::Command;

use crate::support::{approve, scratch, start, write_plugin};

const SHOUT: &str = "return { before_send = function(message) return string.upper(message.content) end }";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_hook_without_its_permission_never_runs() {
	let root = scratch("ungranted-hook");
	write_plugin(&root, "shouty", &["storage"], "", SHOUT);
	let lab = start("ungranted-hook", root, Limits::default(), false).await;
	approve(&lab.host, "shouty");
	assert_eq!(lab.host.status("shouty"), Some(Status::Running));
	let channel_id = lab.running.open_text_channel().await;
	lab.running
		.send_and_wait(channel_id, "quiet please", "quiet please")
		.await;
	lab.stop().await;
}

const SHOUT_BOTH: &str = "local function shout(message) return string.upper(message.content) end\nreturn { before_send = shout, before_edit = shout }";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sends_and_edits_are_rewritten_only_with_their_own_permission() {
	let root = scratch("edit-permission");
	write_plugin(&root, "sender", &["hook:send"], "", SHOUT_BOTH);
	let lab = start("edit-permission", root, Limits::default(), false).await;
	approve(&lab.host, "sender");
	let channel_id = lab.running.open_text_channel().await;
	let message_id = lab
		.running
		.send_and_wait(channel_id, "sent loud", "SENT LOUD")
		.await;
	lab.edit_and_wait(channel_id, message_id, "edited quietly", "edited quietly")
		.await;

	write_plugin(
		&lab.root,
		"sender",
		&["hook:edit"],
		"description = \"edits\"",
		SHOUT_BOTH,
	);
	lab.host.scan();
	assert_eq!(lab.host.status("sender"), Some(Status::NeedsApproval));
	approve(&lab.host, "sender");
	lab.running
		.send_and_wait(channel_id, "sent quietly", "sent quietly")
		.await;
	lab.edit_and_wait(channel_id, message_id, "edited loud", "EDITED LOUD")
		.await;
	lab.running
		.until("the edit to be marked as rewritten", |state| {
			state.altered_by(message_id) == Some("sender")
		})
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_apis_refuse_what_was_not_granted_and_shared_data_is_read_only() {
	let root = scratch("granted-apis");
	write_plugin(
		&root,
		"asker",
		&["hook:send"],
		"",
		r#"return {
			before_send = function(message)
				local stored = pcall(kyoko.storage.set, "k", 1)
				local read = pcall(kyoko.state.current_user)
				local acted = pcall(kyoko.actions.mark_all_read)
				return message.content .. " storage=" .. tostring(stored) .. " state=" .. tostring(read) .. " action=" .. tostring(acted)
			end,
		}"#,
	);
	write_plugin(
		&root,
		"reader",
		&["hook:send", "read:users", "storage"],
		"",
		r#"return {
			before_send = function(message)
				if not string.find(message.content, "^me") then
					return nil
				end
				local me = kyoko.state.current_user()
				me.username = "renamed"
				kyoko.storage.set("seen", me.username)
				return "me " .. kyoko.state.current_user().username .. " " .. kyoko.storage.get("seen") .. string.sub(message.content, 3)
			end,
		}"#,
	);
	let lab = start("granted-apis", root, Limits::default(), false).await;
	approve(&lab.host, "asker");
	approve(&lab.host, "reader");
	let channel_id = lab.running.open_text_channel().await;
	lab.running
		.send_and_wait(
			channel_id,
			"hello",
			"hello storage=false state=false action=false",
		)
		.await;
	let username = lab
		.running
		.session()
		.state()
		.read()
		.current_user()
		.map(|user| user.username.clone())
		.expect("signed in");
	lab.running
		.send_and_wait(
			channel_id,
			"me",
			&format!("me {username} renamed storage=false state=false action=false"),
		)
		.await;
	assert_eq!(
		lab.running
			.session()
			.state()
			.read()
			.current_user()
			.map(|user| user.username.clone()),
		Some(username),
		"a plugin's write to its copy never reaches kyoko's state"
	);
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_typing_veto_stops_typing_traffic_until_the_plugin_is_off() {
	let root = scratch("typing-veto");
	write_plugin(
		&root,
		"silent",
		&["hook:typing"],
		"",
		"return { allow_typing = function() return false end }",
	);
	let lab = start("typing-veto", root, Limits::default(), false).await;
	approve(&lab.host, "silent");
	let channel_id = lab.running.open_text_channel().await;
	lab.running.session().send(Command::Typing { channel_id });
	tokio::time::sleep(Duration::from_millis(600)).await;
	let typed = |lab: &crate::support::Lab| {
		lab.running
			.mock()
			.recording()
			.requests_to("POST", "/channels/*/typing")
			.count()
	};
	assert_eq!(typed(&lab), 0, "no typing traffic while the veto holds");

	lab.host.disable("silent");
	let other = lab
		.running
		.session()
		.state()
		.read()
		.guilds()
		.flat_map(|guild| guild.channels.values())
		.find(|channel| channel.id != channel_id && channel.kind == protocol::ChannelType::GuildText)
		.map(|channel| channel.id)
		.expect("a second text channel");
	lab.running.session().send(Command::Typing { channel_id: other });
	lab.running
		.until_wire("typing once the plugin is off", |recording| {
			(recording.requests_to("POST", "/channels/*/typing").count() > 0).then_some(())
		})
		.await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn asking_for_more_permissions_needs_approval_again() {
	let root = scratch("more-permissions");
	write_plugin(&root, "grower", &["hook:send"], "", SHOUT);
	let lab = start("more-permissions", root.clone(), Limits::default(), false).await;
	approve(&lab.host, "grower");
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "one", "ONE").await;

	write_plugin(&root, "grower", &["hook:send", "storage"], "", SHOUT);
	assert!(lab.host.scan(), "the changed files are noticed");
	assert_eq!(lab.host.status("grower"), Some(Status::NeedsApproval));
	lab.running.send_and_wait(channel_id, "two", "two").await;

	approve(&lab.host, "grower");
	lab.running.send_and_wait(channel_id, "three", "THREE").await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edited_plugin_files_reload_while_kyoko_runs() {
	let root = scratch("hot-reload");
	write_plugin(&root, "live", &["hook:send"], "", SHOUT);
	let lab = start("hot-reload", root.clone(), Limits::default(), false).await;
	approve(&lab.host, "live");
	lab.host.watch(Duration::from_millis(50));
	let channel_id = lab.running.open_text_channel().await;
	lab.running.send_and_wait(channel_id, "before", "BEFORE").await;
	write_plugin(
		&root,
		"live",
		&["hook:send"],
		"description = \"edited\"",
		"return { before_send = function(message) return message.content .. \" (edited live)\" end }",
	);
	lab.running
		.eventually("the edit to be picked up", || {
			lab.host.snapshot().iter().any(|plugin| {
				plugin.id == "live" && plugin.description == "edited" && plugin.status == Status::Running
			})
		})
		.await;
	lab.running
		.send_and_wait(channel_id, "after", "after (edited live)")
		.await;
	lab.stop().await;
}
