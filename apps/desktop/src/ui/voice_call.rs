use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use bridge::{CallTile, Inbox, Pace, SpeakingPace, VoiceCall, VoiceVideo};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use state::{State, USER_VOLUME_MAX};
use zgx::reactive::{Runtime, Signal};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record, snowflake};
use crate::rail::Icons;

const TILE_AVATAR_POINTS: f64 = 80.;
const CAPTURE_SPEECH_FRAMES: usize = 1_500;
const CAPTURE_TONE_HZ: f32 = 440.;
const CAPTURE_TONE_LEVEL: f32 = 0.2;
const CAPTURE_RATE: f32 = 48_000.;
const CAPTURE_FRAME_SAMPLES: usize = 960;

pub(super) struct CallView {
	shown: Signal<bool>,
	anchor: Cell<Option<Snowflake>>,
	call: Shared,
	speaking: Shared,
	published: RefCell<Option<(bool, Option<VoiceCall>)>>,
	pace: RefCell<SpeakingPace>,
	waiting: Cell<bool>,
	wakes: Inbox<Instant>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	visible: RefCell<Vec<String>>,
}

impl CallView {
	pub(super) fn new(host: &ScriptHost, runtime: &Runtime) -> Self {
		Self {
			shown: runtime.signal(false),
			anchor: Cell::new(None),
			call: host.shared("voice.call", call_value(false, None, &[])),
			speaking: host.shared("voice.speaking", Value::Map(Rc::new([]))),
			published: RefCell::new(None),
			pace: RefCell::new(SpeakingPace::default()),
			waiting: Cell::new(false),
			wakes: Inbox::new(),
			avatars: RefCell::new(HashMap::new()),
			visible: RefCell::new(Vec::new()),
		}
	}

	pub(super) fn shown(&self) -> Signal<bool> {
		self.shown.clone()
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.visible.borrow().clone()
	}

	pub(super) fn close(&self) {
		self.wakes.close();
	}

	pub(super) fn reset(&self) {
		self.anchor.set(None);
		if self.shown.with_untracked(|shown| *shown) {
			self.shown.set(false);
		}
		self.published.borrow_mut().take();
		self.pace.borrow_mut().forget();
		self.avatars.borrow_mut().clear();
		self.visible.borrow_mut().clear();
		self.call.set(call_value(false, None, &[]));
		self.speaking.set(Value::Map(Rc::new([])));
	}

	fn open(&self) -> bool {
		self.shown.with_untracked(|shown| *shown)
	}

	fn set_open(&self, open: bool) {
		if self.open() != open {
			self.shown.set(open);
		}
	}

	fn avatar(&self, state: &State, user_id: Snowflake, icons: &Icons<'_>) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone());
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(icons.cdn, user, TILE_AVATAR_POINTS, icons.scale, false)?;
		let name = icons.gallery.request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}
}

impl Link {
	pub(super) fn install_call(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.open_chat", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_call_chat();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.user_volume", move |argument| {
			if let (Some(link), Some((user_id, volume))) = (actions.upgrade(), user_volume(&argument)) {
				link.send(Command::SetUserVolume { user_id, volume });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.local_mute", move |argument| {
			if let (Some(link), Some((user_id, muted))) = (actions.upgrade(), local_mute(&argument)) {
				link.send(Command::SetUserMuted { user_id, muted });
			}
		});
	}

	pub(super) fn open_call(&self) {
		let place = self.state().and_then(|shared| {
			shared
				.read()
				.voice()
				.map(|voice| (voice.guild_id, voice.channel_id))
		});
		let Some((guild_id, channel_id)) = place else {
			return;
		};
		let selected = self.sidebar.sidebar().selected();
		if guild_id.is_some() && selected != guild_id {
			self.open(guild_id, channel_id);
		} else {
			self.select_channel(channel_id);
		}
		let open = self.top_channel(self.timeline.borrow().open());
		self.call.anchor.set(open.map(|open| open.channel_id));
		self.call.set_open(true);
		if let Some(shared) = self.state() {
			self.publish_call(&shared.read());
		}
	}

	fn open_call_chat(&self) {
		self.call.anchor.set(None);
		self.call.set_open(false);
		if let Some(channel_id) = self.voice_channel() {
			self.select_channel(channel_id);
		}
		if let Some(shared) = self.state() {
			self.publish_call(&shared.read());
		}
	}

	pub(super) fn sync_call(&self, open_channel: Option<Snowflake>) {
		if self.call.open() && open_channel != self.call.anchor.get() {
			self.call.anchor.set(None);
			self.call.set_open(false);
			if let Some(shared) = self.state() {
				self.publish_call(&shared.read());
			}
		}
	}

	pub(super) fn publish_call(&self, state: &State) {
		let call = bridge::voice_call(state);
		if call.is_none() && self.call.open() {
			self.call.anchor.set(None);
			self.call.set_open(false);
		}
		let open = self.call.open() && call.is_some();
		let next = (open, call);
		if self.call.published.borrow().as_ref() != Some(&next) {
			let icons = Icons {
				gallery: &self.images.gallery,
				cdn: &self.images.cdn,
				scale: self.scale_factor(),
			};
			let tiles: Vec<(&CallTile, Option<String>)> = next
				.1
				.iter()
				.flat_map(|call| &call.tiles)
				.map(|tile| (tile, self.call.avatar(state, tile.user_id, &icons)))
				.collect();
			let names: Vec<String> = tiles.iter().filter_map(|(_, name)| name.clone()).collect();
			self.call.call.set(call_value(next.0, next.1.as_ref(), &tiles));
			drop(tiles);
			let changed = *self.call.visible.borrow() != names;
			*self.call.visible.borrow_mut() = names;
			*self.call.published.borrow_mut() = Some(next);
			if changed {
				self.refresh_visible();
			}
		}
		self.publish_speaking(state);
	}

	fn publish_speaking(&self, state: &State) {
		let offered = self
			.call
			.pace
			.borrow_mut()
			.offer(bridge::voice_speaking(state), Instant::now());
		match offered {
			Pace::Publish(speaking) => {
				self.call.speaking.set(speaking_value(&speaking));
			}
			Pace::Wait(due) => {
				if !self.call.waiting.replace(true) {
					self.call.wakes.push(due);
				}
			}
			Pace::Same => {}
		}
	}

	pub(super) async fn follow_speaking(self: Rc<Self>) {
		while let Some(dues) = self.call.wakes.next().await {
			let Some(due) = dues.into_iter().min() else {
				continue;
			};
			zgx::timer::sleep(due.saturating_duration_since(Instant::now())).await;
			self.call.waiting.set(false);
			if let Some(shared) = self.state() {
				let state = shared.read();
				self.window.runtime.batch(|| self.publish_speaking(&state));
			}
		}
	}

	pub(super) async fn drive_voice_call(&self) {
		self.drive_voice_panel().await;
		self.open_call();
		let connection = self
			.flow
			.borrow()
			.session()
			.and_then(|session| session.voice_connection());
		if let Some(connection) = connection {
			for index in 0..CAPTURE_SPEECH_FRAMES {
				connection.feed_input(&capture_tone(index));
			}
		}
		self.until_capture(|link| link.call_open() && !link.shown_speaking().is_empty())
			.await;
		tracing::info!("opened the call view for the capture");
	}

	pub(super) fn call_images(&self) -> Vec<String> {
		self.call.images()
	}

	pub(super) fn shown_speaking(&self) -> Vec<Snowflake> {
		self.call.pace.borrow().shown().to_vec()
	}

	pub(super) fn call_open(&self) -> bool {
		self.call.open()
	}
}

fn user_volume(argument: &Value) -> Option<(Snowflake, u8)> {
	let user_id = field(argument, "id").and_then(snowflake)?;
	let volume = match field(argument, "volume")? {
		Value::Num(volume) if volume.is_finite() => volume.round().clamp(0., f64::from(USER_VOLUME_MAX)),
		_ => return None,
	};
	Some((user_id, volume as u8))
}

fn local_mute(argument: &Value) -> Option<(Snowflake, bool)> {
	let user_id = field(argument, "id").and_then(snowflake)?;
	match field(argument, "muted")? {
		Value::Bool(muted) => Some((user_id, *muted)),
		_ => None,
	}
}

fn speaking_value(speaking: &[Snowflake]) -> Value {
	let mut keys: Vec<String> = speaking.iter().map(Snowflake::to_string).collect();
	keys.sort_unstable();
	Value::Map(
		keys.into_iter()
			.map(|key| (Rc::from(key), Value::Bool(true)))
			.collect(),
	)
}

fn call_value(open: bool, call: Option<&VoiceCall>, tiles: &[(&CallTile, Option<String>)]) -> Value {
	record([
		("open", open.into()),
		(
			"channel_id",
			call.map(|call| call.channel_id.to_string())
				.unwrap_or_default()
				.as_str()
				.into(),
		),
		("name", call.map_or("", |call| call.name.as_str()).into()),
		("connected", call.is_some_and(|call| call.connected).into()),
		(
			"tiles",
			Value::List(
				tiles
					.iter()
					.map(|(tile, avatar)| tile_value(tile, avatar.as_deref()))
					.collect(),
			),
		),
	])
}

fn tile_value(tile: &CallTile, avatar: Option<&str>) -> Value {
	let video = match tile.video {
		Some(VoiceVideo::Camera) => "camera",
		Some(VoiceVideo::Screen) => "screen",
		None => "",
	};
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	record([
		("key", format!("t{}", tile.user_id).as_str().into()),
		("id", tile.user_id.to_string().as_str().into()),
		("name", tile.name.as_str().into()),
		("avatar", avatar.as_str().into()),
		("me", tile.me.into()),
		("muted", tile.muted.into()),
		("deafened", tile.deafened.into()),
		("video", video.into()),
		("live", tile.live.into()),
		("volume", f64::from(tile.volume).into()),
		("local_muted", tile.local_muted.into()),
	])
}

fn capture_tone(index: usize) -> Vec<i16> {
	(0..CAPTURE_FRAME_SAMPLES)
		.map(|sample| {
			let at = (index * CAPTURE_FRAME_SAMPLES + sample) as f32 / CAPTURE_RATE;
			let wave = (at * CAPTURE_TONE_HZ * std::f32::consts::TAU).sin() * CAPTURE_TONE_LEVEL;
			(wave * f32::from(i16::MAX)) as i16
		})
		.collect()
}
