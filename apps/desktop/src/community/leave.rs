use std::rc::Rc;

use protocol::Snowflake;
use session::Command;
use zgx::compose::prelude::*;

use super::CommunityLayer;
use super::parts::{Tone, action_button, body, footer, hint, panel};

const WIDTH: f32 = 440.;

pub fn view(layer: &Rc<CommunityLayer>, guild_id: Snowflake, name: &str) -> View {
	let (leave, close, cancel) = (layer.clone(), layer.clone(), layer.clone());
	panel(
		WIDTH,
		format!("Leave {name}?"),
		move || close.close(),
		body()
			.child(hint(
				"You will not be able to rejoin this server unless you are invited again.",
			))
			.child(footer(
				"Cancel",
				action_button("Leave Server", Tone::Danger, move || {
					leave.send(Command::LeaveGuild { guild_id });
					leave.close();
				}),
				move || cancel.close(),
			)),
	)
}
