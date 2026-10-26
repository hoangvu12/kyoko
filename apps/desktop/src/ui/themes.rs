use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use bridge::ToastKind;
use bridge::themes::{ThemeEntry, Themes, link_urls, set_theme_links, toggle_theme};
use session::{Command, ThemeLinkCopy};
use state::Changes;
use store::{Settings, Store};
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, text};
use crate::theme::{self, Catalog};
use crate::toasts;

const FOLDER_CHECK: Duration = Duration::from_secs(1);

pub struct ThemesView {
	themes: RefCell<Themes>,
	store: Arc<Store>,
	list: Shared,
	links: Shared,
}

impl ThemesView {
	pub fn new(host: &ScriptHost, store: Arc<Store>, themes: Themes) -> Self {
		let folder = themes.local_dir().display().to_string();
		let view = Self {
			list: host.shared("themes.list", Value::List(Rc::from([]))),
			links: host.shared("themes.links", ""),
			themes: RefCell::new(themes),
			store,
		};
		host.shared("themes.folder", folder.as_str());
		view
	}

	pub fn startup(store: &Store, bundled: &[std::path::PathBuf]) -> (Themes, Catalog) {
		let settings = store.settings.read().clone();
		let mut themes = Themes::load(bundled, store.paths.themes_dir());
		let copies =
			session::cached_copies(&store.paths.theme_links_dir(), &link_urls(&settings.themes.links));
		themes.set_online(&settings.themes.links, &copies);
		let catalog = Catalog::new(themes.palettes(&settings.themes));
		(themes, catalog)
	}

	fn settings(&self) -> Settings {
		self.store.settings.read().clone()
	}

	fn save(&self, change: impl FnOnce(&mut Settings) -> bool) -> bool {
		let mut changed = false;
		if let Err(error) = self.store.update_settings(|settings| changed = change(settings)) {
			tracing::warn!(%error, "the theme settings could not be saved");
		}
		changed
	}
}

impl Link {
	pub(super) fn install_themes(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("themes.apply", move |argument| {
			if let (Some(link), Some(id)) = (actions.upgrade(), text(&argument)) {
				link.set_setting("theme", &serde_json::Value::from(id));
				link.publish_themes();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("themes.toggle", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let Some(id) = field(&argument, "id").and_then(text) else {
				return;
			};
			let on = matches!(field(&argument, "on"), Some(Value::Bool(true)));
			if on && !link.themes.themes.borrow().has_local(id) {
				return;
			}
			if link
				.themes
				.save(|settings| toggle_theme(&mut settings.themes, id, on))
			{
				link.apply_theme_layers();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("themes.set_links", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let typed = text(&argument).unwrap_or_default();
			link.themes
				.save(|settings| set_theme_links(&mut settings.themes, typed));
			link.refresh_theme_links();
		});
		let actions = Rc::downgrade(link);
		host.action("themes.refresh", move |_| {
			if let Some(link) = actions.upgrade() {
				link.themes.themes.borrow_mut().rescan();
				link.refresh_theme_links();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("themes.open_folder", move |_| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let dir = link.themes.themes.borrow().local_dir().to_path_buf();
			let opened = std::fs::create_dir_all(&dir)
				.map_err(anyhow::Error::from)
				.and_then(|()| platform::open::reveal_path(&dir).map_err(anyhow::Error::from));
			if let Err(error) = opened {
				tracing::warn!(%error, "could not open the themes folder");
				toasts::show(ToastKind::Error, "The themes folder could not be opened.");
			}
		});
	}

	pub(super) fn publish_themes(&self) {
		let settings = self.themes.settings();
		let applied = if settings.theme == theme::SYSTEM {
			theme::current(&self.window.runtime)
		} else {
			settings.theme.clone()
		};
		let entries = self.themes.themes.borrow().entries(&settings.themes, &applied);
		let list: Vec<serde_json::Value> = entries.iter().map(entry_json).collect();
		if let Some(value) = Value::from_json(&serde_json::Value::Array(list).to_string()) {
			self.themes.list.set(value);
		}
		self.themes
			.links
			.set(settings.themes.links.join("\n").as_str().into());
	}

	pub(super) fn enable_capture_themes(&self, files: &[std::path::PathBuf]) {
		if files.is_empty() {
			return;
		}
		let dir = self.themes.themes.borrow().local_dir().to_path_buf();
		let mut ids = Vec::new();
		for file in files {
			let Some(id) = file.file_name().and_then(|name| name.to_str()) else {
				continue;
			};
			let copied = std::fs::create_dir_all(&dir).and_then(|()| std::fs::copy(file, dir.join(id)));
			match copied {
				Ok(_) => ids.push(id.to_owned()),
				Err(error) => tracing::warn!(%error, theme = id, "could not copy a capture theme"),
			}
		}
		self.themes.themes.borrow_mut().rescan();
		self.themes.save(|settings| {
			for id in &ids {
				toggle_theme(&mut settings.themes, id, true);
			}
			true
		});
		self.apply_theme_layers();
	}

	fn apply_theme_layers(&self) {
		let settings = self.themes.settings();
		let palettes = self.themes.themes.borrow().palettes(&settings.themes);
		self.settings_view()
			.catalog()
			.replace(&self.window.runtime, palettes);
		self.publish_themes();
	}

	fn refresh_theme_links(&self) {
		let settings = self.themes.settings();
		let copies = self.theme_link_copies(&settings);
		self.themes
			.themes
			.borrow_mut()
			.set_online(&settings.themes.links, &copies);
		self.apply_theme_layers();
		self.send(Command::RefreshThemeLinks {
			urls: link_urls(&settings.themes.links),
		});
	}

	fn theme_link_copies(&self, settings: &Settings) -> Vec<ThemeLinkCopy> {
		let urls = link_urls(&settings.themes.links);
		let fetched = self
			.flow
			.borrow()
			.session()
			.map(|session| session.theme_links())
			.unwrap_or_default();
		let cached = session::cached_copies(&self.themes.store.paths.theme_links_dir(), &urls);
		urls.into_iter()
			.filter_map(|url| {
				fetched
					.iter()
					.chain(cached.iter())
					.find(|copy| copy.url == url)
					.cloned()
			})
			.collect()
	}

	pub(super) fn themes_apply(&self, changes: &Changes) {
		if !changes.theme_links {
			return;
		}
		let settings = self.themes.settings();
		let copies = self.theme_link_copies(&settings);
		self.themes
			.themes
			.borrow_mut()
			.set_online(&settings.themes.links, &copies);
		self.apply_theme_layers();
	}

	pub(super) fn themes_session_started(&self) {
		let links = self.themes.settings().themes.links;
		if !links.is_empty() {
			self.send(Command::RefreshThemeLinks {
				urls: link_urls(&links),
			});
		}
	}

	pub(super) async fn follow_theme_folder(self: Rc<Self>) {
		self.publish_themes();
		loop {
			zgx::timer::sleep(FOLDER_CHECK).await;
			let changed = self.themes.themes.borrow_mut().rescan();
			if changed {
				tracing::info!("the themes folder changed, reloading themes");
				self.apply_theme_layers();
			}
		}
	}
}

fn entry_json(entry: &ThemeEntry) -> serde_json::Value {
	serde_json::json!({
		"id": entry.id,
		"source": entry.source.name(),
		"name": entry.meta.name,
		"author": entry.meta.author,
		"description": entry.meta.description,
		"version": entry.meta.version,
		"enabled": entry.enabled,
		"error": entry.error,
		"note": entry.note,
		"scheme": entry.scheme.name(),
		"swatch": entry.swatch,
	})
}
