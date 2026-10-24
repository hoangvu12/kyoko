use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, bail};
use protocol::{DispatchEvent, Snowflake};
use session::{Command, SessionHooks};

use crate::harness::{Options, Running};

const HOOK_NAME: &str = "test.rewriter";

struct Rewriter;

impl SessionHooks for Rewriter {
	fn name(&self) -> &str {
		HOOK_NAME
	}

	fn before_send(&self, _channel_id: Snowflake, content: &str) -> Result<Option<String>> {
		Ok(Some(content.to_uppercase()))
	}
}

#[derive(Default)]
struct Broken {
	dispatches: AtomicUsize,
}

impl SessionHooks for Broken {
	fn before_send(&self, _channel_id: Snowflake, _content: &str) -> Result<Option<String>> {
		panic!("broken before_send");
	}

	fn before_edit(
		&self,
		_channel_id: Snowflake,
		_message_id: Snowflake,
		_content: &str,
	) -> Result<Option<String>> {
		bail!("broken before_edit")
	}

	fn allow_typing(&self, _channel_id: Snowflake) -> Result<bool> {
		panic!("broken allow_typing");
	}

	fn allow_notification(&self, _message: &protocol::Message) -> Result<bool> {
		bail!("broken allow_notification")
	}

	fn on_dispatch(&self, _event: &DispatchEvent) -> Result<()> {
		self.dispatches.fetch_add(1, Ordering::Relaxed);
		panic!("broken on_dispatch");
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_send_rewrites_the_outgoing_message() {
	let running = Running::start_with(Options::new("rewrite").hooks(Arc::new(Rewriter))).await;
	let channel_id = running.open_text_channel().await;
	let message_id = running
		.send_and_wait(channel_id, "hooked message", "HOOKED MESSAGE")
		.await;
	assert_eq!(
		running.session().state().read().altered_by(message_id),
		Some(HOOK_NAME)
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_hooks_never_stop_the_session() {
	let broken = Arc::new(Broken::default());
	let running = Running::start_with(Options::new("broken").hooks(broken.clone())).await;
	let channel_id = running.open_text_channel().await;
	assert!(broken.dispatches.load(Ordering::Relaxed) > 0);
	running.session().send(Command::Typing { channel_id });
	let message_id = running
		.send_and_wait(channel_id, "plain message", "plain message")
		.await;
	assert_eq!(running.session().state().read().altered_by(message_id), None);
	let message = running
		.session()
		.state()
		.read()
		.messages(channel_id)
		.and_then(|cache| cache.by_id.get(&message_id).cloned())
		.expect("message is cached");
	assert!(running.session().should_notify(&message));
	running.session().send(Command::EditMessage {
		channel_id,
		message_id,
		content: "edited message".to_owned(),
	});
	running
		.until("the edit to arrive unmodified", |state| {
			state
				.messages(channel_id)
				.and_then(|cache| cache.by_id.get(&message_id))
				.is_some_and(|message| message.content == "edited message")
		})
		.await;
	running
		.until("the session to stay connected", |state| state.session().connected)
		.await;
	running.stop().await;
}
