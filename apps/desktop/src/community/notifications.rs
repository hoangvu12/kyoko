use std::rc::Rc;

use protocol::{NotificationLevel, Snowflake};
use session::Command;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::CommunityLayer;
use super::parts::{Tone, action_button, body, footer, panel, switch_row};
use crate::timeline::{TypeSize, tok};

const WIDTH: f32 = 440.;
const ROW_HEIGHT: f32 = 40.;
const RING: f32 = 20.;
const DOT: f32 = 10.;
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
	let runtime = &layer.runtime;
	let level = runtime.signal(current.level);
	let everyone = runtime.signal(current.suppress_everyone);
	let roles = runtime.signal(current.suppress_roles);
	let push = runtime.signal(true);
	let (save, close, cancel) = (layer.clone(), layer.clone(), layer.clone());
	let chosen = (level.clone(), everyone.clone(), roles.clone(), push.clone());
	panel(
		WIDTH,
		"Notification Settings",
		move || close.close(),
		body()
			.child(
				column().w_full().gap(4.).children(
					LEVELS
						.iter()
						.map(|(label, value)| level_row(label, *value, &level)),
				),
			)
			.child(switch_row("Suppress @everyone and @here", None, everyone))
			.child(switch_row("Suppress All Role @mentions", None, roles))
			.child(switch_row("Mobile Push Notifications", None, push))
			.child(footer(
				"Cancel",
				action_button("Done", Tone::Primary, move || {
					let (level, everyone, roles, push) = &chosen;
					save.send(Command::SetGuildNotifications {
						guild_id: Some(guild_id),
						level: level.with_untracked(|level| *level),
						suppress_everyone: everyone.with_untracked(|value| *value),
						suppress_roles: roles.with_untracked(|value| *value),
						mobile_push: push.with_untracked(|value| *value),
					});
					save.close();
				}),
				move || cancel.close(),
			)),
	)
}

fn level_row(label: &str, value: NotificationLevel, level: &Signal<NotificationLevel>) -> View {
	let (row_level, ring_level, dot_level, pick) =
		(level.clone(), level.clone(), level.clone(), level.clone());
	row()
		.w_full()
		.h(ROW_HEIGHT)
		.items_center()
		.gap(12.)
		.px(12.)
		.rounded(tok("radius_sm"))
		.cursor(Cursor::Pointer)
		.reactive_style(move || {
			Styles::new().bg(tok(if row_level.get() == value {
				"modifier_selected"
			} else {
				"background_secondary"
			}))
		})
		.hover(|style| style.bg(tok("modifier_hover")))
		.on_click(move || {
			pick.set(value);
		})
		.child(
			div()
				.size(RING, RING)
				.shrink_0()
				.rounded(RING)
				.border(2.)
				.items_center()
				.justify_center()
				.reactive_style(move || {
					Styles::new().border_color(tok(if ring_level.get() == value {
						"brand"
					} else {
						"interactive_muted"
					}))
				})
				.child(
					div()
						.size(DOT, DOT)
						.rounded(DOT)
						.bg(tok("brand"))
						.reactive_style(move || {
							if dot_level.get() == value {
								Styles::new().visible()
							} else {
								Styles::new().invisible()
							}
						}),
				),
		)
		.child(
			text(label.to_owned())
				.type_size("font_md")
				.font_weight(600)
				.text_color(tok("header_primary")),
		)
}
