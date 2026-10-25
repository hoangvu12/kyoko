use protocol::{Permissions, Snowflake};
use rest::{RestClient, RestError};
use state::{DeleteBatch, DeleteStatus, LocalEvent};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use super::failures::send_failure;
use super::{Core, Internal};

pub(crate) struct DeleteJob {
	batch: u64,
	channel_id: Snowflake,
	ids: Vec<Snowflake>,
}

pub(super) fn spawn_deleter(
	rest: RestClient,
	internal: mpsc::UnboundedSender<Internal>,
	runtime: &Handle,
) -> mpsc::UnboundedSender<DeleteJob> {
	let (jobs, queued) = mpsc::unbounded_channel();
	runtime.spawn(delete_in_order(rest, queued, internal));
	jobs
}

async fn delete_in_order(
	rest: RestClient,
	mut jobs: mpsc::UnboundedReceiver<DeleteJob>,
	internal: mpsc::UnboundedSender<Internal>,
) {
	while let Some(job) = jobs.recv().await {
		let mut stopped = false;
		for message_id in job.ids {
			let status = if stopped {
				DeleteStatus::Cancelled
			} else {
				match rest.delete_message(job.channel_id, message_id).await {
					Ok(()) | Err(RestError::NotFound) => DeleteStatus::Deleted,
					Err(error) => {
						tracing::warn!(%error, "message was not deleted");
						stopped = true;
						DeleteStatus::Failed(send_failure(&error))
					}
				}
			};
			let progress = Internal::DeleteProgressed {
				batch: job.batch,
				message_id,
				status,
			};
			if internal.send(progress).is_err() {
				return;
			}
		}
	}
}

impl Core {
	pub(super) fn delete_messages(&mut self, channel_id: Snowflake, ids: Vec<Snowflake>) {
		let mut unique: Vec<Snowflake> = Vec::with_capacity(ids.len());
		for id in ids {
			if !unique.contains(&id) {
				unique.push(id);
			}
		}
		let permitted: Vec<bool> = {
			let state = self.state.read();
			let me = state.current_user_id();
			let moderator = state.channel_guild(channel_id).is_some()
				&& state
					.my_permissions(channel_id)
					.contains(Permissions::MANAGE_MESSAGES);
			let cache = state.messages(channel_id);
			unique
				.iter()
				.map(|id| {
					moderator
						|| cache
							.and_then(|cache| cache.by_id.get(id))
							.is_some_and(|message| Some(message.author.id) == me)
				})
				.collect()
		};
		self.delete_batches += 1;
		let batch = DeleteBatch {
			id: self.delete_batches,
			channel_id,
			messages: unique
				.iter()
				.zip(&permitted)
				.map(|(&id, &allowed)| {
					let status = if allowed {
						DeleteStatus::Queued
					} else {
						DeleteStatus::Refused
					};
					(id, status)
				})
				.collect(),
		};
		let allowed: Vec<Snowflake> = unique
			.into_iter()
			.zip(permitted)
			.filter_map(|(id, allowed)| allowed.then_some(id))
			.collect();
		let refused = allowed.len() < batch.messages.len();
		let job = DeleteJob {
			batch: batch.id,
			channel_id,
			ids: allowed,
		};
		self.apply_local(LocalEvent::DeletesQueued(batch));
		if refused {
			self.reject("You can only delete your own messages here");
		}
		if !job.ids.is_empty() && self.deletes.send(job).is_err() {
			tracing::warn!("the delete queue has stopped");
		}
	}
}
