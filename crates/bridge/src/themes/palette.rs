use serde_json::{Map, Value};

use super::color::parse_color;
use super::tokens::{AMOLED, CODE_FONTS, COLORS, LIGHT, SIZES, UI_FONTS};

pub const DEFAULT_THEME: &str = "dark";
pub const BUILTIN_THEMES: [&str; 3] = ["dark", "light", "amoled"];
const ABOUT_KEYS: [&str; 6] = ["name", "author", "description", "version", "source", "website"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
	#[default]
	Any,
	Light,
	Dark,
}

impl Scheme {
	pub fn admits(self, light: bool) -> bool {
		match self {
			Scheme::Any => true,
			Scheme::Light => light,
			Scheme::Dark => !light,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Scheme::Any => "",
			Scheme::Light => "light",
			Scheme::Dark => "dark",
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
	pub name: String,
	colors: Vec<(&'static str, String)>,
	sizes: Vec<(&'static str, f32)>,
	ui_fonts: Vec<String>,
	code_fonts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layer {
	colors: Vec<(&'static str, String, Scheme)>,
	sizes: Vec<(&'static str, f32)>,
	ui_fonts: Option<Vec<String>>,
	code_fonts: Option<Vec<String>>,
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

pub fn color_token(key: &str) -> Option<&'static str> {
	COLORS.iter().map(|(name, _)| *name).find(|name| *name == key)
}

pub fn size_token(key: &str) -> Option<&'static str> {
	SIZES.iter().map(|(name, _)| *name).find(|name| *name == key)
}

fn families(names: &[&str]) -> Vec<String> {
	names.iter().map(|name| (*name).to_owned()).collect()
}

fn json_object(text: &str) -> Result<Map<String, Value>, PaletteError> {
	let Value::Object(tokens) = serde_json::from_str(text)? else {
		return Err(PaletteError::NotAnObject);
	};
	Ok(tokens)
}

impl Palette {
	pub fn dark() -> Self {
		Self {
			name: DEFAULT_THEME.to_owned(),
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
		let overrides = match name {
			"dark" => return Some(Self::dark()),
			"light" => LIGHT,
			"amoled" => AMOLED,
			_ => return None,
		};
		let mut palette = Self::dark();
		palette.name = name.to_owned();
		for (key, value) in overrides {
			palette.set_color(key, value);
		}
		Some(palette)
	}

	pub fn builtins() -> impl Iterator<Item = Self> {
		BUILTIN_THEMES.into_iter().filter_map(Self::builtin)
	}

	pub fn from_json(name: &str, text: &str) -> Result<Self, PaletteError> {
		let mut tokens = json_object(text)?;
		let mut palette = match tokens.remove("base") {
			Some(Value::String(base)) => Self::builtin(&base).ok_or(PaletteError::UnknownBase(base))?,
			_ => Self::dark(),
		};
		palette.name = name.to_owned();
		let layer = Layer::from_tokens(name, tokens);
		palette.apply(&layer, false);
		Ok(palette)
	}

	pub fn color(&self, key: &str) -> Option<&str> {
		self.colors
			.iter()
			.find(|(name, _)| *name == key)
			.map(|(_, value)| value.as_str())
	}

	pub fn colors(&self) -> &[(&'static str, String)] {
		&self.colors
	}

	pub fn sizes(&self) -> &[(&'static str, f32)] {
		&self.sizes
	}

	pub fn ui_fonts(&self) -> &[String] {
		&self.ui_fonts
	}

	pub fn code_fonts(&self) -> &[String] {
		&self.code_fonts
	}

	pub fn is_light(&self) -> bool {
		self.color("background_primary")
			.and_then(parse_color)
			.is_some_and(|color| color.lightness() > 0.5)
	}

	pub fn apply(&mut self, layer: &Layer, light: bool) {
		for (key, value, scheme) in &layer.colors {
			if scheme.admits(light) {
				self.set_color(key, value);
			}
		}
		for (key, value) in &layer.sizes {
			if let Some((_, slot)) = self.sizes.iter_mut().find(|(name, _)| name == key) {
				*slot = *value;
			}
		}
		if let Some(fonts) = &layer.ui_fonts {
			self.ui_fonts.clone_from(fonts);
		}
		if let Some(fonts) = &layer.code_fonts {
			self.code_fonts.clone_from(fonts);
		}
	}

	pub fn layered<'a>(&self, layers: impl IntoIterator<Item = &'a Layer>) -> Self {
		let light = self.is_light();
		let mut palette = self.clone();
		for layer in layers {
			palette.apply(layer, light);
		}
		palette
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
}

impl Layer {
	pub fn from_json(name: &str, text: &str) -> Result<Self, PaletteError> {
		let mut tokens = json_object(text)?;
		tokens.remove("base");
		Ok(Self::from_tokens(name, tokens))
	}

	pub fn from_tokens(name: &str, tokens: Map<String, Value>) -> Self {
		let mut layer = Self::default();
		for (key, value) in tokens {
			if ABOUT_KEYS.contains(&key.as_str()) {
				continue;
			}
			if !layer.take_token(&key, &value) {
				tracing::warn!(token = %key, theme = %name, "ignoring an unknown or malformed theme token");
			}
		}
		layer
	}

	pub fn set_color(&mut self, key: &'static str, value: String, scheme: Scheme) {
		self.colors
			.retain(|(name, _, existing)| !(*name == key && *existing == scheme));
		self.colors.push((key, value, scheme));
	}

	pub fn colors(&self) -> &[(&'static str, String, Scheme)] {
		&self.colors
	}

	pub fn is_empty(&self) -> bool {
		self.colors.is_empty()
			&& self.sizes.is_empty()
			&& self.ui_fonts.is_none()
			&& self.code_fonts.is_none()
	}

	fn take_token(&mut self, key: &str, value: &Value) -> bool {
		match value {
			Value::String(text) => match (color_token(key), parse_color(text)) {
				(Some(token), Some(color)) => {
					self.set_color(token, color.hex(), Scheme::Any);
					true
				}
				_ => false,
			},
			Value::Number(number) => {
				let size = number.as_f64().filter(|size| size.is_finite() && *size >= 0.);
				match (size_token(key), size) {
					(Some(token), Some(size)) => {
						self.sizes.retain(|(name, _)| *name != token);
						self.sizes.push((token, size as f32));
						true
					}
					_ => false,
				}
			}
			Value::Array(items) => {
				let names: Vec<String> = items
					.iter()
					.filter_map(Value::as_str)
					.map(str::trim)
					.filter(|name| !name.is_empty())
					.map(str::to_owned)
					.collect();
				if names.is_empty() {
					return false;
				}
				match key {
					"ui_fonts" => self.ui_fonts = Some(names),
					"code_fonts" => self.code_fonts = Some(names),
					_ => return false,
				}
				true
			}
			_ => false,
		}
	}
}
