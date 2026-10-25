use std::sync::Arc;

use state::{LocalEvent, SecretStorage};
use store::{Retention, SecretHealth, Store};
use tokio::task::JoinHandle;

use super::{Core, Internal};

#[derive(Debug, Clone, Copy)]
pub(super) struct SignOut {
	user_id: Option<protocol::Snowflake>,
}

pub(super) fn secret_storage(health: SecretHealth) -> SecretStorage {
	SecretStorage {
		plaintext: health.plaintext,
		keyring_error: health.keyring_error.map(str::to_owned),
		cache_in_memory: health.cache_in_memory,
	}
}

async fn prune_periodically(store: Arc<Store>, retention: Retention) {
	loop {
		let store = store.clone();
		let pruned = tokio::task::spawn_blocking(move || store.prune(&retention)).await;
		match pruned {
			Ok(Ok(pruned)) => tracing::debug!(
				messages = pruned.messages,
				media_files = pruned.media_files,
				media_bytes = pruned.media_bytes,
				"pruned the local cache"
			),
			Ok(Err(error)) => tracing::warn!(%error, "could not prune the local cache"),
			Err(_) => tracing::warn!("cache pruning panicked"),
		}
		tokio::time::sleep(retention.interval).await;
	}
}

pub(super) async fn finish_sign_out(store: Arc<Store>, sign_out: Option<SignOut>) {
	let Some(SignOut { user_id }) = sign_out else {
		return;
	};
	let wiped = tokio::task::spawn_blocking(move || store.sign_out(user_id)).await;
	match wiped {
		Ok(Ok(())) => tracing::info!("signed out and wiped the account's local data"),
		Ok(Err(error)) => tracing::warn!(%error, "sign-out did not wipe everything"),
		Err(_) => tracing::warn!("sign-out wipe panicked"),
	}
}

impl Core {
	pub(super) fn publish_secret_health(&mut self, health: SecretHealth) {
		self.apply_local(LocalEvent::SecretStorage(secret_storage(health)));
	}

	pub(super) fn start_pruning(&self) -> JoinHandle<()> {
		self.runtime
			.spawn(prune_periodically(self.store.clone(), self.retention))
	}

	pub(super) fn begin_sign_out(&mut self) {
		*self.signed_in.lock() = false;
		self.sign_out = Some(SignOut {
			user_id: self.state.read().current_user_id(),
		});
		self.rest.clear_token();
	}

	pub(super) fn forget_rejected_token(&self) {
		let store = self.store.clone();
		let token = self.token.clone();
		let signed_in = self.signed_in.clone();
		let user_id = self.state.read().current_user_id();
		let internal = self.internal.clone();
		self.runtime.spawn_blocking(move || {
			let mut signed_in = signed_in.lock();
			*signed_in = false;
			if let Err(error) = store.tokens.forget(token.as_str()) {
				tracing::warn!(%error, "the rejected token was not forgotten");
			}
			if let Some(user_id) = user_id
				&& let Err(error) = store.accounts.forget_token(user_id, token.as_str())
			{
				tracing::warn!(%error, "the rejected account token was not forgotten");
			}
			let _ = internal.send(Internal::SecretHealth(store.secret_health()));
		});
	}
}
