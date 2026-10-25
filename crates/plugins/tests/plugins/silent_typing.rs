use std::time::Duration;

use plugins::{EnableError, Limits, Permission, Slot, Status};
use protocol::{ChannelType, Snowflake};
use serde_json::json;
use session::Command;

use crate::support::{Lab, scratch, start};

const ID: &str = "silent-typing";
const QUIET: Duration = Duration::from_millis(600);

fn typed(lab: &Lab) -> usize {
	lab.running
		.mock()
		.recording()
		.requests_to("POST", "/channels/*/typing")
		.count()
}

fn text_channels(lab: &Lab) -> Vec<Snowflake> {
	lab.running
		.session()
		.state()
		.read()
		.guilds()
		.flat_map(|guild| guild.channels.values())
		.filter(|channel| channel.kind == ChannelType::GuildText)
		.map(|channel| channel.id)
		.collect()
}

async fn type_in(lab: &Lab, channel_id: Snowflake) {
	for _ in 0..3 {
		lab.running.session().send(Command::Typing { channel_id });
	}
	tokio::time::sleep(QUIET).await;
}

async fn types_once_more(lab: &Lab, channel_id: Snowflake, before: usize, what: &str) {
	lab.running.session().send(Command::Typing { channel_id });
	lab.running
		.until_wire(what, |recording| {
			(recording.requests_to("POST", "/channels/*/typing").count() > before).then_some(())
		})
		.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn silent_typing_sends_no_typing_while_on_and_types_again_once_off() {
	let lab = start("silent-typing", scratch("silent-typing"), Limits::default(), true).await;
	let first = lab.running.open_text_channel().await;
	let channels = text_channels(&lab);
	let others: Vec<Snowflake> = channels.into_iter().filter(|id| *id != first).collect();
	assert!(others.len() >= 3, "the fixtures have four text channels");

	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let asked = vec![
		Permission::Ui(Slot::ComposerButtons),
		Permission::parse("hook:typing"),
	];
	assert_eq!(
		lab.host.enable(ID),
		Err(EnableError::NeedsApproval(asked.clone()))
	);
	lab.host.approve(ID, &asked).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));

	type_in(&lab, first).await;
	assert_eq!(
		typed(&lab),
		0,
		"no typing request reaches the wire while it is on"
	);

	lab.host
		.set_setting(ID, "silent", &json!(false))
		.expect("the composer button's setting is accepted");
	types_once_more(&lab, others[0], 0, "typing once the toggle is off").await;
	assert_eq!(typed(&lab), 1);

	lab.host
		.set_setting(ID, "silent", &json!(true))
		.expect("accepted");
	type_in(&lab, others[1]).await;
	assert_eq!(typed(&lab), 1, "switched back on, typing stops again");

	lab.host.disable(ID);
	types_once_more(&lab, others[2], 1, "typing once the plugin is off").await;
	lab.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bundled_interface_half_is_unpacked_and_remounted_on_each_start() {
	let root = scratch("silent-typing-ui");
	let lab = start("silent-typing-ui", root.clone(), Limits::default(), true).await;
	let info = |lab: &Lab| {
		lab.host
			.snapshot()
			.into_iter()
			.find(|plugin| plugin.id == ID)
			.expect("SilentTyping is bundled")
	};
	assert!(
		info(&lab).ui_entry.is_none(),
		"nothing is unpacked for a plugin that is off"
	);
	lab.host.approve(ID, &info(&lab).permissions).expect("approved");
	let running = info(&lab);
	let unpacked = root.join("data").join("plugin-ui").join(ID);
	assert_eq!(running.ui_entry, Some(unpacked.join("ui.luau")));
	for file in ["ui.luau", "keyboard.svg", "keyboard_off.svg"] {
		assert!(unpacked.join(file).is_file(), "{file} is unpacked");
	}
	for file in ["plugin.toml", "logic.luau"] {
		assert!(!unpacked.join(file).exists(), "{file} stays in the binary");
	}
	assert!(running.grants.contains(&Permission::Ui(Slot::ComposerButtons)));

	lab.host.reload(ID);
	assert!(
		info(&lab).generation > running.generation,
		"a restart tells the interface to mount the plugin again"
	);
	lab.stop().await;
}
