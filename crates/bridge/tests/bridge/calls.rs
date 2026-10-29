use bridge::{SystemIcon, incoming_call};
use protocol::{ChannelType, Snowflake, StatusKind};
use session::Command;

use crate::harness::{Running, View};

fn direct(running: &Running, index: usize) -> (Snowflake, Snowflake) {
	let state = running.session().state().read();
	running
		.server
		.fixtures()
		.private_channel_ids
		.iter()
		.filter_map(|id| state.channel(Snowflake(*id)))
		.filter(|channel| channel.kind == ChannelType::Dm)
		.nth(index)
		.map(|channel| (channel.id, channel.recipient_ids[0]))
		.expect("a direct message in the fixtures")
}

fn call_line(view: &View) -> Option<(SystemIcon, String)> {
	view.timeline
		.messages()
		.rev()
		.find_map(|entry| entry.system.as_ref())
		.map(|line| (line.icon, line.text.clone()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_incoming_call_rings_with_sound_until_do_not_disturb_and_marks_the_dm() {
	let mut running = Running::start("calls-incoming").await;
	let (dm, caller) = direct(&running, 0);
	let name = running.session().state().read().display_name(None, caller);
	assert!(running.server.control().ring_me(dm.get(), caller.get()));
	running
		.until_state("the incoming call", |state| {
			incoming_call(state).is_some_and(|call| {
				call.channel_id == dm && call.caller == Some(caller) && call.name == name && !call.group
			})
		})
		.await;
	assert!(
		incoming_call(&running.session().state().read()).is_some_and(|call| call.ring_sound),
		"a call rings out loud"
	);
	running
		.until("the DM to show the call", |view| {
			view.dms.entry(dm).is_some_and(|entry| entry.in_call)
		})
		.await;
	running.session().send(Command::SetStatus(StatusKind::Dnd));
	running
		.until_state("do not disturb to silence the ring", |state| {
			incoming_call(state).is_some_and(|call| !call.ring_sound)
		})
		.await;
	running.session().send(Command::DeclineCall { channel_id: dm });
	running
		.until_state("declining to close it", |state| incoming_call(state).is_none())
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_messages_say_who_called_and_how_long_it_lasted() {
	let mut running = Running::start("calls-messages").await;
	let (missed, caller) = direct(&running, 0);
	let name = running.session().state().read().display_name(None, caller);
	let select = {
		let state = running.session().state().clone();
		running.view.timeline.select(&state.read(), None, missed)
	};
	running.session().send(select.expect("selecting the DM"));
	running
		.until("the DM history", |view| view.timeline.messages().next().is_some())
		.await;
	let control = running.server.control();
	assert!(control.ring_me(missed.get(), caller.get()));
	running
		.until("the running call's message", |view| {
			call_line(view) == Some((SystemIcon::Call, format!("{name} started a call.")))
		})
		.await;
	control.call_leave_as(missed.get(), caller.get());
	running
		.until("the missed call", |view| {
			call_line(view)
				== Some((
					SystemIcon::MissedCall,
					format!("You missed a call from {name} that lasted a few seconds."),
				))
		})
		.await;
	assert!(control.ring_me(missed.get(), caller.get()));
	control.answer_elsewhere(missed.get());
	control.call_leave_as(missed.get(), caller.get());
	control.call_leave_as(missed.get(), mockcord::fixtures::CURRENT_USER_ID);
	running
		.until("a call you took part in", |view| {
			call_line(view)
				== Some((
					SystemIcon::Call,
					format!("{name} started a call that lasted a few seconds."),
				))
		})
		.await;
	running.stop().await;
}
