use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use twemoji::Piece;
use zgx::compose::prelude::*;
use zgx::svg::SvgData;

thread_local! {
	static SVGS: RefCell<HashMap<&'static str, Option<Arc<SvgData>>>> = RefCell::default();
}

pub fn art(file: &'static str) -> Option<Arc<SvgData>> {
	SVGS.with(|svgs| {
		svgs.borrow_mut()
			.entry(file)
			.or_insert_with(|| {
				twemoji::svg(file)
					.and_then(|bytes| SvgData::new(bytes).ok())
					.map(Arc::new)
			})
			.clone()
	})
}

pub fn whole(text: &str) -> Option<&'static str> {
	let mut pieces = twemoji::split(text);
	match (pieces.next(), pieces.next()) {
		(Some(Piece::Emoji { text: matched, file }), None) if matched == text => Some(file),
		_ => None,
	}
}

pub fn svg_of(text: &str) -> Option<Arc<SvgData>> {
	whole(text).and_then(art)
}

pub fn luau_code(text: &str) -> String {
	svg_of(text)
		.and_then(|art| std::str::from_utf8(art.bytes()).ok().map(str::to_owned))
		.unwrap_or_default()
}

pub fn glyph(text: impl Into<String>, side: f32) -> View {
	let text = text.into();
	match svg_of(&text) {
		Some(art) => svg(text, art).size(side, side).shrink_0(),
		None => zgx::compose::text(text)
			.shrink_0()
			.text_size(side - 2.)
			.line_height(side),
	}
}
