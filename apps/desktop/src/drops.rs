#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::rc::Rc;

use zgx::compose::View;
use zgx::input::{EventPhase, InputEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
	Main,
	Thread,
	ForumPost,
	VoiceChat,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DropZones {
	pub thread_left: Option<f32>,
	pub forum: bool,
	pub call: bool,
	pub scale: f32,
}

impl DropZones {
	pub fn target(&self, (x, _): (f32, f32)) -> DropTarget {
		match self.thread_left {
			Some(left) if x >= left => DropTarget::Thread,
			_ if self.call => DropTarget::VoiceChat,
			_ if self.forum => DropTarget::ForumPost,
			_ => DropTarget::Main,
		}
	}

	pub fn pointer(&self) -> Option<(f32, f32)> {
		let (x, y) = platform::pointer::over_own_window()?;
		let scale = if self.scale > 0. { self.scale } else { 1. };
		Some((x / scale, y / scale))
	}

	fn target_at(&self, reported: (f32, f32)) -> DropTarget {
		self.target(self.pointer().unwrap_or(reported))
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileDrag {
	Over(DropTarget),
	Left,
	Dropped(DropTarget, Vec<PathBuf>),
}

pub fn accept_files(
	view: View,
	zones: impl Fn() -> DropZones + 'static,
	heard: impl Fn(FileDrag) + 'static,
) -> View {
	let zones = Rc::new(zones);
	let heard = Rc::new(heard);
	let (hover_zones, hover_heard) = (zones.clone(), heard.clone());
	view.on_event(move |cx| {
		if cx.phase == EventPhase::Capture {
			return;
		}
		match cx.event {
			InputEvent::FileHover { x, y, .. } => {
				hover_heard(FileDrag::Over(hover_zones().target_at((x, y))))
			}
			InputEvent::FileHoverCancelled | InputEvent::FilesDropRejected { .. } => {
				hover_heard(FileDrag::Left)
			}
			_ => {}
		}
	})
	.on_files_drop(move |paths, cx| {
		let reported = match cx.event {
			InputEvent::FilesDrop { x, y, .. } => (x, y),
			_ => (0., 0.),
		};
		heard(FileDrag::Left);
		heard(FileDrag::Dropped(zones().target_at(reported), paths.to_vec()));
	})
}
