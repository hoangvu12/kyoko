use crate::package::{Embedded, Package};

const CLEAR_URLS: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/clear-urls/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/clear-urls/logic.luau"),
	),
	(
		"rules.luau",
		include_str!("../../../plugins/clear-urls/rules.luau"),
	),
];

const SILENT_TYPING: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/silent-typing/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/silent-typing/logic.luau"),
	),
	("ui.luau", include_str!("../../../plugins/silent-typing/ui.luau")),
	(
		"keyboard.svg",
		include_str!("../../../plugins/silent-typing/keyboard.svg"),
	),
	(
		"keyboard_off.svg",
		include_str!("../../../plugins/silent-typing/keyboard_off.svg"),
	),
];

const TEXT_REPLACE: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/text-replace/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/text-replace/logic.luau"),
	),
];

const PACKAGES: [Embedded; 3] = [CLEAR_URLS, SILENT_TYPING, TEXT_REPLACE];

pub fn packages() -> Vec<Package> {
	PACKAGES
		.iter()
		.filter_map(|files| match Package::embedded(files) {
			Ok(package) => Some(package),
			Err(error) => {
				tracing::error!(%error, "a bundled plugin is broken");
				None
			}
		})
		.collect()
}
