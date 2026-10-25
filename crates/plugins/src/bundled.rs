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

const PACKAGES: [Embedded; 1] = [CLEAR_URLS];

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
