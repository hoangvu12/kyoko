use std::sync::Arc;

use super::{Core, Internal};
use crate::theme_links::fetch_all;

impl Core {
	pub(super) fn refresh_theme_links(&mut self, urls: Vec<String>) {
		let links = Arc::clone(&self.themes);
		let internal = self.internal.clone();
		let tuning = self.theme_tuning.clone();
		let cache_dir = self.store.paths.theme_links_dir();
		let generation = links.begin();
		self.runtime.spawn(async move {
			let copies = fetch_all(&tuning, &cache_dir, &urls).await;
			if links.finish(generation, copies) {
				let _ = internal.send(Internal::ThemeLinksChanged);
			}
		});
	}
}
