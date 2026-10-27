use std::sync::Arc;
use std::time::Duration;

use mockcord::Recording;
use parking_lot::Mutex;
use plugins::{Action, EnableError, Limits, Permission, PluginHost, PluginRequest, Slot, Status};
use protocol::{ProtoStatusSettings, decode_status_settings};
use session::RestTuning;
use state::State;

use crate::harness::Options;
use crate::support::{Lab, scratch, start_with};

const ID: &str = "game-activity-toggle";
const SETTINGS: &str = "/users/@me/settings-proto/1";

fn writes(recording: &Recording) -> Vec<ProtoStatusSettings> {
	recording
		.requests_to("PATCH", SETTINGS)
		.filter_map(|request| decode_status_settings(request.json()?["settings"].as_str()?))
		.collect()
}

fn listen(host: &PluginHost) -> Arc<Mutex<Vec<PluginRequest>>> {
	let heard = Arc::new(Mutex::new(Vec::new()));
	let sink = heard.clone();
	host.on_request(move |request| sink.lock().push(request));
	heard
}

fn slot_click() -> Vec<(String, String)> {
	vec![
		("source".to_owned(), "slot".to_owned()),
		("slot".to_owned(), "panel.buttons".to_owned()),
	]
}

async fn click_until(lab: &Lab, shared: bool, written: usize) {
	lab.host
		.gesture(ID, "toggle", &slot_click())
		.expect("the click runs the plugin");
	lab.running
		.until("the sharing flipped at once", |state: &State| {
			state.status_settings().shares_activity() == shared
		})
		.await;
	let saved = lab
		.running
		.until_wire("the settings write", |recording| {
			writes(recording).into_iter().nth(written)
		})
		.await;
	assert_eq!(saved.show_current_game, Some(shared));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_panel_button_flips_show_current_game_through_the_settings_proto() {
	let tuning = RestTuning {
		mutation_gap: Duration::ZERO,
		settings_delay: Duration::from_millis(100),
		settings_gap: Duration::from_millis(300),
		..RestTuning::default()
	};
	let name = "game-activity-toggle";
	let lab = start_with(
		Options::new(name).rest(tuning),
		scratch(name),
		Limits::default(),
		true,
	)
	.await;
	lab.running.connected().await;
	let heard = listen(&lab.host);

	assert_eq!(lab.host.status(ID), Some(Status::Disabled), "it ships disabled");
	let asked = vec![
		Permission::Ui(Slot::PanelButtons),
		Permission::Action(Action::Activity),
	];
	assert_eq!(
		lab.host.enable(ID),
		Err(EnableError::NeedsApproval(asked.clone()))
	);
	lab.host.approve(ID, &asked).expect("approved");
	assert_eq!(lab.host.status(ID), Some(Status::Running));
	assert!(
		lab.running
			.session()
			.state()
			.read()
			.status_settings()
			.shares_activity()
	);

	lab.host
		.gesture(ID, "toggle", &[("source".to_owned(), "settings".to_owned())])
		.expect("a click elsewhere runs the plugin too");
	click_until(&lab, false, 0).await;
	click_until(&lab, true, 1).await;
	assert_eq!(
		heard.lock().as_slice(),
		[
			PluginRequest::ActivitySharing {
				plugin: ID.to_owned(),
				shared: false,
			},
			PluginRequest::ActivitySharing {
				plugin: ID.to_owned(),
				shared: true,
			},
		],
		"each click asks the window for its toast, and nothing else does"
	);
	tokio::time::sleep(Duration::from_millis(800)).await;
	assert_eq!(
		writes(&lab.running.mock().recording()).len(),
		2,
		"one write per click"
	);
	lab.stop().await;
}
