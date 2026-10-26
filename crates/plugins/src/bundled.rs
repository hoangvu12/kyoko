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

const ONE_PING_PER_DM: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/one-ping-per-dm/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/one-ping-per-dm/logic.luau"),
	),
];

const SHOW_ME_YOUR_NAME: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/show-me-your-name/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/show-me-your-name/logic.luau"),
	),
];

const DONT_ROUND_MY_TIMESTAMPS: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/dont-round-my-timestamps/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/dont-round-my-timestamps/logic.luau"),
	),
];

const FAVORITE_EMOJI_FIRST: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/favorite-emoji-first/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/favorite-emoji-first/logic.luau"),
	),
];

const EXPRESSION_CLONER: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/expression-cloner/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/expression-cloner/logic.luau"),
	),
	(
		"copy_plus.svg",
		include_str!("../../../plugins/expression-cloner/copy_plus.svg"),
	),
];

const MESSAGE_LOGGER: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/message-logger/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/message-logger/logic.luau"),
	),
];

const SHOW_HIDDEN_CHANNELS: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/show-hidden-channels/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/show-hidden-channels/logic.luau"),
	),
];

const SHOW_HIDDEN_THINGS: Embedded = &[
	(
		"plugin.toml",
		include_str!("../../../plugins/show-hidden-things/plugin.toml"),
	),
	(
		"logic.luau",
		include_str!("../../../plugins/show-hidden-things/logic.luau"),
	),
];

const PACKAGES: [Embedded; 11] = [
	CLEAR_URLS,
	SILENT_TYPING,
	TEXT_REPLACE,
	ONE_PING_PER_DM,
	SHOW_ME_YOUR_NAME,
	DONT_ROUND_MY_TIMESTAMPS,
	FAVORITE_EMOJI_FIRST,
	EXPRESSION_CLONER,
	MESSAGE_LOGGER,
	SHOW_HIDDEN_CHANNELS,
	SHOW_HIDDEN_THINGS,
];

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
