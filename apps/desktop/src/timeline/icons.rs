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
	"friend_requests",
	"phone_off",
	"circle_x",
	"circle_check",
	"info",
	"warning",
);

thread_local! {
	static LOADED: RefCell<HashMap<&'static str, Option<Arc<SvgData>>>> = RefCell::default();
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
