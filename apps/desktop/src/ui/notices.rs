use std::rc::Rc;

use bridge::ToastKind;
use state::{Changes, State};

use super::Link;
use crate::capture;

impl Link {
	pub(super) fn install_toasts(self: &Rc<Self>) {
		self.toasts.install();
		let bounds = self.window.info.bounds.clone();
		let link = Rc::downgrade(self);
		let effect = self.window.runtime.effect_now(move || {
			let (width, height) = bounds.with(|bounds| bounds.size);
			if let Some(link) = link.upgrade() {
				let scale = link.scale_factor() as f32;
				if width > 0 && height > 0 && scale > 0. {
					link.toasts
						.set_window(width as f32 / scale, height as f32 / scale);
					link.scrim.set_window(width as f32 / scale, height as f32 / scale);
				}
			}
		});
		*self.toast_window.borrow_mut() = Some(effect);
	}

	pub(super) fn toasts_apply(&self, state: &State, changes: &Changes) {
		if !changes.rejected {
			return;
		}
		let Some(reason) = self.toasts.new_rejection(state) else {
			return;
		};
		let claimed = self.timeline_view.acted_recently()
			|| self.threads_acted_recently()
			|| self.composer_acted_recently();
		if !claimed {
			self.toasts.push(ToastKind::Error, &reason);
		}
	}

	pub(super) fn drive_toast(&self) {
		self.toasts.push(ToastKind::Error, capture::TOAST);
	}

	pub(super) fn toast_settled(&self) -> bool {
		!self.toasts.showing().is_empty()
	}
}
