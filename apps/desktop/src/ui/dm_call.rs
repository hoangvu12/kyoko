use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use audio::Cue;
use bridge::IncomingCall;
use media::ImageName;
use protocol::{ChannelType, Snowflake};
use session::Command;
use state::{State, VoiceStatus};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record, snowflake};
use crate::rail::Icons;

const CALLER_AVATAR_POINTS: f64 = 80.;
const PERSON_AVATAR_POINTS: f64 = 80.;
const CAPTURE_SPEECH_FRAMES: usize = 1_500;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Person {
	id: Snowflake,
	name: String,
	ringing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Area {
	channel_id: Option<Snowflake>,
	joined: bool,
	name: String,
	people: Vec<Person>,
}

pub(super) struct DmCallView {
	incoming: Shared,
	area: Shared,
	ringer: Option<Cue>,
	shown: RefCell<Option<(Option<IncomingCall>, Area)>>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	visible: RefCell<Vec<String>>,
}

impl DmCallView {
	pub(super) fn new(host: &ScriptHost, mock: bool) -> Self {
		Self {
			incoming: host.shared("voice.incoming", incoming_value(None, None)),
			area: host.shared("voice.dm_call", area_value(&Area::default(), &[])),
			ringer: (!mock).then(|| Cue::looping(audio::ringtone)),
			shown: RefCell::new(None),
			avatars: RefCell::new(HashMap::new()),
			visible: RefCell::new(Vec::new()),
		}
	}

	pub(super) fn reset(&self) {
		self.ring(false);
		self.shown.borrow_mut().take();
		self.avatars.borrow_mut().clear();
		self.visible.borrow_mut().clear();
		self.incoming.set(incoming_value(None, None));
		self.area.set(area_value(&Area::default(), &[]));
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.visible.borrow().clone()
	}

	fn ring(&self, on: bool) {
		match &self.ringer {
			Some(cue) if on => cue.play(),
			Some(cue) => cue.stop(),
			None => {}
		}
	}

	pub(super) fn ringing(&self) -> bool {
		self.ringer.as_ref().is_some_and(Cue::is_playing)
	}

	fn avatar(&self, state: &State, user_id: Snowflake, points: f64, icons: &Icons<'_>) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone());
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(icons.cdn, user, points, icons.scale, false)?;
		let name = icons.gallery.request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}
}

fn area_of(state: &State, open: Option<Snowflake>) -> Area {
	let Some(channel_id) = open.filter(|channel_id| {
		state
			.channel(*channel_id)
			.is_some_and(|channel| channel.guild_id.is_none())
	}) else {
		return Area::default();
	};
	let joined = state
		.voice()
		.is_some_and(|voice| voice.guild_id.is_none() && voice.channel_id == channel_id);
	let Some(call) = state.call(channel_id).filter(|call| !call.unavailable) else {
		return if joined {
			Area {
				channel_id: Some(channel_id),
				joined,
				name: bridge::private_name(state, channel_id),
				people: Vec::new(),
			}
		} else {
			Area::default()
		};
	};
	if call.voice_states.is_empty() && !joined {
		return Area::default();
	}
	let mut people: Vec<Person> = call
		.participants()
		.map(|id| Person {
			id,
			name: state.display_name(None, id),
			ringing: false,
		})
		.collect();
	people.extend(call.ringing.iter().filter(|id| !call.has(**id)).map(|id| Person {
		id: *id,
		name: state.display_name(None, *id),
		ringing: true,
	}));
	Area {
		channel_id: Some(channel_id),
		joined,
		name: bridge::private_name(state, channel_id),
		people,
	}
}

impl Link {
	pub(super) fn install_dm_call(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("voice.start_call", move |argument| {
			if let (Some(link), Some((channel_id, video))) = (actions.upgrade(), call_choice(&argument)) {
				link.open(None, channel_id);
				link.send(Command::StartCall { channel_id, video });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.accept_call", move |argument| {
			if let (Some(link), Some((channel_id, video))) = (actions.upgrade(), call_choice(&argument)) {
				link.dm_call.ring(false);
				link.open(None, channel_id);
				link.send(Command::StartCall { channel_id, video });
			}
		});
		let actions = Rc::downgrade(link);
		host.action("voice.decline_call", move |argument| {
			if let (Some(link), Some(channel_id)) =
				(actions.upgrade(), field(&argument, "id").and_then(snowflake))
			{
				link.dm_call.ring(false);
				link.send(Command::DeclineCall { channel_id });
			}
		});
	}

	pub(super) fn publish_dm_call(&self, state: &State) {
		let incoming = bridge::incoming_call(state);
		let open = self
			.top_channel(self.timeline.borrow().open())
			.map(|open| open.channel_id);
		let area = area_of(state, open);
		self.dm_call
			.ring(incoming.as_ref().is_some_and(|call| call.ring_sound));
		let next = (incoming, area);
		if self.dm_call.shown.borrow().as_ref() == Some(&next) {
			return;
		}
		let icons = Icons {
			gallery: &self.images.gallery,
			cdn: &self.images.cdn,
			scale: self.scale_factor(),
		};
		let caller_avatar = next
			.0
			.as_ref()
			.and_then(|call| call.caller)
			.and_then(|caller| self.dm_call.avatar(state, caller, CALLER_AVATAR_POINTS, &icons));
		let faces: Vec<Option<String>> = next
			.1
			.people
			.iter()
			.map(|person| {
				self.dm_call
					.avatar(state, person.id, PERSON_AVATAR_POINTS, &icons)
			})
			.collect();
		self.dm_call
			.incoming
			.set(incoming_value(next.0.as_ref(), caller_avatar.as_deref()));
		self.dm_call.area.set(area_value(&next.1, &faces));
		let names: Vec<String> = caller_avatar
			.into_iter()
			.chain(faces.into_iter().flatten())
			.collect();
		let changed = *self.dm_call.visible.borrow() != names;
		*self.dm_call.visible.borrow_mut() = names;
		*self.dm_call.shown.borrow_mut() = Some(next);
		if changed {
			self.refresh_visible();
		}
	}

	pub(super) fn dm_call_images(&self) -> Vec<String> {
		self.dm_call.images()
	}

	pub(super) fn incoming_ringing(&self) -> bool {
		self.dm_call.ringing()
	}

	pub(super) fn incoming_shown(&self) -> bool {
		self.dm_call
			.shown
			.borrow()
			.as_ref()
			.is_some_and(|(incoming, _)| incoming.is_some())
	}

	pub(super) async fn drive_incoming_call(&self) {
		self.until_capture(Link::incoming_shown).await;
		tracing::info!(
			ringing = self.incoming_ringing(),
			"the incoming call shows for the capture"
		);
	}

	pub(super) async fn drive_dm_call(&self) {
		let target = self.state().and_then(|shared| {
			shared
				.read()
				.private_channels()
				.filter(|channel| channel.kind == ChannelType::Dm)
				.map(|channel| channel.id)
				.min()
		});
		let Some(channel_id) = target else {
			tracing::warn!("no DM to call");
			return;
		};
		self.open(None, channel_id);
		self.send(Command::StartCall {
			channel_id,
			video: false,
		});
		self.until_capture(|link| link.dm_call_shown() && answered(link, channel_id))
			.await;
		let connection = self
			.flow
			.borrow()
			.session()
			.and_then(|session| session.voice_connection());
		if let Some(connection) = connection {
			for index in 0..CAPTURE_SPEECH_FRAMES {
				connection.feed_input(&super::voice_call::capture_tone(index));
			}
		}
		self.until_capture(|link| !link.shown_speaking().is_empty()).await;
		tracing::info!("the DM call shows for the capture");
	}

	pub(super) fn dm_call_shown(&self) -> bool {
		self.dm_call
			.shown
			.borrow()
			.as_ref()
			.is_some_and(|(_, area)| area.channel_id.is_some())
	}
}

fn answered(link: &Link, channel_id: Snowflake) -> bool {
	link.state().is_some_and(|shared| {
		let state = shared.read();
		let connected = state
			.voice()
			.is_some_and(|voice| voice.channel_id == channel_id && voice.status == VoiceStatus::Connected);
		connected
			&& state
				.call(channel_id)
				.is_some_and(|call| call.voice_states.len() >= 2)
	})
}

fn call_choice(argument: &Value) -> Option<(Snowflake, bool)> {
	let channel_id = field(argument, "id").and_then(snowflake)?;
	let video = matches!(field(argument, "video"), Some(Value::Bool(true)));
	Some((channel_id, video))
}

fn host_image(name: Option<&str>) -> String {
	name.map(|name| format!("host:{name}")).unwrap_or_default()
}

fn incoming_value(call: Option<&IncomingCall>, avatar: Option<&str>) -> Value {
	record([
		("open", call.is_some().into()),
		(
			"channel_id",
			call.map(|call| call.channel_id.to_string())
				.unwrap_or_default()
				.as_str()
				.into(),
		),
		("name", call.map_or("", |call| call.name.as_str()).into()),
		("avatar", host_image(avatar).as_str().into()),
		("group", call.is_some_and(|call| call.group).into()),
	])
}

fn area_value(area: &Area, faces: &[Option<String>]) -> Value {
	record([
		("shown", area.channel_id.is_some().into()),
		(
			"channel_id",
			area.channel_id
				.map(|id| id.to_string())
				.unwrap_or_default()
				.as_str()
				.into(),
		),
		("joined", area.joined.into()),
		("name", area.name.as_str().into()),
		(
			"people",
			Value::List(
				area.people
					.iter()
					.zip(faces.iter().map(Option::as_deref).chain(std::iter::repeat(None)))
					.map(|(person, face)| {
						record([
							("id", person.id.to_string().as_str().into()),
							("name", person.name.as_str().into()),
							("avatar", host_image(face).as_str().into()),
							("ringing", person.ringing.into()),
						])
					})
					.collect(),
			),
		),
	])
}
