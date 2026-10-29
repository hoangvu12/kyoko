use protocol::{ChannelType, Snowflake};
use serde_json::Value;
use state::LocalEvent;
use tokio::time::sleep;

use super::voice_link::Target;
use super::{Core, Internal};

const NOT_A_CALL_CHANNEL: &str = "Calls start only from a direct message or a group.";

fn channel_of(payload: &Value) -> Option<Snowflake> {
	payload["channel_id"].as_str()?.parse().ok().map(Snowflake)
}

impl Core {
	pub(super) fn start_call(&mut self, channel_id: Snowflake, video: bool) {
		let (callable, ongoing) = {
			let state = self.state.read();
			let callable = state
				.channel(channel_id)
				.is_some_and(|channel| matches!(channel.kind, ChannelType::Dm | ChannelType::GroupDm));
			let ongoing = state
				.call(channel_id)
				.is_some_and(|call| !call.voice_states.is_empty());
			(callable, ongoing)
		};
		if !callable {
			self.reject(NOT_A_CALL_CHANNEL);
			return;
		}
		let target = Target {
			guild_id: None,
			channel_id,
		};
		if self.voice.target == Some(target) {
			if video {
				self.set_self_video(true);
			}
			return;
		}
		self.enter_voice(target);
		self.voice.video_on_join = video;
		self.voice.ring_after_join = (!ongoing).then_some(channel_id);
	}

	pub(super) fn decline_call(&mut self, channel_id: Snowflake) {
		let rung = {
			let state = self.state.read();
			let me = state.current_user_id();
			state
				.call(channel_id)
				.is_some_and(|call| me.is_some_and(|me| call.rings(me)))
		};
		self.apply_local(LocalEvent::CallDeclined(channel_id));
		if rung {
			self.spawn_rest("stop ringing", move |rest| async move {
				rest.stop_ringing(channel_id, None).await
			});
		}
	}

	pub(super) fn call_changed(&mut self, payload: &Value) {
		let Some(channel_id) = channel_of(payload) else {
			return;
		};
		self.ring_when_started(channel_id);
		self.follow_ring(channel_id);
	}

	pub(super) fn call_ended(&mut self, payload: &Value) {
		if let Some(channel_id) = channel_of(payload) {
			self.voice.rings.remove(&channel_id);
			if self.voice.ring_after_join == Some(channel_id) {
				self.voice.ring_after_join = None;
			}
		}
	}

	pub(super) fn ring_timed_out(&mut self, channel_id: Snowflake, serial: u64) {
		if self.voice.rings.get(&channel_id) != Some(&serial) {
			return;
		}
		self.voice.rings.remove(&channel_id);
		let ringing = self
			.state
			.read()
			.incoming_calls()
			.any(|call| call.channel_id == channel_id);
		if ringing {
			self.apply_local(LocalEvent::CallDeclined(channel_id));
		}
	}

	fn ring_when_started(&mut self, channel_id: Snowflake) {
		if self.voice.ring_after_join != Some(channel_id) {
			return;
		}
		let rung_already = {
			let state = self.state.read();
			let Some(me) = state.current_user_id() else {
				return;
			};
			let Some(call) = state.call(channel_id) else {
				return;
			};
			if !call.has(me) {
				return;
			}
			!call.ringing.is_empty()
		};
		self.voice.ring_after_join = None;
		if !rung_already {
			self.spawn_rest("ring", move |rest| async move {
				rest.ring_call(channel_id, None).await
			});
		}
	}

	fn follow_ring(&mut self, channel_id: Snowflake) {
		let incoming = self
			.state
			.read()
			.incoming_calls()
			.any(|call| call.channel_id == channel_id);
		if !incoming {
			self.voice.rings.remove(&channel_id);
			return;
		}
		if self.voice.rings.contains_key(&channel_id) {
			return;
		}
		self.voice.ring_serial += 1;
		let serial = self.voice.ring_serial;
		self.voice.rings.insert(channel_id, serial);
		let timeout = self.voice_shared.setup.lock().ring_timeout;
		let internal = self.internal.clone();
		self.runtime.spawn(async move {
			sleep(timeout).await;
			let _ = internal.send(Internal::RingTimedOut { channel_id, serial });
		});
	}
}
