use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use zgx::reactive::Runtime;
use zgx::theme::{Theme, ThemeKind, Themes, TokenId, TokenInput, font_family, parse_color};

use crate::fonts;

pub const DEFAULT: &str = "dark";
pub const SYSTEM: &str = "system";
pub const NAMESPACE: &str = "kyoko";
const FILE: &str = "theme.json";

const UI_FONTS: &[&str] = &[
	"gg sans",
	"Noto Sans",
	"Segoe UI",
	"Helvetica Neue",
	"Arial",
	"sans-serif",
];
const CODE_FONTS: &[&str] = &["Consolas", "Menlo", "monospace"];

pub const COLORS: &[(&str, &str)] = &[
	("background_primary", "#313338"),
	("background_secondary", "#2b2d31"),
	("background_tertiary", "#1e1f22"),
	("background_floating", "#111214"),
	("user_panel_background", "#232428"),
	("modifier_hover", "rgba(79, 84, 92, 0.16)"),
	("modifier_selected", "rgba(79, 84, 92, 0.24)"),
	("modifier_active", "rgba(79, 84, 92, 0.48)"),
	("message_hover", "rgba(0, 0, 0, 0.06)"),
	("message_selected", "rgba(88, 101, 242, 0.14)"),
	("brand_tint", "rgba(88, 101, 242, 0.16)"),
	("brand_tint_strong", "rgba(88, 101, 242, 0.2)"),
	("tag_badge", "rgba(181, 186, 193, 0.18)"),
	("tag_badge_moderated", "rgba(88, 101, 242, 0.18)"),
	("reaction_me", "rgba(88, 101, 242, 0.15)"),
	("poll_fill", "rgba(78, 80, 88, 0.45)"),
	("poll_fill_chosen", "rgba(88, 101, 242, 0.45)"),
	("text_normal", "#dbdee1"),
	("text_muted", "#949ba4"),
	("header_primary", "#f2f3f5"),
	("header_secondary", "#b5bac1"),
	("interactive_normal", "#b5bac1"),
	("interactive_hover", "#dbdee1"),
	("interactive_active", "#ffffff"),
	("interactive_muted", "#4e5058"),
	("brand", "#5865f2"),
	("brand_hover", "#4752c4"),
	("text_on_brand", "#ffffff"),
	("focus_ring", "rgba(88, 101, 242, 0.5)"),
	("brand_wash", "rgba(88, 101, 242, 0.16)"),
	("status_online", "#23a559"),
	("status_idle", "#f0b232"),
	("status_dnd", "#f23f43"),
	("status_offline", "#80848e"),
	("status_streaming", "#593695"),
	("mention_background", "rgba(88, 101, 242, 0.3)"),
	("mention_text", "#c9cdfb"),
	("link", "#00a8fc"),
	("danger", "#f23f43"),
	("danger_hover", "#da373c"),
	("success", "#23a559"),
	("warning", "#f0b232"),
	("spoiler", "#202225"),
	("code_background", "#2b2d31"),
	("code_border", "#1e1f22"),
	("scrollbar_thumb", "#1a1b1e"),
	("scrollbar_track", "#2b2d31"),
	("divider", "#3f4147"),
	("input_background", "#383a40"),
	("button_secondary", "#4e5058"),
	("button_secondary_hover", "#6d6f78"),
	("unread_pill", "#f2f3f5"),
	("mention_badge", "#f23f43"),
	("overlay", "rgba(0, 0, 0, 0.7)"),
	("speaking_ring", "#23a559"),
	("live_badge", "#f23f43"),
	("brand_selected", "rgba(88, 101, 242, 0.16)"),
	("gif_shade", "rgba(30, 31, 34, 0.55)"),
	("banner_button", "rgba(78, 80, 88, 0.3)"),
];

pub const SIZES: &[(&str, f32)] = &[
	("title_bar_height", 22.),
	("server_rail_width", 72.),
	("server_icon", 48.),
	("channel_sidebar_width", 240.),
	("member_list_width", 240.),
	("top_bar_height", 48.),
	("user_panel_height", 52.),
	("guild_header_height", 48.),
	("channel_row_height", 32.),
	("voice_row_height", 28.),
	("voice_row_avatar", 24.),
	("voice_badge_icon", 14.),
	("member_row_height", 42.),
	("message_avatar", 40.),
	("message_gutter", 16.),
	("compact_row", 22.),
	("font_tiny", 12.),
	("font_xs", 13.),
	("font_sm", 14.),
	("font_md", 16.),
	("font_lg", 20.),
	("font_xl", 24.),
	("radius_sm", 4.),
	("radius_md", 8.),
	("radius_lg", 16.),
	("radius_xl", 24.),
	("space_2", 2.),
	("space_4", 4.),
	("space_8", 8.),
	("space_12", 12.),
	("space_16", 16.),
	("space_20", 20.),
	("space_24", 24.),
	("space_32", 32.),
	("login_card_width", 784.),
	("login_qr_column", 240.),
	("login_qr_size", 184.),
	("login_logo", 56.),
	("login_logo_icon", 30.),
	("icon_sm", 16.),
	("icon_md", 20.),
	("icon_lg", 24.),
	("avatar_xs", 24.),
	("avatar_sm", 32.),
	("avatar_md", 40.),
	("avatar_lg", 80.),
	("avatar_xl", 120.),
	("friends_row_height", 62.),
	("friends_active_width", 360.),
	("profile_popover_width", 300.),
	("profile_modal_width", 600.),
	("profile_banner_height", 60.),
	("profile_banner_height_lg", 120.),
	("modal_width", 440.),
	("status_popover_width", 260.),
	("badge_icon", 22.),
	("chip_height", 24.),
	("field_height", 36.),
	("settings_card_width", 400.),
	("list_max_height", 280.),
	("glyph_sm", 16.),
	("glyph_md", 20.),
	("glyph_lg", 24.),
	("portrait_sm", 24.),
	("portrait_md", 32.),
	("portrait_lg", 48.),
	("control_height", 32.),
	("list_row_height", 44.),
	("thread_panel_width", 420.),
	("popover_width", 400.),
	("popover_height", 440.),
	("picker_width", 456.),
	("picker_height", 440.),
	("picker_cell", 44.),
	("picker_sticker", 104.),
	("picker_rail", 48.),
	("gif_gap", 4.),
	("forum_card_width", 288.),
	("forum_thumb_height", 160.),
	("forum_thumb_width", 96.),
	("settings_nav_width", 232.),
	("settings_content_width", 760.),
	("settings_swatch", 28.),
	("modal_width_sm", 440.),
	("modal_width_md", 540.),
	("modal_width_lg", 700.),
	("invite_card_width", 432.),
	("invite_card_icon", 56.),
	("role_dot", 12.),
	("permission_row_height", 48.),
	("manage_grid_cell", 108.),
	("toast_width", 360.),
];

const LIGHT: &[(&str, &str)] = &[
	("background_primary", "#ffffff"),
	("background_secondary", "#f2f3f5"),
	("background_tertiary", "#e3e5e8"),
	("background_floating", "#ffffff"),
	("user_panel_background", "#ebedef"),
	("modifier_hover", "rgba(116, 127, 141, 0.08)"),
	("modifier_selected", "rgba(116, 127, 141, 0.16)"),
	("modifier_active", "rgba(116, 127, 141, 0.24)"),
	("text_normal", "#313338"),
	("text_muted", "#5c5e66"),
	("header_primary", "#060607"),
	("header_secondary", "#4e5058"),
	("interactive_normal", "#4e5058"),
	("interactive_hover", "#313338"),
	("interactive_active", "#060607"),
	("interactive_muted", "#a4a6ab"),
	("mention_background", "rgba(88, 101, 242, 0.15)"),
	("mention_text", "#3c45a5"),
	("link", "#006ce7"),
	("spoiler", "#d4d6d9"),
	("code_background", "#f2f3f5"),
	("code_border", "#e3e5e8"),
	("scrollbar_thumb", "#c1c3c7"),
	("scrollbar_track", "#f2f3f5"),
	("divider", "#e1e2e4"),
	("input_background", "#ebedef"),
	("button_secondary", "#e3e5e8"),
	("button_secondary_hover", "#d7d9dc"),
	("unread_pill", "#060607"),
	("overlay", "rgba(0, 0, 0, 0.5)"),
	("gif_shade", "rgba(227, 229, 232, 0.55)"),
];

const AMOLED: &[(&str, &str)] = &[
	("background_primary", "#000000"),
	("background_secondary", "#0a0a0b"),
	("background_tertiary", "#000000"),
	("background_floating", "#0a0a0b"),
	("user_panel_background", "#050506"),
	("code_background", "#0a0a0b"),
	("code_border", "#1e1f22"),
	("scrollbar_track", "#000000"),
	("divider", "#1e1f22"),
	("input_background", "#141416"),
	("overlay", "rgba(0, 0, 0, 0.85)"),
	("gif_shade", "rgba(0, 0, 0, 0.55)"),
];

const STANDARD: &[(&str, &str)] = &[
	("background", "background_primary"),
	("surface.sunken", "background_tertiary"),
	("surface.base", "background_secondary"),
	("surface.raised", "background_primary"),
	("surface.overlay", "background_floating"),
	("text.primary", "text_normal"),
	("text.secondary", "text_muted"),
	("text.tertiary", "interactive_muted"),
	("border", "divider"),
	("border.subtle", "background_tertiary"),
	("accent", "brand"),
	("accent.hover", "brand_hover"),
	("accent.text", "text_on_brand"),
	("danger", "danger"),
	("warning", "warning"),
	("success", "success"),
	("info", "link"),
	("hover", "modifier_hover"),
	("pressed", "modifier_active"),
	("selection.item", "modifier_selected"),
	("focus.ring", "brand"),
	("scrollbar.thumb", "scrollbar_thumb"),
	("scrollbar.thumb.active", "scrollbar_thumb"),
	("scrollbar.track", "scrollbar_track"),
	("font.size.md", "font_md"),
	("radius.sm", "radius_sm"),
	("radius.md", "radius_md"),
	("radius.lg", "radius_lg"),
	("radius.xl", "radius_xl"),
	("control.height", "control_height"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
	pub name: String,
	colors: Vec<(&'static str, String)>,
	sizes: Vec<(&'static str, f32)>,
	ui_fonts: Vec<String>,
	code_fonts: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PaletteError {
	#[error("could not read the theme file: {0}")]
	Io(#[from] std::io::Error),
	#[error("the theme file is not valid json: {0}")]
	Json(#[from] serde_json::Error),
	#[error("the theme file must be a json object")]
	NotAnObject,
	#[error("unknown base theme {0:?}")]
	UnknownBase(String),
}

impl Palette {
	pub fn dark() -> Self {
		Self {
			name: DEFAULT.to_owned(),
			colors: COLORS
				.iter()
				.map(|(key, value)| (*key, (*value).to_owned()))
				.collect(),
			sizes: SIZES.to_vec(),
			ui_fonts: families(UI_FONTS),
			code_fonts: families(CODE_FONTS),
		}
	}

	pub fn builtin(name: &str) -> Option<Self> {
		match name {
			"dark" => Some(Self::dark()),
			"light" => Some(Self::dark().with_overrides("light", LIGHT)),
			"amoled" => Some(Self::dark().with_overrides("amoled", AMOLED)),
			_ => None,
		}
	}

	pub fn builtins() -> impl Iterator<Item = Self> {
		["dark", "light", "amoled"].into_iter().filter_map(Self::builtin)
	}

	pub fn from_file(name: &str, path: &Path) -> Result<Self, PaletteError> {
		Self::from_json(name, &std::fs::read_to_string(path)?)
	}

	pub fn from_json(name: &str, text: &str) -> Result<Self, PaletteError> {
		let Value::Object(mut tokens) = serde_json::from_str(text)? else {
			return Err(PaletteError::NotAnObject);
		};
		let mut palette = match tokens.remove("base") {
			Some(Value::String(base)) => Self::builtin(&base).ok_or(PaletteError::UnknownBase(base))?,
			_ => Self::dark(),
		};
		palette.name = name.to_owned();
		palette.apply(tokens);
		Ok(palette)
	}

	pub fn color(&self, key: &str) -> Option<&str> {
		self.colors
			.iter()
			.find(|(name, _)| *name == key)
			.map(|(_, value)| value.as_str())
	}

	pub fn is_light(&self) -> bool {
		self.color("background_primary")
			.and_then(parse_color)
			.is_some_and(|color| {
				let channels = [color.0, color.1, color.2];
				let high = channels.iter().max().copied().unwrap_or(0) as f32;
				let low = channels.iter().min().copied().unwrap_or(0) as f32;
				(high + low) / 2. / 255. > 0.5
			})
	}

	pub fn to_theme(&self, installed: &[String]) -> Theme {
		let kind = if self.is_light() {
			ThemeKind::Light
		} else {
			ThemeKind::Dark
		};
		let mut theme = Theme::new(&self.name, kind);
		for (key, value) in &self.colors {
			if let Ok(input) = TokenInput::parse(value) {
				theme = theme.with(own(key), input);
			}
		}
		for (key, value) in &self.sizes {
			theme = theme.with(own(key), *value);
		}
		for (standard, source) in STANDARD {
			theme = theme.with(
				zgx::theme::token(standard),
				TokenInput::parse(&format!("{{{}}}", token(source))).expect("a token reference"),
			);
		}
		let ui_font = fonts::pick(&self.ui_fonts, installed);
		let code_font = fonts::pick(&self.code_fonts, installed);
		theme
			.with(own("ui_font"), font_family(&ui_font))
			.with(own("code_font"), font_family(&code_font))
			.with(zgx::theme::token("font.sans"), font_family(&ui_font))
			.with(zgx::theme::token("font.mono"), font_family(&code_font))
	}

	fn with_overrides(mut self, name: &str, overrides: &[(&str, &str)]) -> Self {
		self.name = name.to_owned();
		for (key, value) in overrides {
			self.set_color(key, value);
		}
		self
	}

	fn apply(&mut self, tokens: Map<String, Value>) {
		for (key, value) in tokens {
			if !self.apply_token(&key, &value) {
				tracing::warn!(token = %key, theme = %self.name, "ignoring an unknown or malformed theme token");
			}
		}
	}

	fn apply_token(&mut self, key: &str, value: &Value) -> bool {
		match value {
			Value::String(text) => parse_color(text).is_some() && self.set_color(key, text),
			Value::Number(number) => number
				.as_f64()
				.filter(|size| size.is_finite() && *size >= 0.)
				.is_some_and(|size| self.set_size(key, size as f32)),
			Value::Array(items) => {
				let names: Vec<String> = items
					.iter()
					.filter_map(Value::as_str)
					.map(str::trim)
					.filter(|name| !name.is_empty())
					.map(str::to_owned)
					.collect();
				let slot = match key {
					"ui_fonts" => &mut self.ui_fonts,
					"code_fonts" => &mut self.code_fonts,
					_ => return false,
				};
				if names.is_empty() {
					return false;
				}
				*slot = names;
				true
			}
			_ => false,
		}
	}

	fn set_color(&mut self, key: &str, value: &str) -> bool {
		match self.colors.iter_mut().find(|(name, _)| *name == key) {
			Some((_, slot)) => {
				*slot = value.to_owned();
				true
			}
			None => false,
		}
	}

	fn set_size(&mut self, key: &str, value: f32) -> bool {
		match self.sizes.iter_mut().find(|(name, _)| *name == key) {
			Some((_, slot)) => {
				*slot = value;
				true
			}
			None => false,
		}
	}
}

pub struct Catalog {
	pub palettes: Vec<Palette>,
}

impl Catalog {
	pub fn load(dirs: &[PathBuf]) -> Self {
		let mut palettes: Vec<Palette> = Palette::builtins().collect();
		for dir in dirs {
			for (name, path) in theme_files(dir) {
				if palettes.iter().any(|palette| palette.name == name) {
					continue;
				}
				match Palette::from_file(&name, &path) {
					Ok(palette) => palettes.push(palette),
					Err(error) => tracing::warn!(%error, theme = %name, "could not load a theme"),
				}
			}
		}
		Self { palettes }
	}

	pub fn install(&self, runtime: &Runtime, chosen: &str) -> String {
		let installed = fonts::installed();
		let themes = Themes::of(runtime);
		for (key, value) in COLORS {
			if let Ok(input) = TokenInput::parse(value) {
				themes.declare(own(key), input);
			}
		}
		for (key, value) in SIZES {
			themes.declare(own(key), *value);
		}
		for palette in &self.palettes {
			themes.register(palette.to_theme(&installed));
		}
		if chosen == SYSTEM {
			themes.follow_system(Some(("dark", "light")));
			return themes.current().to_string();
		}
		let name = self.resolve(chosen);
		if let Err(error) = themes.set_default(&name) {
			tracing::warn!(%error, theme = %name, "could not apply the theme");
		}
		name
	}

	pub fn apply(&self, runtime: &Runtime, chosen: &str) -> String {
		let themes = Themes::of(runtime);
		if chosen == SYSTEM {
			themes.follow_system(Some(("dark", "light")));
			return themes.current().to_string();
		}
		let name = self.resolve(chosen);
		if let Err(error) = themes.set(&name) {
			tracing::warn!(%error, theme = %name, "could not apply the theme");
		}
		name
	}

	pub fn code_font(&self, theme: &str) -> String {
		let palette = self
			.palettes
			.iter()
			.find(|palette| palette.name == theme)
			.or_else(|| self.palettes.first());
		match palette {
			Some(palette) => fonts::pick(&palette.code_fonts, &fonts::installed()),
			None => "monospace".to_owned(),
		}
	}

	fn resolve(&self, chosen: &str) -> String {
		if self.palettes.iter().any(|palette| palette.name == chosen) {
			return chosen.to_owned();
		}
		tracing::warn!(theme = %chosen, "unknown theme, using dark");
		DEFAULT.to_owned()
	}
}

pub fn token(key: &str) -> String {
	format!("{NAMESPACE}.{key}")
}

fn theme_files(dir: &Path) -> Vec<(String, PathBuf)> {
	let Ok(entries) = std::fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut files: Vec<(String, PathBuf)> = entries
		.flatten()
		.filter_map(|entry| {
			let path = entry.path();
			let stem = path.file_stem()?.to_str()?.to_owned();
			if path.is_dir() {
				let file = path.join(FILE);
				return file.is_file().then_some((stem, file));
			}
			(path.extension()? == "json").then_some((stem, path))
		})
		.collect();
	files.sort();
	files
}

fn families(names: &[&str]) -> Vec<String> {
	names.iter().map(|name| (*name).to_owned()).collect()
}

fn own(key: &str) -> TokenId {
	zgx::theme::token(&token(key))
}
