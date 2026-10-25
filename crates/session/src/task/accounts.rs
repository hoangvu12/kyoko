use std::sync::Arc;

use protocol::{ChannelType, Snowflake, Token};

use super::{Core, Internal};

impl Core {
	pub(super) fn remember_account(&mut self) {
		if self.account_saved {
			return;
		}
		let Some(user) = self.current_user() else {
			return;
		};
		self.account_saved = true;
		let store = self.store.clone();
		let token = self.token.clone();
		let signed_in = self.signed_in.clone();
		let internal = self.internal.clone();
		self.runtime.spawn_blocking(move || {
			let signed_in = signed_in.lock();
			if !*signed_in {
				return;
			}
			let saved = store
				.accounts
				.save(user.id, &user.username, user.avatar.as_deref(), token.as_str())
				.and_then(|_| store.accounts.set_current(user.id));
			if let Err(error) = saved {
				tracing::warn!(%error, "account was not saved");
			}
			let _ = internal.send(Internal::SecretHealth(store.secret_health()));
		});
	}

	pub(super) fn adopt_token(&mut self, token: Token) {
		self.rest.set_token(Token::new(token.as_str()));
		self.token = Arc::new(token);
		if !self.remember_account {
			return;
		}
		self.account_saved = false;
		let store = self.store.clone();
		let token = self.token.clone();
		let signed_in = self.signed_in.clone();
		let internal = self.internal.clone();
		self.runtime.spawn_blocking(move || {
			let signed_in = signed_in.lock();
			if !*signed_in {
				return;
			}
			if let Err(error) = store.tokens.save(token.as_str()) {
				tracing::warn!(%error, "refreshed token was not saved");
			}
			let _ = internal.send(Internal::SecretHealth(store.secret_health()));
		});
	}

	pub(super) fn select_new_guild(&mut self, guild_id: Snowflake) {
		if !std::mem::take(&mut self.select_created_guild) {
			return;
		}
		let first_text = self.state.read().guild(guild_id).and_then(|guild| {
			guild
				.channels
				.values()
				.filter(|channel| channel.kind == ChannelType::GuildText)
				.min_by_key(|channel| channel.position)
				.map(|channel| channel.id)
		});
		if let Some(channel_id) = first_text {
			self.select_channel(Some(guild_id), channel_id);
		}
	}

	pub(super) fn switch_account(&self, user_id: Snowflake) {
		let store = self.store.clone();
		let internal = self.internal.clone();
		self.runtime.spawn_blocking(move || {
			let message = match store.accounts.token_for(user_id) {
				Ok(Some(_)) => Internal::AccountSwitch(user_id),
				Ok(None) => Internal::Rejected("That account has no saved sign-in".to_owned()),
				Err(error) => {
					tracing::warn!(%error, "account sign-in was not read");
					Internal::Rejected("That account could not be read".to_owned())
				}
			};
			let _ = internal.send(message);
		});
	}
}
