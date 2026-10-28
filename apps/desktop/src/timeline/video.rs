use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use audio::Sink;
use bridge::{Inbox, VideoClip, VideoPhase, VideoPlayer, VideoState};
use media::{Gallery, ImageName, Playback};
use protocol::Snowflake;
use zgx::compose::prelude::*;
use zgx::compose::{NodeHandle, Tasks};
use zgx::image::ImageData;
use zgx::input::{EventContext, EventPhase, InputEvent, PointerButton};
use zgx::reactive::{Runtime, Signal};
use zgx::scene::Rect;

use super::attachments::picture;
use super::icons::icon;
use super::pictures::{Picture, Pictures};
use super::rows::tok;

const CONTROLS: f32 = 32.;
const CONTROL_ICON: f32 = 20.;
const CONTROL_BUTTON: f32 = 28.;
const SEEK_HEIGHT: f32 = 6.;
const VOLUME_WIDTH: f32 = 64.;
const PLAY_DISC: f32 = 48.;
const PLAY_ICON: f32 = 24.;
const SCRIM: u32 = 0x000000;
const BAR: u32 = 0x0000_0099;
const SEEK_TRACK: u32 = 0xffff_ff4d;

#[derive(Clone)]
struct Theater {
	clip: VideoClip,
	poster: Option<ImageName>,
}

impl PartialEq for Theater {
	fn eq(&self, other: &Self) -> bool {
		self.clip == other.clip
	}
}

type Hook = RefCell<Option<Box<dyn Fn(bool)>>>;

pub struct Watching {
	runtime: Runtime,
	gallery: Gallery,
	player: VideoPlayer,
	tick: Signal<u64>,
	wakes: Inbox<()>,
	started: Cell<bool>,
	mock_origin: Option<String>,
	frames: RefCell<HashMap<String, Picture>>,
	views: RefCell<HashMap<String, (u32, u32)>>,
	clips: RefCell<HashMap<String, (Snowflake, VideoClip)>>,
	visible: RefCell<HashSet<Snowflake>>,
	hovered: RefCell<Option<String>>,
	theater: Signal<Option<Theater>>,
	window: Signal<(f32, f32)>,
	fullscreen: Hook,
	scale: Cell<f64>,
	shown: Cell<u64>,
	anchors: RefCell<Vec<(String, NodeHandle)>>,
}

impl Watching {
	pub fn new(runtime: &Runtime, gallery: &Gallery, mock_origin: Option<String>) -> Self {
		let sink = if mock_origin.is_some() {
			Sink::Silent
		} else {
			Sink::Device
		};
		let player = VideoPlayer::new(gallery.loader().clone(), gallery.runtime().clone(), sink);
		let wakes = Inbox::new();
		let pushed = wakes.clone();
		player.set_waker(move || pushed.push(()));
		let refreshing = gallery.clone();
		player.set_refresher(move |origin| refreshing.refresh(origin));
		Self {
			runtime: runtime.clone(),
			gallery: gallery.clone(),
			player,
			tick: runtime.signal(0),
			wakes,
			started: Cell::new(false),
			mock_origin,
			frames: RefCell::default(),
			views: RefCell::default(),
			clips: RefCell::default(),
			visible: RefCell::default(),
			hovered: RefCell::default(),
			theater: runtime.signal(None),
			window: runtime.signal((0., 0.)),
			fullscreen: RefCell::new(None),
			scale: Cell::new(1.),
			shown: Cell::new(0),
			anchors: RefCell::default(),
		}
	}

	pub fn run(self: &Rc<Self>, tasks: &Tasks) {
		if self.started.replace(true) {
			return;
		}
		let watching = Rc::downgrade(self);
		tasks.spawn(async move {
			loop {
				let Some(wakes) = watching.upgrade().map(|this| this.wakes.clone()) else {
					return;
				};
				if wakes.next().await.is_none() {
					return;
				}
				match watching.upgrade() {
					Some(this) => this.drain(),
					None => return,
				}
			}
		});
	}

	fn drain(&self) {
		for shown in self.player.drain() {
			let picture = shown.picture;
			match ImageData::new(picture.width, picture.height, picture.rgba) {
				Ok(image) => {
					self.shown.set(self.shown.get() + 1);
					self.frame(&shown.key).set(Some(Arc::new(image)));
				}
				Err(error) => tracing::debug!(error, "a video frame was not usable"),
			}
		}
		self.tick.update(|tick| *tick = tick.wrapping_add(1));
	}

	pub fn player(&self) -> &VideoPlayer {
		&self.player
	}

	pub fn frames_shown(&self) -> u64 {
		self.shown.get()
	}

	pub fn on_fullscreen(&self, hook: impl Fn(bool) + 'static) {
		*self.fullscreen.borrow_mut() = Some(Box::new(hook));
	}

	pub fn set_window(&self, width: f32, height: f32) {
		if self.window.with_untracked(|window| *window != (width, height)) {
			self.window.set((width, height));
		}
		let theater = self.theater.with_untracked(Clone::clone);
		if let Some(theater) = theater {
			self.player.set_view(
				&theater.clip,
				device(width, self.scale()),
				device(height, self.scale()),
			);
		}
	}

	fn scale(&self) -> f64 {
		self.scale.get()
	}

	pub fn set_scale(&self, scale: f64) {
		self.scale.set(scale);
	}

	pub fn clip_of(&self, mut clip: VideoClip) -> VideoClip {
		clip.url = bridge::local_video(&clip.url, self.mock_origin.as_deref());
		clip
	}

	fn frame(&self, key: &str) -> Picture {
		self.frames
			.borrow_mut()
			.entry(key.to_owned())
			.or_insert_with(|| self.runtime.signal(None))
			.clone()
	}

	fn register(&self, clip: &VideoClip, size: (f32, f32)) {
		let view = (device(size.0, self.scale()), device(size.1, self.scale()));
		self.views.borrow_mut().insert(clip.key.clone(), view);
		let theater_open = self.theater.with_untracked(|theater| {
			theater
				.as_ref()
				.is_some_and(|theater| theater.clip.key == clip.key)
		});
		if !theater_open {
			self.player.set_view(clip, view.0, view.1);
		}
		self.clips
			.borrow_mut()
			.insert(clip.key.clone(), (clip.message_id, clip.clone()));
		if clip.gifv && self.visible.borrow().contains(&clip.message_id) {
			self.autoplay(clip, true);
		}
	}

	fn gif_motion(&self, clip: &VideoClip) -> bool {
		match self.gallery.gif_playback() {
			Playback::Always => true,
			Playback::OnHover => self.hovered.borrow().as_deref() == Some(clip.key.as_str()),
			Playback::Never => false,
		}
	}

	fn autoplay(&self, clip: &VideoClip, visible: bool) {
		let state = self.player.state(clip);
		let wanted = visible && self.gif_motion(clip);
		if wanted && !state.is_playing() && state.phase != VideoPhase::Failed {
			self.player.play(clip);
		} else if !wanted && state.is_playing() {
			self.player.pause(clip);
		}
	}

	pub fn set_visible(&self, messages: impl IntoIterator<Item = Snowflake>) {
		let messages: HashSet<Snowflake> = messages.into_iter().collect();
		if *self.visible.borrow() == messages {
			return;
		}
		*self.visible.borrow_mut() = messages;
		self.refresh_visible();
	}

	fn refresh_visible(&self) {
		let theater = self
			.theater
			.with_untracked(|theater| theater.as_ref().map(|theater| theater.clip.key.clone()));
		let clips: Vec<(bool, VideoClip)> = self
			.clips
			.borrow()
			.iter()
			.map(|(key, (message, clip))| {
				let shown = self.visible.borrow().contains(message) || theater.as_ref() == Some(key);
				(shown, clip.clone())
			})
			.collect();
		let keys: Vec<String> = clips
			.iter()
			.filter(|(shown, _)| *shown)
			.map(|(_, clip)| clip.key.clone())
			.collect();
		self.player.set_visible(keys.iter().map(String::as_str));
		for (shown, clip) in clips.iter().filter(|(_, clip)| clip.gifv) {
			self.autoplay(clip, *shown);
		}
	}

	fn hover_gif(&self, clip: &VideoClip, over: bool) {
		*self.hovered.borrow_mut() = over.then(|| clip.key.clone());
		let visible = self.visible.borrow().contains(&clip.message_id);
		self.autoplay(clip, visible);
	}

	pub fn left(&self, channel_id: Snowflake) {
		self.leave_fullscreen();
		self.player.left(channel_id);
		let gone: Vec<String> = self
			.clips
			.borrow()
			.values()
			.filter(|(_, clip)| clip.channel_id == channel_id)
			.map(|(_, clip)| clip.key.clone())
			.collect();
		let mut frames = self.frames.borrow_mut();
		for key in &gone {
			self.clips.borrow_mut().remove(key);
			self.views.borrow_mut().remove(key);
			if let Some(frame) = frames.remove(key) {
				frame.set(None);
			}
		}
	}

	pub fn stop(&self) {
		self.leave_fullscreen();
		self.player.stop();
		self.clips.borrow_mut().clear();
		for frame in self.frames.borrow_mut().drain().map(|(_, frame)| frame) {
			frame.set(None);
		}
	}

	pub fn anchor(&self, id: &str) -> Option<Rect> {
		self.anchors
			.borrow()
			.iter()
			.rev()
			.find(|(known, _)| known == id)
			.and_then(|(_, handle)| bounds_of(handle))
	}

	fn remember(&self, id: String, handle: NodeHandle) {
		let mut anchors = self.anchors.borrow_mut();
		anchors.retain(|(_, known)| bounds_of(known).is_some());
		anchors.push((id, handle));
	}

	fn watch(&self, cx: &mut Context, clip: &VideoClip) -> Signal<VideoState> {
		let state = cx.state(self.player.state(clip));
		let (tick, player, clip, shown) = (
			self.tick.clone(),
			self.player.clone(),
			clip.clone(),
			state.clone(),
		);
		let effect = self.runtime.effect(move || {
			tick.get();
			let now = player.state(&clip);
			if shown.with_untracked(|before| *before != now) {
				shown.set(now);
			}
		});
		cx.retain(effect);
		state
	}

	fn toggle(&self, clip: &VideoClip) {
		self.player.toggle(clip);
		self.drain();
	}

	fn seek(&self, clip: &VideoClip, fraction: f32) {
		self.player.seek(clip, fraction);
		self.drain();
	}

	fn set_volume(&self, volume: f32) {
		self.player.set_volume(volume);
		if volume > 0. && self.player.sound().1 {
			self.player.toggle_mute();
		}
		self.drain();
	}

	fn toggle_mute(&self) {
		self.player.toggle_mute();
		self.drain();
	}

	pub fn is_fullscreen(&self) -> bool {
		self.theater.with_untracked(Option::is_some)
	}

	fn enter_fullscreen(&self, clip: &VideoClip, poster: Option<ImageName>) {
		let (width, height) = self.window.with_untracked(|window| *window);
		self.player
			.set_view(clip, device(width, self.scale()), device(height, self.scale()));
		self.theater.set(Some(Theater {
			clip: clip.clone(),
			poster,
		}));
		if let Some(hook) = self.fullscreen.borrow().as_ref() {
			hook(true);
		}
	}

	pub fn leave_fullscreen(&self) -> bool {
		let Some(theater) = self.theater.with_untracked(Clone::clone) else {
			return false;
		};
		self.theater.set(None);
		if let Some((width, height)) = self.views.borrow().get(&theater.clip.key).copied() {
			self.player.set_view(&theater.clip, width, height);
		}
		if let Some(hook) = self.fullscreen.borrow().as_ref() {
			hook(false);
		}
		self.refresh_visible();
		true
	}

	fn toggle_fullscreen(&self, clip: &VideoClip, poster: Option<ImageName>) {
		if self.is_fullscreen() {
			self.leave_fullscreen();
		} else {
			self.enter_fullscreen(clip, poster);
		}
	}

	pub fn view(self: &Rc<Self>, pictures: &Rc<Pictures>, code_font: &str) -> View {
		let (this, pictures, code_font) = (self.clone(), pictures.clone(), code_font.to_owned());
		let (theater, sized, window) = (self.theater.clone(), self.theater.clone(), self.window.clone());
		div()
			.reactive_style(move || {
				if sized.with(Option::is_some) {
					let (width, height) = window.get();
					Styles::new().absolute().left(0.).top(0.).w(width).h(height)
				} else {
					Styles::new().absolute().left(0.).top(0.).w(0.).h(0.)
				}
			})
			.child(switch(
				move || theater.get(),
				move |theater: Option<Theater>, _| match theater {
					Some(theater) => this.theater_view(&theater, &pictures, &code_font),
					None => div(),
				},
			))
	}

	fn theater_view(self: &Rc<Self>, theater: &Theater, pictures: &Rc<Pictures>, code_font: &str) -> View {
		let window = self.window.clone();
		let screen = div()
			.id("video-fullscreen")
			.reactive_style(move || {
				let (width, height) = window.get();
				Styles::new().w(width).h(height)
			})
			.bg(rgb(SCRIM));
		screen.child(self.surface(
			&theater.clip,
			theater.poster.clone(),
			None,
			pictures,
			code_font,
			true,
		))
	}

	pub fn player_view(
		self: &Rc<Self>,
		clip: VideoClip,
		poster: Option<ImageName>,
		placeholder: Option<String>,
		size: (f32, f32),
		pictures: &Rc<Pictures>,
		code_font: &str,
	) -> View {
		self.register(&clip, size);
		self.surface(&clip, poster, placeholder, pictures, code_font, false)
	}

	fn surface(
		self: &Rc<Self>,
		clip: &VideoClip,
		poster: Option<ImageName>,
		placeholder: Option<String>,
		pictures: &Rc<Pictures>,
		code_font: &str,
		full: bool,
	) -> View {
		let (this, clip, pictures, code_font) =
			(self.clone(), clip.clone(), pictures.clone(), code_font.to_owned());
		component(move |cx| {
			let state = this.watch(cx, &clip);
			let hovered = cx.state(false);
			let frame = this.frame(&clip.key);
			let (toggler, toggled) = (this.clone(), clip.clone());
			let (entering, leaving) = (hovered.clone(), hovered.clone());
			let id = if full {
				format!("video-fullscreen-{}", clip.key)
			} else {
				format!("video-{}", clip.key)
			};
			let (anchors, anchor_id) = (this.clone(), id.clone());
			let shows_frame = frame.clone();
			div()
				.id(id)
				.on_mount(move |handle| anchors.remember(anchor_id.clone(), handle))
				.relative()
				.w_full()
				.h_full()
				.overflow_hidden()
				.cursor(Cursor::Pointer)
				.child(picture(
					&pictures,
					poster.clone(),
					placeholder.as_deref(),
					ObjectFit::Cover,
				))
				.child(
					div()
						.absolute()
						.left(0.)
						.top(0.)
						.w_full()
						.h_full()
						.reactive_style(move || match shows_frame.get() {
							Some(image) => Styles::new().bg(rgb(SCRIM)).bg_image(image, ObjectFit::Contain),
							None => Styles::new(),
						}),
				)
				.child(center(&this, &clip, &state))
				.child(controls(
					&this,
					&clip,
					poster.clone(),
					&state,
					&hovered,
					&code_font,
					full,
				))
				.on_event(move |event| match event.event {
					InputEvent::PointerEnter => {
						entering.set(true);
					}
					InputEvent::PointerLeave => {
						leaving.set(false);
					}
					_ => {}
				})
				.on_click(move || toggler.toggle(&toggled))
		})
	}

	pub fn gif_view(self: &Rc<Self>, clip: VideoClip, size: (f32, f32)) -> View {
		self.register(&clip, size);
		let frame = self.frame(&clip.key);
		let (entered, left, over, out) = (self.clone(), self.clone(), clip.clone(), clip.clone());
		div()
			.id(format!("video-{}", clip.key))
			.absolute()
			.left(0.)
			.top(0.)
			.w_full()
			.h_full()
			.reactive_style(move || match frame.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			})
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => entered.hover_gif(&over, true),
				InputEvent::PointerLeave => left.hover_gif(&out, false),
				_ => {}
			})
	}
}

fn center(watching: &Rc<Watching>, clip: &VideoClip, state: &Signal<VideoState>) -> View {
	let (shown, toggler, toggled) = (state.clone(), watching.clone(), clip.clone());
	let id = format!("video-play-{}", clip.key);
	div()
		.absolute()
		.left(0.)
		.top(0.)
		.w_full()
		.h_full()
		.items_center()
		.justify_center()
		.child(switch(
			move || overlay_of(&shown.get()),
			move |overlay: Overlay, _| {
				let (toggler, toggled) = (toggler.clone(), toggled.clone());
				match overlay {
					Overlay::None => div(),
					Overlay::Play => div()
						.id(id.clone())
						.size(PLAY_DISC, PLAY_DISC)
						.rounded(PLAY_DISC / 2.)
						.bg(tok("overlay"))
						.items_center()
						.justify_center()
						.on_event(swallow_press)
						.on_click(move || toggler.toggle(&toggled))
						.child(icon("play_fill", PLAY_ICON, "overlay_text")),
					Overlay::Loading => icon("spinner", PLAY_ICON, "overlay_text"),
					Overlay::Failed => column()
						.items_center()
						.gap(tok("space_4"))
						.p(tok("space_8"))
						.rounded(tok("radius_sm"))
						.bg(tok("overlay"))
						.child(icon("warning", PLAY_ICON, "overlay_text"))
						.child(
							text("Can't play this video")
								.text_size(tok("font_size_xs"))
								.line_height(tok("line_height_xs"))
								.text_color(tok("overlay_text")),
						),
				}
			},
		))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
	None,
	Play,
	Loading,
	Failed,
}

fn overlay_of(state: &VideoState) -> Overlay {
	match state.phase {
		VideoPhase::Idle => Overlay::Play,
		VideoPhase::Loading if !state.shown => Overlay::Loading,
		VideoPhase::Failed => Overlay::Failed,
		_ => Overlay::None,
	}
}

fn controls(
	watching: &Rc<Watching>,
	clip: &VideoClip,
	poster: Option<ImageName>,
	state: &Signal<VideoState>,
	hovered: &Signal<bool>,
	code_font: &str,
	full: bool,
) -> View {
	let (shown, over, kept) = (state.clone(), hovered.clone(), state.clone());
	let (watching, clip, code_font) = (watching.clone(), clip.clone(), code_font.to_owned());
	div().absolute().left(0.).bottom(0.).w_full().child(switch(
		move || {
			let state = shown.get();
			state.started() && state.phase != VideoPhase::Failed && (over.get() || !state.is_playing())
		},
		move |visible: bool, _| {
			if visible {
				bar(&watching, &clip, poster.clone(), &kept, &code_font, full)
			} else {
				div()
			}
		},
	))
}

fn bar(
	watching: &Rc<Watching>,
	clip: &VideoClip,
	poster: Option<ImageName>,
	state: &Signal<VideoState>,
	code_font: &str,
	full: bool,
) -> View {
	let (toggler, toggled) = (watching.clone(), clip.clone());
	let (screen, screened) = (watching.clone(), clip.clone());
	let state = state.clone();
	let key = clip.key.clone();
	row()
		.id(format!("video-controls-{key}"))
		.w_full()
		.h(CONTROLS)
		.items_center()
		.gap(tok("space_8"))
		.px(tok("space_8"))
		.bg(rgba(BAR))
		.on_event(swallow_press)
		.child(
			button(format!("video-toggle-{key}"), move || toggler.toggle(&toggled)).child(switch(
				{
					let state = state.clone();
					move || state.get().is_playing()
				},
				|playing: bool, _| {
					icon(
						if playing { "pause_fill" } else { "play_fill" },
						CONTROL_ICON,
						"overlay_text",
					)
				},
			)),
		)
		.child(
			text_signal({
				let state = state.clone();
				move || clock(&state.get())
			})
			.font_family(code_font)
			.text_size(tok("font_size_xs"))
			.line_height(tok("line_height_xs"))
			.text_color(tok("overlay_text"))
			.shrink_0(),
		)
		.child(seek_bar(watching, clip, &state))
		.when(!clip.gifv, |bar| bar.child(volume(watching, clip, &state)))
		.child(
			button(format!("video-fullscreen-button-{key}"), move || {
				screen.toggle_fullscreen(&screened, poster.clone())
			})
			.child(icon(
				if full { "fullscreen_exit" } else { "fullscreen" },
				CONTROL_ICON,
				"overlay_text",
			)),
		)
}

fn button(id: String, action: impl Fn() + 'static) -> View {
	div()
		.id(id)
		.size(CONTROL_BUTTON, CONTROL_BUTTON)
		.shrink_0()
		.items_center()
		.justify_center()
		.cursor(Cursor::Pointer)
		.on_event(swallow_press)
		.on_click(action)
}

fn seek_bar(watching: &Rc<Watching>, clip: &VideoClip, state: &Signal<VideoState>) -> View {
	let played = state.clone();
	let (seeker, sought) = (watching.clone(), clip.clone());
	slider(
		format!("video-seek-{}", clip.key),
		div()
			.grow()
			.min_w(0.)
			.h(SEEK_HEIGHT)
			.rounded(SEEK_HEIGHT / 2.)
			.bg(rgba(SEEK_TRACK))
			.overflow_hidden()
			.child(
				div()
					.h_full()
					.rounded(SEEK_HEIGHT / 2.)
					.bg(tok("accent_2"))
					.reactive_style(move || Styles::new().w_percent(played.get().progress() * 100.)),
			),
		watching,
		move |fraction| seeker.seek(&sought, fraction),
	)
}

fn volume(watching: &Rc<Watching>, clip: &VideoClip, state: &Signal<VideoState>) -> View {
	let hovered = watching.runtime.signal(false);
	let (entering, leaving, open) = (hovered.clone(), hovered.clone(), hovered.clone());
	let muter = watching.clone();
	let (muted, level) = (state.clone(), state.clone());
	let setter = watching.clone();
	let key = clip.key.clone();
	row()
		.items_center()
		.shrink_0()
		.on_event(move |event| match event.event {
			InputEvent::PointerEnter => {
				entering.set(true);
			}
			InputEvent::PointerLeave => {
				leaving.set(false);
			}
			_ => {}
		})
		.child(switch(
			move || open.get(),
			move |open: bool, _| {
				if !open {
					return div();
				}
				let level = level.clone();
				let setter = setter.clone();
				slider(
					format!("video-volume-{key}"),
					div()
						.w(VOLUME_WIDTH)
						.h(SEEK_HEIGHT)
						.rounded(SEEK_HEIGHT / 2.)
						.bg(rgba(SEEK_TRACK))
						.overflow_hidden()
						.child(
							div()
								.h_full()
								.rounded(SEEK_HEIGHT / 2.)
								.bg(tok("overlay_text"))
								.reactive_style(move || {
									let state = level.get();
									let shown = if state.muted { 0. } else { state.volume };
									Styles::new().w_percent(shown * 100.)
								}),
						),
					&setter,
					{
						let setter = setter.clone();
						move |fraction| setter.set_volume(fraction)
					},
				)
			},
		))
		.child(
			button(format!("video-mute-{}", clip.key), move || muter.toggle_mute()).child(switch(
				move || muted.get().muted || muted.get().volume == 0.,
				|silent: bool, _| {
					icon(
						if silent { "volume_mute_fill" } else { "volume_fill" },
						CONTROL_ICON,
						"overlay_text",
					)
				},
			)),
		)
}

fn slider(id: String, view: View, watching: &Rc<Watching>, set: impl Fn(f32) + 'static) -> View {
	let node: Rc<RefCell<Option<NodeHandle>>> = Rc::default();
	let mounted = node.clone();
	let dragging = Rc::new(Cell::new(false));
	let anchors = watching.clone();
	view.id(id.clone())
		.cursor(Cursor::Pointer)
		.on_mount(move |handle| {
			anchors.remember(id.clone(), handle.clone());
			mounted.replace(Some(handle));
		})
		.on_event(move |event| {
			if event.phase == EventPhase::Capture {
				return;
			}
			let at = match event.event {
				InputEvent::PointerDown {
					x,
					button: PointerButton::Primary,
					..
				} => {
					dragging.set(true);
					event.capture_pointer();
					Some(x)
				}
				InputEvent::PointerMove { x, .. } if dragging.get() => Some(x),
				InputEvent::PointerUp { .. } | InputEvent::PointerCancel if dragging.replace(false) => {
					event.release_pointer();
					None
				}
				InputEvent::Activate => {
					event.stop_propagation();
					None
				}
				_ => None,
			};
			let Some(x) = at else {
				return;
			};
			if let Some(fraction) = node.borrow().as_ref().and_then(|handle| fraction_at(handle, x)) {
				set(fraction);
			}
			event.prevent_default();
			event.stop_propagation();
		})
}

fn swallow_press(event: &mut EventContext) {
	if event.phase == EventPhase::Capture {
		return;
	}
	if matches!(event.event, InputEvent::Activate) {
		event.stop_propagation();
	}
}

fn fraction_at(handle: &NodeHandle, x: f32) -> Option<f32> {
	let bounds = bounds_of(handle)?;
	(bounds.width > 0.).then(|| ((x - bounds.x) / bounds.width).clamp(0., 1.))
}

fn bounds_of(handle: &NodeHandle) -> Option<Rect> {
	let scene = handle.scene()?;
	let scene = scene.borrow();
	scene.contains(handle.node()).then(|| scene.bounds(handle.node()))
}

fn clock(state: &VideoState) -> String {
	let total = state
		.duration
		.map_or_else(|| "-:--".to_owned(), bridge::audio_clock);
	format!("{} / {total}", bridge::audio_clock(state.position))
}

fn device(points: f32, scale: f64) -> u32 {
	(f64::from(points) * scale).ceil().clamp(2., f64::from(u32::MAX)) as u32
}
