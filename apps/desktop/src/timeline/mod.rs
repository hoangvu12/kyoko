mod avatars;
mod rows;
mod scroll;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{Delta, Row, RowKey, Timeline};
use media::{Gallery, ImageName};
use protocol::Snowflake;
use session::Command;
use state::Viewport;
use zgx::compose::prelude::*;
use zgx::compose::{Tasks, VariableHeights, measured_virtual_list};
use zgx::image::ImageData;
use zgx::input::InputEvent;
use zgx::reactive::{Effect, Runtime, Signal};

use self::avatars::Avatars;
use self::rows::RowEnv;
use self::scroll::{Anchor, Ends, Extent, Page, Paging, Reporter};

const OVERSCAN: usize = 4;
const PAD_TOP: f32 = 8.;
const PAD_BOTTOM: f32 = 16.;
const DEFAULT_HEIGHT: f32 = 44.;
const AVATAR_MARGIN: usize = 2;
const REPORT_AFTER: Duration = Duration::from_millis(250);

type Callback<T> = RefCell<Option<Box<dyn Fn(T)>>>;

struct Measured<'a>(&'a VariableHeights);

impl Extent for Measured<'_> {
	fn row_offset(&self, index: usize) -> f32 {
		self.0.row_offset(index)
	}

	fn content_height(&self) -> f32 {
		self.0.content_height()
	}

	fn row_at(&self, offset: f32) -> Option<usize> {
		self.0.row_at(offset)
	}
}

pub struct TimelineView {
	runtime: Runtime,
	offset: Signal<f32>,
	heights: VariableHeights,
	keys: Rc<RefCell<Vec<RowKey>>>,
	rows: RefCell<HashMap<RowKey, Signal<Rc<Row>>>>,
	size: Signal<(f32, f32)>,
	channel: Cell<Option<Snowflake>>,
	env: Rc<RowEnv>,
	ends: Cell<Ends>,
	paging: RefCell<Paging>,
	reporter: RefCell<Reporter>,
	avatars: Rc<Avatars>,
	visible_avatars: RefCell<Vec<ImageName>>,
	tasks: RefCell<Option<Tasks>>,
	watcher: RefCell<Option<Effect>>,
	commands: Callback<Command>,
	visible_changed: Callback<()>,
}

impl TimelineView {
	pub fn new(runtime: &Runtime, gallery: Gallery, cdn: String) -> Rc<Self> {
		let avatars = Rc::new(Avatars::new(runtime.clone(), gallery, cdn));
		Rc::new(Self {
			runtime: runtime.clone(),
			offset: runtime.signal(0.),
			heights: VariableHeights::new(runtime, 0, DEFAULT_HEIGHT).anchor_end(),
			keys: Rc::default(),
			rows: RefCell::default(),
			size: runtime.signal((0., 0.)),
			channel: Cell::new(None),
			env: Rc::new(RowEnv {
				avatars: avatars.clone(),
				channel_name: runtime.signal(String::new()),
			}),
			ends: Cell::new(Ends {
				has_older: false,
				detached: false,
				oldest: None,
				newest: None,
			}),
			paging: RefCell::default(),
			reporter: RefCell::default(),
			avatars,
			visible_avatars: RefCell::default(),
			tasks: RefCell::new(None),
			watcher: RefCell::new(None),
			commands: RefCell::new(None),
			visible_changed: RefCell::new(None),
		})
	}

	pub fn on_command(&self, send: impl Fn(Command) + 'static) {
		*self.commands.borrow_mut() = Some(Box::new(send));
	}

	pub fn on_visible_changed(&self, changed: impl Fn(()) + 'static) {
		*self.visible_changed.borrow_mut() = Some(Box::new(changed));
	}

	pub fn resize(&self, width: f32, height: f32) {
		self.size.set((width, height));
	}

	pub fn set_scale(&self, scale: f64) {
		self.avatars.set_scale(scale);
	}

	pub fn show_image(&self, name: &ImageName, image: Option<Arc<ImageData>>) {
		self.avatars.show(name, image);
	}

	pub fn visible_images(&self) -> Vec<ImageName> {
		self.visible_avatars.borrow().clone()
	}

	pub fn settled(&self) -> bool {
		!self.keys.borrow().is_empty()
			&& self
				.visible_avatars
				.borrow()
				.iter()
				.all(|name| self.avatars.is_shown(name))
	}

	pub fn view(self: &Rc<Self>) -> View {
		let this = self.clone();
		component(move |cx| {
			*this.tasks.borrow_mut() = Some(cx.tasks());
			this.watch();
			let keys = this.keys.clone();
			let builder = Rc::downgrade(&this);
			measured_virtual_list(
				this.offset.clone(),
				this.heights.clone(),
				OVERSCAN,
				move |index| keys.borrow()[index].clone(),
				move |_, key, cx| match builder.upgrade() {
					Some(view) => view.row(&key, cx),
					None => div(),
				},
			)
			.w_full()
			.h_full()
			.pt(PAD_TOP)
			.pb(PAD_BOTTOM)
			.scrollbar(true)
			.selectable(true)
		})
	}

	fn row(&self, key: &RowKey, cx: &mut Context) -> View {
		let Some(signal) = self.rows.borrow().get(key).cloned() else {
			return div();
		};
		let hovered = cx.state(false);
		let tracker = hovered.clone();
		let env = self.env.clone();
		div()
			.w_full()
			.on_event(move |event| {
				if matches!(event.event, InputEvent::PointerEnter) {
					tracker.set(true);
				} else if matches!(event.event, InputEvent::PointerLeave) {
					tracker.set(false);
				}
			})
			.child(switch(
				move || signal.get(),
				move |row: Rc<Row>, _| rows::render(&row, &env, &hovered),
			))
	}

	fn watch(self: &Rc<Self>) {
		if self.watcher.borrow().is_some() {
			return;
		}
		let weak = Rc::downgrade(self);
		let (offset, heights, size) = (self.offset.clone(), self.heights.clone(), self.size.clone());
		let runtime = self.runtime.clone();
		let effect = self.runtime.effect(move || {
			offset.get();
			heights.len();
			size.get();
			if let Some(view) = weak.upgrade() {
				runtime.untracked(|| view.scrolled());
			}
		});
		*self.watcher.borrow_mut() = Some(effect);
	}

	fn viewport(&self) -> (f32, f32) {
		let (width, height) = self.size.with_untracked(|size| *size);
		(width, (height - PAD_TOP - PAD_BOTTOM).max(0.))
	}

	pub fn apply(&self, timeline: &Timeline, delta: &Delta, channel_name: &str) {
		let channel = timeline.open().map(|open| open.channel_id);
		let reset = delta.reset || channel != self.channel.replace(channel);
		let messages = || timeline.messages().filter(|message| !message.pending);
		self.ends.set(Ends {
			has_older: timeline.has_older(),
			detached: timeline.is_detached(),
			oldest: messages().next().map(|message| message.id),
			newest: messages().next_back().map(|message| message.id),
		});
		let (width, viewport) = self.viewport();
		let rows = timeline.rows();
		let keys: Vec<RowKey> = rows.iter().map(|row| row.key.clone()).collect();
		let (anchor, heights) = self.runtime.untracked(|| {
			let old = self.keys.borrow();
			let measured = Measured(&self.heights);
			let offset = self.offset.with_untracked(|offset| *offset);
			let following = scroll::at_bottom(&measured, offset, viewport);
			let anchor = (!reset && !following && viewport > 0.)
				.then(|| Anchor::capture(&old, &measured, offset, viewport))
				.flatten();
			let known: HashMap<&RowKey, f32> = old
				.iter()
				.enumerate()
				.filter_map(|(index, key)| Some((key, self.heights.row_height(index)?)))
				.collect();
			let changed: HashSet<&RowKey> = delta.updated.iter().collect();
			let heights: Vec<f32> = rows
				.iter()
				.map(|row| {
					known
						.get(&row.key)
						.copied()
						.filter(|_| !changed.contains(&row.key))
						.unwrap_or_else(|| rows::estimate(&row.row, width))
				})
				.collect();
			(anchor, heights)
		});
		self.runtime.batch(|| {
			self.env.channel_name.set(channel_name.to_owned());
			{
				let mut store = self.rows.borrow_mut();
				let current: HashSet<&RowKey> = keys.iter().collect();
				store.retain(|key, _| current.contains(key));
				for row in rows {
					let value = Rc::new(row.row.clone());
					match store.get(&row.key) {
						Some(signal) => {
							signal.set(value);
						}
						None => {
							store.insert(row.key.clone(), self.runtime.signal(value));
						}
					}
				}
			}
			*self.keys.borrow_mut() = keys;
			if self.heights.replace(heights).is_err() {
				tracing::warn!("the timeline row heights were not usable");
			}
			if reset {
				self.paging.borrow_mut().reset();
				self.reporter.borrow_mut().reset();
				self.offset.set(f32::MAX);
			} else if let Some(anchor) = anchor {
				let restored = self
					.runtime
					.untracked(|| anchor.restore(&self.keys.borrow(), &Measured(&self.heights)));
				if let Some(offset) = restored {
					self.offset.set(offset);
				}
			}
		});
	}

	fn scrolled(self: &Rc<Self>) {
		let Some(channel_id) = self.channel.get() else {
			return;
		};
		let (_, viewport) = self.viewport();
		if viewport <= 0. {
			return;
		}
		let measured = Measured(&self.heights);
		let offset = self.offset.with_untracked(|offset| *offset);
		let bottom = scroll::at_bottom(&measured, offset, viewport);
		let Some((first, last)) = scroll::visible(&measured, offset, viewport) else {
			return;
		};
		let (page, report, avatars) = {
			let keys = self.keys.borrow();
			if keys.is_empty() {
				return;
			}
			let last = last.min(keys.len() - 1);
			tracing::trace!(
				first = ?keys[first],
				top = measured.row_offset(first) - offset.min(scroll::limit(&measured, viewport)),
				bottom,
				rows = keys.len(),
				"timeline scrolled"
			);
			let page =
				self.paging
					.borrow_mut()
					.wanted(self.ends.get(), first, keys.len(), bottom, Instant::now());
			let ids: Vec<Snowflake> = keys[first..=last]
				.iter()
				.filter_map(|key| match key {
					RowKey::Message(id) => Some(*id),
					_ => None,
				})
				.collect();
			let report = match (ids.first(), ids.last()) {
				(Some(&first), Some(&last)) => Some(Viewport {
					first,
					last,
					at_bottom: bottom,
				}),
				_ => None,
			};
			let around = first.saturating_sub(AVATAR_MARGIN)..(last + AVATAR_MARGIN + 1).min(keys.len());
			(page, report, self.avatars_in(&keys[around]))
		};
		if let Some(page) = page {
			self.send(match page {
				Page::Older => Command::LoadOlder { channel_id },
				Page::Newer => Command::LoadNewer { channel_id },
			});
		}
		if let Some(viewport) = report
			&& self.reporter.borrow_mut().offer(channel_id, viewport)
		{
			self.schedule_report();
		}
		if *self.visible_avatars.borrow() != avatars {
			*self.visible_avatars.borrow_mut() = avatars;
			if let Some(changed) = self.visible_changed.borrow().as_ref() {
				changed(());
			}
		}
	}

	fn avatars_in(&self, keys: &[RowKey]) -> Vec<ImageName> {
		let store = self.rows.borrow();
		let mut names: Vec<ImageName> = keys
			.iter()
			.filter_map(|key| {
				let row = store.get(key)?.with_untracked(Rc::clone);
				match row.as_ref() {
					Row::Message(entry) if entry.is_plain() && !entry.grouped => {
						self.avatars.name_for(&entry.message.author)
					}
					_ => None,
				}
			})
			.collect();
		names.dedup();
		names
	}

	fn schedule_report(self: &Rc<Self>) {
		let Some(tasks) = self.tasks.borrow().clone() else {
			return;
		};
		let weak: Weak<Self> = Rc::downgrade(self);
		tasks.spawn(async move {
			zgx::timer::sleep(REPORT_AFTER).await;
			let Some(view) = weak.upgrade() else {
				return;
			};
			let flushed = view.reporter.borrow_mut().flush();
			if let Some((channel_id, viewport)) = flushed
				&& view.channel.get() == Some(channel_id)
			{
				view.send(Command::SetViewport { channel_id, viewport });
			}
		});
	}

	fn send(&self, command: Command) {
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}
}
