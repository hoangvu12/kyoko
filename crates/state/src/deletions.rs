use protocol::Snowflake;

use crate::changes::Changes;
use crate::messages::SendFailure;
use crate::state::State;

const KEPT_BATCHES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteStatus {
	Queued,
	Deleted,
	Refused,
	Failed(SendFailure),
	Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteBatch {
	pub id: u64,
	pub channel_id: Snowflake,
	pub messages: Vec<(Snowflake, DeleteStatus)>,
}

impl DeleteBatch {
	pub fn status(&self, message_id: Snowflake) -> Option<&DeleteStatus> {
		self.messages
			.iter()
			.find(|(id, _)| *id == message_id)
			.map(|(_, status)| status)
	}
}

impl State {
	pub fn deletions(&self) -> impl DoubleEndedIterator<Item = &DeleteBatch> {
		self.deletions.iter()
	}

	pub(crate) fn queue_deletes(&mut self, batch: &DeleteBatch, changes: &mut Changes) {
		if self.deletions.len() >= KEPT_BATCHES {
			self.deletions.pop_front();
		}
		self.deletions.push_back(batch.clone());
		changes.deletions = true;
	}

	pub(crate) fn delete_progressed(
		&mut self,
		batch: u64,
		message_id: Snowflake,
		status: &DeleteStatus,
		changes: &mut Changes,
	) {
		let entry = self
			.deletions
			.iter_mut()
			.find(|queued| queued.id == batch)
			.and_then(|queued| queued.messages.iter_mut().find(|(id, _)| *id == message_id));
		if let Some((_, current)) = entry {
			*current = status.clone();
			changes.deletions = true;
		}
	}
}
