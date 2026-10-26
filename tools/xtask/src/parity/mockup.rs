use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::screens::ScreenEntry;

pub const MOCKUP_DIR: &str = "docs/design/mockups";

pub const MOCKUPS: &[(&str, &str)] = &[
	("app-frame", "midnight-discord"),
	("login-tos", "login"),
	("login-token", "login-token"),
	("login-qr-scanned", "login-qr-scanned"),
	("login-qr-expired", "login-qr-expired"),
	("login-web", "login-web"),
	("server-rail", "midnight-discord"),
	("server-rail-folder", "midnight-discord"),
	("channel-sidebar", "midnight-discord"),
	("dm-sidebar", "dm"),
	("dm-group", "group-dm"),
	("timeline", "midnight-discord"),
	("composer", "composer-markdown"),
	("composer-reply", "composer-markdown"),
	("message-hover", "midnight-discord"),
	("message-context-menu", "message-menu"),
	("member-list", "midnight-discord"),
	("user-panel", "midnight-discord"),
	("status-menu", "status-menu"),
	("custom-status", "dialog-custom-status"),
	("profile-popout", "profile-popout"),
	("full-profile", "profile-full"),
	("quick-switcher", "quick-switcher"),
	("search", "search-results"),
	("lightbox", "lightbox"),
	("emoji-picker", "emoji-picker"),
	("gif-picker", "gif-picker"),
	("sticker-picker", "sticker-picker"),
	("threads-list", "threads-list"),
	("thread-panel", "threads-split-view"),
	("thread-create", "threads-create"),
	("pins", "pins"),
	("forum", "forum"),
	("forum-post", "forum-post"),
	("forum-post-create", "forum-new-post"),
	("friends-online", "home-friends"),
	("friends-all", "friends-all"),
	("friends-pending", "friends-pending"),
	("friends-blocked", "friends-blocked"),
	("friends-add", "friends-add-friend"),
	("add-server", "dialog-add-a-server"),
	("forward", "dialog-forward"),
	("guild-notifications", "dialog-notification-settings"),
	("server-settings-overview", "server-settings-overview"),
	("server-settings-roles", "server-settings-roles"),
	("server-settings-emoji", "server-settings-emoji"),
	("server-settings-stickers", "server-settings-stickers"),
	("server-settings-members", "server-settings-members"),
	("server-settings-invites", "server-settings-invites"),
	("server-settings-bans", "server-settings-bans"),
	("server-settings-audit-log", "server-settings-audit-log"),
	("moderation-nickname", "dialog-change-nickname"),
	("moderation-timeout", "dialog-timeout"),
	("moderation-kick", "dialog-kick"),
	("moderation-ban", "dialog-ban"),
	("settings-my-account", "settings-my-account"),
	("settings-appearance", "settings-appearance"),
	("settings-notifications", "settings-notifications"),
	("settings-plugins", "settings-plugins"),
	("toast", "toast-rate-limited"),
	("connection-offline", "connection-lost"),
];

pub struct Pairing {
	pub screen: ScreenEntry,
	pub page: PathBuf,
}

pub fn page_for(screen: &str) -> Option<&'static str> {
	MOCKUPS
		.iter()
		.find(|(name, _)| *name == screen)
		.map(|(_, page)| *page)
}

pub fn select(root: &Path, listed: &[ScreenEntry], wanted: &[String]) -> Result<Vec<Pairing>> {
	let names: Vec<&str> = if wanted.is_empty() {
		MOCKUPS.iter().map(|(name, _)| *name).collect()
	} else {
		wanted.iter().map(String::as_str).collect()
	};
	names
		.into_iter()
		.map(|name| {
			let Some(page) = page_for(name) else {
				bail!("{name} has no mockup yet; add it to the mockup table in docs/parity.md and tools/xtask/src/parity/mockup.rs");
			};
			let screen = listed
				.iter()
				.find(|screen| screen.name == name)
				.with_context(|| format!("{name} is not in the parity list"))?;
			let page = root.join(MOCKUP_DIR).join(format!("{page}.html"));
			if !page.is_file() {
				bail!("the mockup {} for {name} does not exist", page.display());
			}
			Ok(Pairing {
				screen: ScreenEntry {
					steps: format!("Mockup {}.html. {}", page_stem(&page), screen.steps),
					..screen.clone()
				},
				page,
			})
		})
		.collect()
}

fn page_stem(page: &Path) -> String {
	page.file_stem()
		.map(|stem| stem.to_string_lossy().into_owned())
		.unwrap_or_default()
}
