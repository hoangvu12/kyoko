use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use anyhow::Result;
use protocol::{DispatchEvent, Message, Snowflake};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
	pub content: String,
	pub by: Option<String>,
}

impl Rewrite {
	pub fn by(content: String, by: impl Into<String>) -> Self {
		Self {
			content,
			by: Some(by.into()),
		}
	}
}

impl From<String> for Rewrite {
	fn from(content: String) -> Self {
		Self { content, by: None }
	}
}

pub trait SessionHooks: Send + Sync {
	fn name(&self) -> &str {
		"hooks"
	}

	fn before_send(&self, _channel_id: Snowflake, _content: &str) -> Result<Option<Rewrite>> {
		Ok(None)
	}

	fn before_edit(
		&self,
		_channel_id: Snowflake,
		_message_id: Snowflake,
		_content: &str,
	) -> Result<Option<String>> {
		Ok(None)
	}

	fn allow_typing(&self, _channel_id: Snowflake) -> Result<bool> {
		Ok(true)
	}

	fn allow_notification(&self, _message: &Message) -> Result<bool> {
		Ok(true)
	}

	fn on_dispatch(&self, _event: &DispatchEvent) -> Result<()> {
		Ok(())
	}
}

pub(crate) type Hooks = Option<Arc<dyn SessionHooks>>;

pub(crate) fn guarded<T>(
	hooks: &Hooks,
	hook: &'static str,
	call: impl FnOnce(&dyn SessionHooks) -> Result<T>,
) -> Option<T> {
	let hooks = hooks.as_deref()?;
	match catch_unwind(AssertUnwindSafe(|| call(hooks))) {
		Ok(Ok(value)) => Some(value),
		Ok(Err(_)) => {
			tracing::warn!(hook, owner = hooks.name(), "session hook failed");
			None
		}
		Err(_) => {
			tracing::warn!(hook, owner = hooks.name(), "session hook panicked");
			None
		}
	}
}

pub(crate) async fn guarded_blocking<T: Send + 'static>(
	hooks: &Hooks,
	hook: &'static str,
	call: impl FnOnce(&dyn SessionHooks) -> Result<T> + Send + 'static,
) -> Option<T> {
	let hooks = hooks.clone()?;
	tokio::task::spawn_blocking(move || guarded(&Some(hooks), hook, call))
		.await
		.ok()
		.flatten()
}
