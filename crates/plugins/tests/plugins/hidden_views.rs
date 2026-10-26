use std::time::{Duration, Instant};

use plugins::{Limits, PluginHost, Slot, Status};

use crate::support::{approve, config, scratch, write_plugin};

const CHANNELS: &str = "show-hidden-channels";
const THINGS: &str = "show-hidden-things";
const WAIT: Duration = Duration::from_secs(15);

async fn until(what: &str, done: impl Fn() -> bool) {
	let deadline = Instant::now() + WAIT;
	while !done() {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		tokio::time::sleep(Duration::from_millis(20)).await;
	}
}

fn shown(host: &PluginHost) -> (bool, bool) {
	(host.shows(Slot::HiddenChannels), host.shows(Slot::ModeratorNotes))
}

#[tokio::test(flavor = "multi_thread")]
async fn hidden_views_turn_on_only_while_a_granted_plugin_runs() {
	let root = scratch("hidden-views");
	write_plugin(&root, "asks-nothing", &[], "", "return {}");
	let host = PluginHost::new(config(&root, Limits::default(), true));
	for id in [CHANNELS, THINGS] {
		assert_eq!(host.status(id), Some(Status::Disabled), "{id} ships disabled");
	}
	assert_eq!(shown(&host), (false, false));

	approve(&host, "asks-nothing");
	until("the plugin without the slots to run", || {
		host.status("asks-nothing") == Some(Status::Running)
	})
	.await;
	assert_eq!(shown(&host), (false, false), "running alone grants nothing");

	approve(&host, CHANNELS);
	until("ShowHiddenChannels to run", || {
		host.status(CHANNELS) == Some(Status::Running)
	})
	.await;
	assert_eq!(shown(&host), (true, false));

	approve(&host, THINGS);
	until("ShowHiddenThings to run", || {
		host.status(THINGS) == Some(Status::Running)
	})
	.await;
	assert_eq!(shown(&host), (true, true));

	host.disable(CHANNELS);
	assert_eq!(shown(&host), (false, true), "disabling hides the channels again");
	host.revoke(THINGS);
	assert_eq!(shown(&host), (false, false), "revoking hides the notes again");
	let _ = std::fs::remove_dir_all(&root);
}
