use std::cell::RefCell;
use std::time::Duration;

use bridge::{AlertOutlet, ToastText, raise_alerts};
use protocol::{ChannelType, Snowflake};
use session::Alert;
use store::Settings;
use tokio::time::Instant;

use crate::harness::Running;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);

#[derive(Default)]
struct Outlet {
	chimes: RefCell<usize>,
	toasts: RefCell<Vec<ToastText>>,
}

impl AlertOutlet for Outlet {
	fn chime(&self) {
		*self.chimes.borrow_mut() += 1;
	}

	fn toast(&self, _: &Alert, text: ToastText) {
		self.toasts.borrow_mut().push(text);
	}
}

impl Outlet {
	fn heard(running: &Running, alerts: &[Alert], settings: &Settings) -> (usize, Vec<String>) {
		let outlet = Outlet::default();
		raise_alerts(&running.session().state().read(), settings, alerts, &outlet);
		let bodies = outlet.toasts.take().into_iter().map(|toast| toast.body).collect();
		(outlet.chimes.take(), bodies)
	}
}

fn direct_message(running: &Running) -> (Snowflake, u64) {
	let shared = running.session().state().clone();
	let state = shared.read();
	let blocked: Vec<Snowflake> = state.blocked().iter().map(|row| row.user_id).collect();
	state
		.private_channels()
		.filter(|channel| channel.kind == ChannelType::Dm)
		.find_map(|channel| {
			let author = *channel.recipient_ids.first()?;
			(!blocked.contains(&author)).then_some((channel.id, author.get()))
		})
		.expect("a direct message with someone not blocked")
}

async fn two_alerts(running: &Running) -> Vec<Alert> {
	let (channel_id, author) = direct_message(running);
	for content in ["first ping", "second ping"] {
		running
			.server
			.control()
			.post_message(channel_id.get(), author, content)
			.expect("mockcord posts the message");
	}
	let deadline = Instant::now() + WAIT;
	let mut alerts = Vec::new();
	while alerts.len() < 2 {
		assert!(Instant::now() < deadline, "timed out waiting for two alerts");
		alerts.extend(running.session().take_alerts());
		tokio::time::sleep(POLL).await;
	}
	alerts
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alerts_chime_once_and_toast_each_message_as_the_settings_say() {
	let running = Running::start("alerts-chime").await;
	let alerts = two_alerts(&running).await;
	let pings = vec!["first ping".to_owned(), "second ping".to_owned()];

	let settings = Settings::default();
	assert_eq!(Outlet::heard(&running, &alerts, &settings), (1, pings.clone()));
	assert_eq!(Outlet::heard(&running, &[], &settings), (0, Vec::new()));

	let mut quiet = Settings::default();
	quiet.notifications.sounds = false;
	assert_eq!(Outlet::heard(&running, &alerts, &quiet), (0, pings.clone()));

	let mut no_popups = Settings::default();
	no_popups.notifications.enabled = false;
	assert_eq!(Outlet::heard(&running, &alerts, &no_popups), (1, Vec::new()));

	let mut streaming = Settings::default();
	streaming.streamer_mode.enabled = true;
	streaming.streamer_mode.disable_sounds = true;
	assert_eq!(Outlet::heard(&running, &alerts, &streaming), (0, pings));

	let mut hidden = Settings::default();
	hidden.notifications.show_content = false;
	assert_eq!(
		Outlet::heard(&running, &alerts, &hidden),
		(1, vec!["Sent a message".to_owned(); 2])
	);
}
