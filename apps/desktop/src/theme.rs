use bridge::themes::{COLORS, Palette, SIZES};
use zgx::reactive::Runtime;
use zgx::theme::{Theme, ThemeKind, Themes as ZgxThemes, TokenId, TokenInput, font_family};

use crate::fonts;

pub const DEFAULT: &str = "midnight";
pub const SYSTEM: &str = "system";
pub const NAMESPACE: &str = "kyoko";
const CLEAR: &str = "#00000000";

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

pub fn to_theme(palette: &Palette, installed: &[String]) -> Theme {
	let kind = if palette.is_light() {
		ThemeKind::Light
	} else {
		ThemeKind::Dark
	};
	let mut theme = Theme::new(&palette.name, kind);
	for (key, value) in palette.colors() {
		if let Ok(input) = TokenInput::parse(value) {
			theme = theme.with(own(key), input);
		}
	}
	for (key, value) in palette.sizes() {
		theme = theme.with(own(key), *value);
	}
	for (standard, source) in STANDARD {
		theme = theme.with(
			zgx::theme::token(standard),
			TokenInput::parse(&format!("{{{}}}", token(source))).expect("a token reference"),
		);
	}
	theme = theme.with(
		zgx::theme::token("backdrop"),
		TokenInput::parse(CLEAR).expect("a colour"),
	);
	let ui_font = fonts::pick(palette.ui_fonts(), installed);
	let code_font = fonts::pick(palette.code_fonts(), installed);
	theme
		.with(own("ui_font"), font_family(&ui_font))
		.with(own("code_font"), font_family(&code_font))
		.with(zgx::theme::token("font.sans"), font_family(&ui_font))
		.with(zgx::theme::token("font.mono"), font_family(&code_font))
}

pub struct Catalog {
	pub palettes: Vec<Palette>,
	installed: Vec<String>,
}

impl Catalog {
	pub fn new(palettes: Vec<Palette>) -> Self {
		Self {
			palettes,
			installed: fonts::installed(),
		}
	}

	#[cfg(test)]
	pub fn load(bundled: &[std::path::PathBuf]) -> Self {
		let themes = bridge::themes::Themes::load(bundled, std::path::PathBuf::new());
		Self::new(themes.palettes(&store::ThemeSettings::default()))
	}

	pub fn install(&self, runtime: &Runtime, chosen: &str) -> String {
		let themes = ZgxThemes::of(runtime);
		for (key, value) in COLORS {
			if let Ok(input) = TokenInput::parse(value) {
				themes.declare(own(key), input);
			}
		}
		for (key, value) in SIZES {
			themes.declare(own(key), *value);
		}
		for palette in &self.palettes {
			themes.register(to_theme(palette, &self.installed));
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

	pub fn replace(&mut self, runtime: &Runtime, palettes: Vec<Palette>) {
		let themes = ZgxThemes::of(runtime);
		runtime.batch(|| {
			for palette in palettes.iter().filter(|palette| !self.palettes.contains(palette)) {
				themes.register(to_theme(palette, &self.installed));
			}
		});
		self.palettes = palettes;
	}

	pub fn apply(&self, runtime: &Runtime, chosen: &str) -> String {
		let themes = ZgxThemes::of(runtime);
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
			Some(palette) => fonts::pick(palette.code_fonts(), &self.installed),
			None => "monospace".to_owned(),
		}
	}

	fn resolve(&self, chosen: &str) -> String {
		if self.palettes.iter().any(|palette| palette.name == chosen) {
			return chosen.to_owned();
		}
		tracing::warn!(theme = %chosen, "unknown theme, using the default");
		DEFAULT.to_owned()
	}
}

pub fn current(runtime: &Runtime) -> String {
	ZgxThemes::of(runtime).current().to_string()
}

pub fn token(key: &str) -> String {
	format!("{NAMESPACE}.{key}")
}

fn own(key: &str) -> TokenId {
	zgx::theme::token(&token(key))
}
