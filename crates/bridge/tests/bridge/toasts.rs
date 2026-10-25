use std::time::{Duration, Instant};

use bridge::{TOAST_EXIT, TOAST_LIFETIME, TOAST_LIMIT, ToastKind, Toasts};
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

const MISSING_CHANNEL: Snowflake = Snowflake(1);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_command_becomes_one_error_toast_that_expires() {
	let mut running = Running::start("toasts").await;
	let mut toasts = Toasts::default();
	let state = running.session().state().clone();
	assert_eq!(toasts.new_rejection(&state.read()), None);

	running.session().send(Command::DeleteChannel {
		channel_id: MISSING_CHANNEL,
	});
	running
		.until_state("the command to be refused", |state| {
			state.last_rejection().is_some()
		})
		.await;
	let reason = toasts.new_rejection(&state.read()).expect("the refusal is new");
	assert_eq!(reason, "This only works in a server");
	assert_eq!(toasts.new_rejection(&state.read()), None);

	let shown = Instant::now();
	let id = toasts.push(ToastKind::Error, &reason, shown);
	assert_eq!(toasts.push(ToastKind::Error, &reason, shown), id);
	assert_eq!(toasts.items().len(), 1);
	assert_eq!(toasts.items()[0].kind, ToastKind::Error);
	assert_eq!(toasts.items()[0].text, reason);

	toasts.pause(shown + Duration::from_secs(1));
	assert!(!toasts.tick(shown + TOAST_LIFETIME * 2));
	toasts.resume(shown + TOAST_LIFETIME * 2);
	assert!(toasts.tick(shown + TOAST_LIFETIME * 3));
	assert!(toasts.items()[0].leaving);
	assert!(toasts.tick(shown + TOAST_LIFETIME * 3 + TOAST_EXIT));
	assert!(toasts.items().is_empty());
	assert_eq!(toasts.next_deadline(), None);

	for index in 0..=TOAST_LIMIT {
		toasts.push(ToastKind::Info, &format!("note {index}"), shown);
	}
	let staying = toasts.items().iter().filter(|toast| !toast.leaving).count();
	assert_eq!(staying, TOAST_LIMIT);
	running.stop().await;
}
