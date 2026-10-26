use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use zgx::compose::prelude::*;

use super::rows::tok;

macro_rules! icon_files {
	($($name:literal),* $(,)?) => {
		fn source(name: &str) -> Option<&'static [u8]> {
			match name {
				$($name => Some(include_bytes!(concat!("../../ui/icons/", $name, ".svg"))),)*
				_ => None,
			}
		}
	};
}

icon_files!(
	"hash",
	"join",
	"pin",
	"thread",
	"nitro",
	"close",
	"phone",
	"edit",
	"image",
	"chat",
	"play",
	"file",
	"download",
	"check",
	"reply",
	"copy",
	"link",
	"forward",
	"unpin",
	"new_thread",
	"inbox",
	"funnel",
	"trash",
	"flag",
	"emoji",
	"more",
	"clock",
	"lock",
	"profile",
	"account",
	"log_out",
	"megaphone",
	"volume",
	"stage",
	"forum",
	"at_sign",
	"group",
	"home",
	"gif",
	"sticker",
	"external_link",
	"eye",
	"warning_fill",
	"archive",
	"unarchive",
	"members",
	"attach",
	"plus",
	"chevron_down",
	"list_layout",
	"grid_layout",
	"upload",
	"shield",
	"timer",
	"user_x",
	"ban",
	"mic",
	"mic_off",
	"headphones",
	"headphones_off",
	"chevron_right",
	"chevron_left",
	"user_cog",
	"friend_requests",
	"phone_off",
	"circle_x",
	"circle_check",
	"info",
	"warning",
	"star",
	"star_filled",
	"mark_read",
	"picker_trophy",
	"picker_star",
	"picker_star_outline",
	"picker_clock",
	"picker_people",
	"picker_nature",
	"picker_food",
	"picker_activities",
	"picker_travel",
	"picker_objects",
	"picker_symbols",
	"picker_flags",
	"picker_trending",
	"picker_back",
	"picker_lock",
	"picker_chevron",
	"picker_search",
	"sparkle",
	"add_reaction",
	"add_super_reaction",
	"user_plus",
	"smile",
	"refresh",
	"play_fill",
	"pause_fill",
	"volume_fill",
	"volume_mute_fill",
	"file_audio",
	"chevron_up",
	"spinner",
	"poll",
	"apps",
);

const PLUGIN_ICONS: usize = 64;

thread_local! {
	static LOADED: RefCell<HashMap<&'static str, Option<Arc<SvgData>>>> = RefCell::default();
	static PLUGIN_LOADED: RefCell<HashMap<Arc<str>, Option<Arc<SvgData>>>> = RefCell::default();
}

fn plugin_shape(source: &Arc<str>) -> Option<Arc<SvgData>> {
	PLUGIN_LOADED.with(|loaded| {
		let mut loaded = loaded.borrow_mut();
		if loaded.len() >= PLUGIN_ICONS && !loaded.contains_key(source) {
			loaded.clear();
		}
		loaded
			.entry(source.clone())
			.or_insert_with(|| {
				SvgData::new(source.as_bytes())
					.ok()
					.map(|svg| Arc::new(svg.current_color()))
			})
			.clone()
	})
}

pub fn plugin_icon(source: Option<&Arc<str>>, size: f32, color: &str) -> View {
	let shape = source.and_then(plugin_shape);
	div()
		.size(size, size)
		.shrink_0()
		.text_color(tok(color))
		.when_some(shape, |slot, shape| {
			slot.child(svg("plugin", shape).size(size, size))
		})
}

fn load(name: &'static str) -> Option<Arc<SvgData>> {
	LOADED.with(|loaded| {
		loaded
			.borrow_mut()
			.entry(name)
			.or_insert_with(|| {
				source(name)
					.and_then(|bytes| SvgData::new(bytes).ok())
					.map(|svg| Arc::new(svg.current_color()))
			})
			.clone()
	})
}

pub fn icon(name: &'static str, size: f32, color: &str) -> View {
	glyph(name, size).text_color(tok(color))
}

pub fn glyph(name: &'static str, size: f32) -> View {
	let shape = load(name);
	div()
		.size(size, size)
		.shrink_0()
		.when_some(shape, |slot, shape| slot.child(svg(name, shape).size(size, size)))
}
