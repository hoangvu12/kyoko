use std::cell::Cell;
use std::time::{Duration, Instant};

use zgx::scene::{Color, NodeId, NodeKind, Scene};
use zgx::widgets::{EditorHandle, Ui, WeakUi};

const CLEAR: Color = Color(0, 0, 0, 0);

pub struct Blink {
	half: Duration,
	shown: Cell<Option<Shown>>,
}

#[derive(Clone, Copy, PartialEq)]
struct Place {
	editor: NodeId,
	revision: u64,
	anchor: usize,
	focus: usize,
	lit: bool,
}

#[derive(Clone, Copy)]
struct Shown {
	place: Place,
	caret: NodeId,
	since: Instant,
	hidden: Option<Color>,
}

impl Blink {
	pub fn new(half: Duration) -> Self {
		Self {
			half: half.max(Duration::from_millis(1)),
			shown: Cell::new(None),
		}
	}

	pub fn tick(&self, ui: &Ui, now: Instant) -> Instant {
		let focused = ui.focused_editor();
		let mut scene = ui.scene.borrow_mut();
		let current = focused.map(|editor| (place(&editor, &scene), editor.caret));
		let kept = match (self.shown.take(), current) {
			(Some(shown), Some((place, _))) if shown.place == place => Some(shown),
			(previous, current) => {
				if let Some(previous) = previous {
					reveal(&mut scene, previous);
				}
				current.map(|(place, caret)| Shown {
					place,
					caret,
					since: now,
					hidden: None,
				})
			}
		};
		let Some(shown) = kept else {
			return now + self.half;
		};
		let phase = now.saturating_duration_since(shown.since).as_nanos() / self.half.as_nanos();
		let shown = if phase % 2 == 1 {
			hide(&mut scene, shown)
		} else {
			reveal(&mut scene, shown)
		};
		self.shown.set(Some(shown));
		shown.since + self.half * u32::try_from(phase + 1).unwrap_or(u32::MAX)
	}
}

pub async fn run(ui: WeakUi, blink: Blink) {
	loop {
		let Some(alive) = ui.upgrade() else {
			return;
		};
		let next = blink.tick(&alive, Instant::now());
		drop(alive);
		zgx::timer::sleep_until(next).await;
	}
}

fn place(editor: &EditorHandle, scene: &Scene) -> Place {
	let model = editor.editor.borrow();
	let selection = model.selection();
	Place {
		editor: editor.node,
		revision: model.revision(),
		anchor: selection.anchor,
		focus: selection.focus,
		lit: lit(scene, editor.caret),
	}
}

fn lit(scene: &Scene, caret: NodeId) -> bool {
	scene.contains(caret) && scene.effects(caret).opacity > 0.
}

fn hide(scene: &mut Scene, shown: Shown) -> Shown {
	if shown.hidden.is_some() || !lit(scene, shown.caret) {
		return shown;
	}
	match *scene.kind(shown.caret) {
		NodeKind::Rect(color) if color != CLEAR => {
			scene.set_kind(shown.caret, NodeKind::Rect(CLEAR));
			Shown {
				hidden: Some(color),
				..shown
			}
		}
		_ => shown,
	}
}

fn reveal(scene: &mut Scene, shown: Shown) -> Shown {
	if let Some(color) = shown.hidden
		&& scene.contains(shown.caret)
		&& matches!(scene.kind(shown.caret), NodeKind::Rect(now) if *now == CLEAR)
	{
		scene.set_kind(shown.caret, NodeKind::Rect(color));
	}
	Shown {
		hidden: None,
		..shown
	}
}
