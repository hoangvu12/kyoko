use chrono::Utc;
use state::{LocalEvent, RiskyLock, RiskyReason, RiskyTarget};

use super::{Command, Core};

impl Core {
	pub(super) fn held_by_lock(&mut self, command: &Command) -> bool {
		let Some(lock) = self.lock_for(command) else {
			return false;
		};
		tracing::info!(reason = lock.reason.code(), "risky action held for confirmation");
		if !matches!(command, Command::Typing { .. }) {
			self.reject(lock.reason.explanation());
		}
		true
	}

	fn lock_for(&self, command: &Command) -> Option<RiskyLock> {
		let state = self.state.read();
		let now = Utc::now();
		match command {
			Command::SendMessage { channel_id, .. }
			| Command::SendSticker { channel_id, .. }
			| Command::SendGif { channel_id, .. }
			| Command::CreateForumPost { channel_id, .. }
			| Command::CreateThread { channel_id, .. }
			| Command::AddReaction { channel_id, .. }
			| Command::Typing { channel_id }
			| Command::RunApplicationCommand { channel_id, .. }
			| Command::ForwardMessage {
				to_channel_id: channel_id,
				..
			} => state.channel_lock(*channel_id, now),
			Command::OpenDm { user_id } => state.dm_lock(*user_id),
			_ => None,
		}
	}

	pub(super) fn confirm_risky(&mut self, target: RiskyTarget, reason: RiskyReason) {
		self.apply_local(LocalEvent::RiskyConfirmed { target, reason });
	}
}
