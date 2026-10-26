use std::rc::Rc;

use bridge::MessageEntry;
use session::{DeletedLook, HistoryLook};
use zgx::compose::prelude::*;
use zgx::reactive::Signal;

use super::body::{self, Look};
use super::rows::{RowEnv, tok};
use crate::plugin_menu::AccessoryClick;

const HISTORY_SPOILERS: usize = 10_000;
const DELETED: (&str, &str) = ("(deleted)", "red_2");
const EDITED: (&str, &str) = ("(edited)", "text_5");

fn deleted_look(entry: &MessageEntry) -> Option<DeletedLook> {
	entry
		.look
		.as_ref()
		.and_then(|look| look.deleted)
		.filter(|_| entry.deleted)
}

pub fn tinted(entry: &MessageEntry) -> bool {
	deleted_look(entry) == Some(DeletedLook::Tint)
}

pub fn text_color(entry: &MessageEntry) -> &'static str {
	if entry.failed {
		return "red_2";
	}
	if !entry.deleted {
		return "text_3";
	}
	match deleted_look(entry) {
		Some(DeletedLook::Text) => "red_2",
		Some(DeletedLook::Tint) => "text_3",
		_ => "text_5",
	}
}

pub fn trailer(entry: &MessageEntry) -> Option<(&'static str, &'static str)> {
	if entry.deleted {
		Some(DELETED)
	} else if entry.edited {
		Some(EDITED)
	} else {
		None
	}
}

fn collapsed(entry: &MessageEntry) -> bool {
	!entry.history.is_empty()
		&& entry
			.look
			.as_ref()
			.is_some_and(|look| look.history == Some(HistoryLook::Collapsed))
}

fn history_shown(entry: &MessageEntry, env: &RowEnv) -> bool {
	!entry.history.is_empty() && (!collapsed(entry) || env.opened.borrow().contains(&entry.id))
}

pub fn history_toggle(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>) -> Option<Rc<dyn Fn()>> {
	if !collapsed(entry) {
		return None;
	}
	let (env, bump, id) = (env.clone(), bump.clone(), entry.id);
	Some(Rc::new(move || {
		{
			let mut opened = env.opened.borrow_mut();
			if !opened.remove(&id) {
				opened.insert(id);
			}
		}
		bump.update(|count| *count += 1);
	}))
}

pub fn history(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>, size: f32) -> Vec<View> {
	if !history_shown(entry, env) {
		return Vec::new();
	}
	entry
		.history
		.iter()
		.enumerate()
		.map(|(index, document)| {
			let look = Look {
				size,
				color: "text_5",
				jumbo: false,
				trailer: Some(EDITED),
				on_trailer: None,
			};
			let base = HISTORY_SPOILERS * (index + 1);
			body::render(document, env, &entry.names, entry.id, base, bump, look)
				.id(format!("kept-version-{}-{index}", entry.id))
		})
		.collect()
}

pub fn accessories(entry: &MessageEntry, env: &Rc<RowEnv>) -> Vec<View> {
	let Some(look) = entry.look.as_ref() else {
		return Vec::new();
	};
	look.accessories
		.iter()
		.map(|accessory| {
			let line = text(accessory.text.clone())
				.text_size(tok("font_size_xs"))
				.line_height(16.)
				.text_color(tok("text_5"));
			let (Some(key), Some(menu)) = (accessory.action.clone(), env.actions.plugin_menu()) else {
				return line;
			};
			let click = AccessoryClick {
				plugin: accessory.plugin.clone(),
				key,
				channel_id: entry.message.channel_id,
				message_id: entry.id,
			};
			let pressing = menu.clone();
			line.id(format!("accessory-{}-{}", accessory.plugin, entry.id))
				.cursor(Cursor::Pointer)
				.hover(|style| style.text_color(tok("text_3")))
				.on_event(move |event| {
					if event.phase != zgx::input::EventPhase::Capture
						&& matches!(event.event, zgx::input::InputEvent::PointerDown { .. })
					{
						pressing.press();
					}
				})
				.on_click(move || menu.access(click.clone()))
		})
		.collect()
}
