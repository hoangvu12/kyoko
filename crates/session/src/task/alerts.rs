use std::sync::Arc;
use std::time::Instant;

use protocol::{Message, StatusKind};
use state::Changes;

use super::{Core, Internal};
use crate::alerts::Alert;
use crate::hooks::guarded_blocking;

impl Core {
	pub(super) fn consider_alert(&mut self, message: &Message) {
		if self.status == StatusKind::Dnd {
			return;
		}
		let alert = {
			let state = self.state.read();
			let Some(kind) = state.alert_for(message) else {
				return;
			};
			let in_view = self.viewing
				&& state
					.selected()
					.is_some_and(|selected| selected.channel_id == message.channel_id);
			if in_view {
				return;
			}
			Alert {
				kind,
				guild_id: message
					.guild_id
					.or_else(|| state.channel_guild(message.channel_id)),
				message: Arc::new(message.clone()),
			}
		};
		if self.hooks.is_none() {
			self.raise_alert(alert);
			return;
		}
		let hooks = self.hooks.clone();
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			let message = alert.message.clone();
			let allowed = guarded_blocking(&hooks, "allow_notification", move |hooks| {
				hooks.allow_notification(&message)
			})
			.await
			.unwrap_or(true);
			if allowed {
				let _ = internal.send(Internal::Alert(alert));
			}
		});
	}

	pub(super) fn raise_alert(&mut self, alert: Alert) {
		if !self.pacing.allow(Instant::now()) {
			tracing::debug!("a notification was held back by the burst limit");
			return;
		}
		self.alerts.push(alert);
		self.notifier.push(Changes {
			alerts: true,
			..Changes::default()
		});
	}
}
