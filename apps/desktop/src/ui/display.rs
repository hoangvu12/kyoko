use std::rc::Rc;
use std::sync::Arc;

use bridge::{Display, Inbox};
use plugins::PluginHost;
use session::Formatting;
use state::Changes;

use super::Link;

pub(super) fn start(host: &PluginHost) -> (Display, Inbox<Formatting>) {
	let wakes = Inbox::new();
	let woken = wakes.clone();
	let display = Display::new(Arc::new(host.clone()), move |formatting| woken.push(formatting));
	host.on_change(display.refresher());
	(display, wakes)
}

impl Link {
	pub(super) async fn follow_display(self: Rc<Self>) {
		while let Some(batch) = self.display_wakes.next().await {
			let times = batch.contains(&Formatting::RelativeTimes);
			let names = batch.contains(&Formatting::Names);
			let people = names || batch.contains(&Formatting::Decorations);
			self.display_changed(people, times);
		}
	}

	fn display_changed(&self, names: bool, times: bool) {
		let Some(shared) = self.state() else {
			return;
		};
		let state = shared.read();
		let formatting = if times {
			Formatting::RelativeTimes
		} else if names {
			Formatting::Names
		} else {
			Formatting::Messages
		};
		tracing::debug!(
			names,
			times,
			calls = self.display.calls(),
			"display formatting changed"
		);
		let redrawn = self.timeline.borrow_mut().redraw(&state, formatting);
		self.window.runtime.batch(|| {
			if redrawn {
				self.publish_timeline(&state);
			}
			self.redraw_thread(&state, formatting);
			if names {
				self.members.redraw();
				self.publish_profiles(&state, &Changes::default());
			}
		});
	}
}
