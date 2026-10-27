use bridge::MessageEntry;
use zgx::compose::prelude::*;

use super::icons::plugin_icon;
use super::rows::tok;

const DECORATION: f32 = 16.;

pub fn author_decorations(entry: &MessageEntry) -> Option<View> {
	let marks: Vec<View> = entry
		.decorations
		.iter()
		.filter(|badge| badge.icon.is_some())
		.map(|badge| {
			let mark = div()
				.id(format!("decoration-{}-{}-{}", badge.plugin, badge.key, entry.id))
				.child(plugin_icon(badge.icon.as_ref(), DECORATION, &badge.color));
			if badge.tooltip.is_empty() {
				mark
			} else {
				mark.tooltip(badge.tooltip.clone())
			}
		})
		.collect();
	(!marks.is_empty()).then(|| {
		row()
			.items_center()
			.gap(tok("space_2"))
			.shrink_0()
			.children(marks)
	})
}
