use super::{Command, Core};

impl Core {
	pub(super) fn refuses_hidden(&self, command: &Command) -> bool {
		let Some(channel_id) = command.reads_channel() else {
			return false;
		};
		if !self.state.read().is_hidden(channel_id) {
			return false;
		}
		tracing::debug!(%channel_id, "dropped a command for a channel the user cannot view");
		true
	}
}
