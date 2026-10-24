use std::sync::Arc;

use super::{Core, Internal};
use crate::catalog::{fetch_theme_catalog, install_theme};
use crate::command::Command;

impl Core {
	pub(super) fn on_theme_command(&mut self, command: Command) {
		let themes = Arc::clone(&self.themes);
		let internal = self.internal.clone();
		match command {
			Command::ThemeCatalogRefresh { url } => {
				self.runtime.spawn(async move {
					themes.record_fetch(fetch_theme_catalog(&url).await);
					let _ = internal.send(Internal::ThemeCatalogChanged);
				});
			}
			Command::ThemeInstall { id } => {
				let themes_dir = self.store.paths.themes_dir();
				self.runtime.spawn(async move {
					let outcome = install_theme(themes.catalog(), themes_dir, &id).await;
					themes.record_error(outcome.err());
					let _ = internal.send(Internal::ThemeCatalogChanged);
				});
			}
			_ => {}
		}
	}
}
