use protocol::{ChannelType, Snowflake, StatusKind};
use state::State;

use crate::voice_call::private_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
	pub channel_id: Snowflake,
	pub caller: Option<Snowflake>,
	pub name: String,
	pub group: bool,
	pub ring_sound: bool,
}

pub fn incoming_call(state: &State) -> Option<IncomingCall> {
	let me = state.current_user_id();
	let call = state
		.incoming_calls()
		.min_by_key(|call| (call.message_id, call.channel_id))?;
	let group = state
		.channel(call.channel_id)
		.is_some_and(|channel| channel.kind == ChannelType::GroupDm);
	let caller = call.participants().find(|user_id| Some(*user_id) != me);
	let name = match (group, caller) {
		(false, Some(caller)) => state.display_name(None, caller),
		_ => private_name(state, call.channel_id),
	};
	Some(IncomingCall {
		channel_id: call.channel_id,
		caller,
		name,
		group,
		ring_sound: state.own_status() != StatusKind::Dnd,
	})
}
