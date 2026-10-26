use std::sync::Arc;

use protocol::InteractionModal;

use crate::changes::Changes;
use crate::state::State;

pub const MODAL_FAILED: &str = "Something went wrong. Try again.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalStatus {
	Open,
	Submitting,
	Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppModal {
	pub modal: Arc<InteractionModal>,
	pub status: ModalStatus,
	pub missing: Vec<String>,
	pub serial: u64,
}

impl State {
	pub fn app_modal(&self) -> Option<&AppModal> {
		self.app_modal.as_ref()
	}

	pub(crate) fn modal_opened(&mut self, modal: &InteractionModal, changes: &mut Changes) {
		let serial = self.app_modal.as_ref().map_or(0, |open| open.serial) + 1;
		self.app_modal = Some(AppModal {
			modal: Arc::new(modal.clone()),
			status: ModalStatus::Open,
			missing: Vec::new(),
			serial,
		});
		changes.commands = true;
	}

	pub(crate) fn modal_changed(&mut self, status: &ModalStatus, missing: &[String], changes: &mut Changes) {
		if let Some(open) = self.app_modal.as_mut() {
			open.status = status.clone();
			open.missing = missing.to_vec();
			changes.commands = true;
		}
	}

	pub(crate) fn modal_closed(&mut self, changes: &mut Changes) {
		if self.app_modal.take().is_some() {
			changes.commands = true;
		}
	}
}
