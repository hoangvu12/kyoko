use std::rc::Rc;

use protocol::{NotificationLevel, Snowflake};
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::CommunityLayer;
use super::parts::{Tone, action_button, body, caption, panel_with, switch_row};
use crate::kit;
use crate::timeline::tok;

const WIDTH: f32 = 440.;
const LEVELS: [(&str, NotificationLevel); 3] = [
	("All Messages", NotificationLevel::AllMessages),
	("Only @mentions", NotificationLevel::OnlyMentions),
	("Nothing", NotificationLevel::NoMessages),
];

pub fn view(layer: &Rc<CommunityLayer>, guild_id: Snowflake) -> View {
	let current = layer
		.read(|state| bridge::guild_notifications(state, guild_id))
		.unwrap_or(bridge::GuildNotifications {
			level: NotificationLevel::AllMessages,
			suppress_everyone: false,
			suppress_roles: false,
		});
	let server = layer
		.read(|state| state.guild(guild_id).map(|guild| guild.name.clone()))
		.flatten();
	let runtime = &layer.runtime;
	let level = runtime.signal(current.level);
	let everyone = runtime.signal(current.suppress_everyone);
	let roles = runtime.signal(current.suppress_roles);
	let push = runtime.signal(true);
	let (save, close) = (layer.clone(), layer.clone());
	let chosen = (level.clone(), everyone.clone(), roles.clone(), push.clone());
	panel_with(
		WIDTH,
		"Notification Settings",
		server,
		move || close.close(),
		body()
			.child(caption("Server Notification Settings"))
			.child(
				column().w_full().gap(tok("space_8")).children(
					LEVELS
						.iter()
						.map(|(label, value)| level_row(label, *value, &level)),
				),
			)
			.child(kit::divider())
			.child(switch_row("Suppress @everyone and @here", None, everyone))
			.child(switch_row("Suppress All Role @mentions", None, roles))
			.child(switch_row("Mobile Push Notifications", None, push))
			.child(kit::footer(vec![action_button(
				"Done",
				Tone::Primary,
				move || {
					let (level, everyone, roles, push) = &chosen;
					save.send(Command::SetGuildNotifications {
						guild_id: Some(guild_id),
						level: level.with_untracked(|level| *level),
						suppress_everyone: everyone.with_untracked(|value| *value),
						suppress_roles: roles.with_untracked(|value| *value),
						mobile_push: push.with_untracked(|value| *value),
					});
					save.close();
				},
			)])),
	)
}

fn level_row(label: &str, value: NotificationLevel, level: &Signal<NotificationLevel>) -> View {
	let (shown, pick) = (level.clone(), level.clone());
	kit::radio_row(
		label,
		move || shown.get() == value,
		move || {
			pick.set(value);
		},
	)
}
