use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use audio::Sink;
use bridge::{AudioPhase, AudioPlayer, AudioState, AudioTrack, Inbox, WAVEFORM_BARS};
use media::Gallery;
use protocol::{Attachment, Message, Snowflake};
use zgx::compose::prelude::*;
use zgx::compose::{NodeHandle, Tasks};
use zgx::input::{EventPhase, InputEvent, PointerButton};
use zgx::reactive::{Runtime, Signal};
use zgx::scene::Rect;

use super::icons::icon;
use super::rows::{RowEnv, tok};

const TICK: Duration = Duration::from_millis(100);
const PILL_RADIUS: f32 = 24.;
const PLAY_BUTTON: f32 = 32.;
const PLAY_ICON: f32 = 16.;
const BAR_MAX_HEIGHT: f32 = 20.;
const BAR_WIDTH: f32 = 2.;
const SPEED_HEIGHT: f32 = 20.;
const PLAYER_ICON: f32 = 20.;
const FILE_ICON: f32 = 32.;
const LOADING_OPACITY: f32 = 0.5;

pub struct Listening {
	runtime: Runtime,
	player: AudioPlayer,
	tick: Signal<u64>,
	wakes: Inbox<()>,
	started: Cell<bool>,
	mock_origin: Option<String>,
	frozen: Cell<bool>,
	anchors: RefCell<Vec<(String, NodeHandle)>>,
}

impl Listening {
	pub fn new(runtime: &Runtime, gallery: &Gallery, mock_origin: Option<String>) -> Self {
		let sink = if mock_origin.is_some() {
			Sink::Silent
		} else {
			Sink::Device
		};
		let player = AudioPlayer::new(gallery.loader().clone(), gallery.runtime().clone(), sink);
		let wakes = Inbox::new();
		let pushed = wakes.clone();
		player.set_waker(move || pushed.push(()));
		let refreshing = gallery.clone();
		player.set_refresher(move |origin| refreshing.refresh(origin));
		Self {
			runtime: runtime.clone(),
			player,
			tick: runtime.signal(0),
			wakes,
			started: Cell::new(false),
			mock_origin,
			frozen: Cell::new(false),
			anchors: RefCell::default(),
		}
	}

	pub fn run(self: &Rc<Self>, tasks: &Tasks) {
		if self.started.replace(true) {
			return;
		}
		let listening = Rc::downgrade(self);
		tasks.spawn(async move {
			loop {
				let Some(this) = listening.upgrade() else {
					return;
				};
				let ticking = this.player.playing().is_some() && !this.frozen.get();
				let wakes = this.wakes.clone();
				drop(this);
				if ticking {
					zgx::timer::sleep(TICK).await;
				} else if wakes.next().await.is_none() {
					return;
				}
				match listening.upgrade() {
					Some(this) => this.bump(),
					None => return,
				}
			}
		});
	}

	pub fn freeze(&self) {
		self.frozen.set(true);
		self.player.set_sink(Sink::Manual);
	}

	pub fn track(&self, message: &Message, attachment: &Attachment) -> Option<AudioTrack> {
		let mut track = AudioTrack::of(message, attachment)?;
		track.url = bridge::local_audio(&track.url, self.mock_origin.as_deref());
		Some(track)
	}

	pub fn left(&self, channel_id: Snowflake) {
		self.player.left(channel_id);
	}

	pub fn stop(&self) {
		self.player.stop();
	}

	pub fn player(&self) -> &AudioPlayer {
		&self.player
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

	fn watch(&self, cx: &mut Context, track: &AudioTrack) -> Signal<AudioState> {
		let state = cx.state(self.player.state(track));
		let (tick, player, track, shown) = (
			self.tick.clone(),
			self.player.clone(),
			track.clone(),
			state.clone(),
		);
		let effect = self.runtime.effect(move || {
			tick.get();
			shown.set(player.state(&track));
		});
		cx.retain(effect);
		state
	}

	fn bump(&self) {
		self.tick.update(|tick| *tick = tick.wrapping_add(1));
	}

	fn toggle(&self, track: &AudioTrack) {
		self.player.toggle(track);
		self.bump();
	}

	fn seek(&self, track: &AudioTrack, fraction: f32) {
		self.player.seek(track, fraction);
		self.bump();
	}

	fn cycle_speed(&self, track: &AudioTrack) {
		self.player.cycle_speed(track);
		self.bump();
	}

	fn toggle_mute(&self, track: &AudioTrack) {
		self.player.toggle_mute(track);
		self.bump();
	}
}

pub fn voice_message(message: &Message, attachment: &Attachment, env: &Rc<RowEnv>) -> Option<View> {
	let listening = env.pictures.listening();
	let track = listening.track(message, attachment)?;
	let bars = bridge::waveform(attachment, WAVEFORM_BARS);
	let code_font = env.code_font.clone();
	Some(component(move |cx| {
		let state = listening.watch(cx, &track);
		let shown = state.clone();
		row()
			.id(format!("voice-message-{}", track.id))
			.mt(4.)
			.w(tok("voice_message_width"))
			.max_w_percent(100.)
			.h(tok("voice_message_height"))
			.shrink_0()
			.items_center()
			.gap(8.)
			.pl(8.)
			.pr(12.)
			.rounded(PILL_RADIUS)
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.child(play_button(&listening, &track, &state))
			.child(waveform(&listening, &track, &state, bars))
			.child(
				text_signal(move || pill_time(&shown.get()))
					.font_family(code_font.as_str())
					.text_size(tok("font_size_xs"))
					.line_height(tok("line_height_xs"))
					.text_color(tok("text_4"))
					.shrink_0(),
			)
			.child(speed_chip(&listening, &track, &state))
	}))
}

pub fn audio_card(message: &Message, attachment: &Attachment, env: &Rc<RowEnv>) -> Option<View> {
	let listening = env.pictures.listening();
	let track = listening.track(message, attachment)?;
	listening.player().probe(&track);
	let size = attachment
		.size
		.map(|bytes| bridge::file_size(bytes.max(0) as u64));
	let links = env.links.clone();
	let url = attachment.url.clone().unwrap_or_default();
	let filename = attachment.filename.clone();
	let code_font = env.code_font.clone();
	Some(component(move |cx| {
		let state = listening.watch(cx, &track);
		column()
			.id(format!("audio-attachment-{}", track.id))
			.mt(4.)
			.w(tok("audio_card_width"))
			.max_w_percent(100.)
			.gap(12.)
			.p(12.)
			.rounded(tok("radius_md"))
			.bg(tok("bg_3"))
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.child(
				row()
					.items_center()
					.gap(8.)
					.child(icon("file_audio", FILE_ICON, "text_4"))
					.child(
						column()
							.min_w(0.)
							.child(
								text(filename)
									.truncate()
									.text_size(tok("font_size_md"))
									.line_height(tok("line_height_label"))
									.font_weight(500)
									.text_color(tok("link"))
									.cursor(Cursor::Pointer)
									.on_click(move || links.download(&url)),
							)
							.when_some(size, |column, size| {
								column.child(
									text(size)
										.text_size(tok("font_size_xs"))
										.line_height(tok("line_height_xs"))
										.text_color(tok("text_4")),
								)
							}),
					),
			)
			.child(audio_player(&listening, &track, &state, &code_font))
	}))
}

fn audio_player(
	listening: &Rc<Listening>,
	track: &AudioTrack,
	state: &Signal<AudioState>,
	code_font: &str,
) -> View {
	let (toggler, toggled) = (listening.clone(), track.clone());
	let (muter, muted) = (listening.clone(), track.clone());
	let (shown, time, volume) = (state.clone(), state.clone(), state.clone());
	row()
		.id(format!("audio-player-{}", track.id))
		.items_center()
		.gap(12.)
		.h(tok("button_height_sm"))
		.px(8.)
		.rounded(tok("radius_sm"))
		.bg(tok("bg_4"))
		.child(
			div()
				.id(format!("audio-play-{}", track.id))
				.cursor(Cursor::Pointer)
				.on_click(move || toggler.toggle(&toggled))
				.child(switch(
					move || play_glyph(&shown.get()),
					|glyph: &'static str, _| icon(glyph, PLAYER_ICON, "text_2"),
				)),
		)
		.child(
			text_signal(move || player_time(&time.get()))
				.font_family(code_font)
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.text_color(tok("text_3"))
				.shrink_0(),
		)
		.child(seek_bar(listening, track, state))
		.child(
			div()
				.id(format!("audio-mute-{}", track.id))
				.cursor(Cursor::Pointer)
				.on_click(move || muter.toggle_mute(&muted))
				.child(switch(
					move || volume.get().muted,
					|muted: bool, _| {
						icon(
							if muted { "volume_mute_fill" } else { "volume_fill" },
							PLAYER_ICON,
							"text_3",
						)
					},
				)),
		)
}

fn play_button(listening: &Rc<Listening>, track: &AudioTrack, state: &Signal<AudioState>) -> View {
	let (toggler, toggled) = (listening.clone(), track.clone());
	let (shown, dimmed) = (state.clone(), state.clone());
	let (anchors, id) = (listening.clone(), format!("voice-play-{}", track.id));
	div()
		.id(id.clone())
		.on_mount(move |handle| anchors.remember(id.clone(), handle))
		.size(PLAY_BUTTON, PLAY_BUTTON)
		.shrink_0()
		.rounded(PLAY_BUTTON / 2.)
		.bg(tok("accent_3"))
		.items_center()
		.justify_center()
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			if dimmed.get().phase == AudioPhase::Loading {
				Styles::new().opacity(LOADING_OPACITY)
			} else {
				Styles::new()
			}
		})
		.on_click(move || toggler.toggle(&toggled))
		.child(switch(
			move || play_glyph(&shown.get()),
			|glyph: &'static str, _| icon(glyph, PLAY_ICON, "text_0"),
		))
}

fn waveform(
	listening: &Rc<Listening>,
	track: &AudioTrack,
	state: &Signal<AudioState>,
	bars: Vec<f32>,
) -> View {
	let count = bars.len().max(1) as f32;
	seekable(
		row()
			.id(format!("voice-waveform-{}", track.id))
			.grow()
			.min_w(0.)
			.h(tok("waveform_bar_max_height"))
			.items_center()
			.justify_between()
			.gap(tok("waveform_bar_width"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.children(bars.into_iter().enumerate().map(|(index, height)| {
				let played = state.clone();
				let start = index as f32 / count;
				div()
					.w(tok("waveform_bar_width"))
					.h(BAR_MAX_HEIGHT * height)
					.shrink_0()
					.rounded(BAR_WIDTH)
					.reactive_style(move || {
						let state = played.get();
						let lit = state.started() && start < state.progress();
						Styles::new().bg(tok(if lit { "accent_2" } else { "text_4" }))
					})
			})),
		listening,
		track,
		format!("voice-waveform-{}", track.id),
	)
}

fn seek_bar(listening: &Rc<Listening>, track: &AudioTrack, state: &Signal<AudioState>) -> View {
	let played = state.clone();
	seekable(
		div()
			.id(format!("audio-seek-{}", track.id))
			.grow()
			.min_w(0.)
			.h(tok("progress_height"))
			.rounded(tok("progress_height"))
			.bg(tok("bg_2"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.child(
				div()
					.h_full()
					.rounded(tok("progress_height"))
					.bg(tok("accent_2"))
					.reactive_style(move || Styles::new().w_percent(played.get().progress() * 100.)),
			),
		listening,
		track,
		format!("audio-seek-{}", track.id),
	)
}

fn seekable(view: View, listening: &Rc<Listening>, track: &AudioTrack, id: String) -> View {
	let node: Rc<RefCell<Option<NodeHandle>>> = Rc::default();
	let mounted = node.clone();
	let (seeker, sought, anchors) = (listening.clone(), track.clone(), listening.clone());
	view.on_mount(move |handle| {
		anchors.remember(id.clone(), handle.clone());
		mounted.replace(Some(handle));
	})
	.on_event(move |event| {
		let InputEvent::PointerDown {
			x,
			button: PointerButton::Primary,
			..
		} = event.event
		else {
			return;
		};
		if event.phase == EventPhase::Capture {
			return;
		}
		let Some(fraction) = node.borrow().as_ref().and_then(|handle| fraction_at(handle, x)) else {
			return;
		};
		seeker.seek(&sought, fraction);
		event.prevent_default();
		event.stop_propagation();
	})
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

fn speed_chip(listening: &Rc<Listening>, track: &AudioTrack, state: &Signal<AudioState>) -> View {
	let (cycler, cycled) = (listening.clone(), track.clone());
	let shown = state.clone();
	div()
		.id(format!("voice-speed-{}", track.id))
		.h(SPEED_HEIGHT)
		.px(6.)
		.shrink_0()
		.items_center()
		.rounded(tok("radius_xs"))
		.bg(tok("bg_2"))
		.cursor(Cursor::Pointer)
		.on_click(move || cycler.cycle_speed(&cycled))
		.child(
			text_signal(move || shown.get().speed.label().to_owned())
				.text_size(tok("font_size_xs"))
				.line_height(tok("line_height_xs"))
				.font_weight(600)
				.text_color(tok("text_3")),
		)
}

fn play_glyph(state: &AudioState) -> &'static str {
	match state.phase {
		AudioPhase::Playing => "pause_fill",
		AudioPhase::Failed => "warning",
		AudioPhase::Idle | AudioPhase::Loading | AudioPhase::Paused => "play_fill",
	}
}

fn pill_time(state: &AudioState) -> String {
	if state.phase == AudioPhase::Failed {
		return "Can't play".to_owned();
	}
	match (state.started() || !state.position.is_zero(), state.duration) {
		(true, _) => bridge::audio_clock(state.position),
		(false, Some(duration)) => bridge::audio_clock(duration),
		(false, None) => "-:--".to_owned(),
	}
}

fn player_time(state: &AudioState) -> String {
	if state.phase == AudioPhase::Failed {
		return "Can't play this file".to_owned();
	}
	let total = state
		.duration
		.map_or_else(|| "-:--".to_owned(), bridge::audio_clock);
	format!("{} / {total}", bridge::audio_clock(state.position))
}
