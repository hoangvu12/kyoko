mod color;
mod css;
mod links;
mod palette;
mod tokens;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use session::{MAX_THEME_BYTES, ThemeLinkCopy};
use store::ThemeSettings;

pub use color::{Rgba, parse_color};
pub use css::{CSS_MAPPING, ThemeMeta, import_css, parse_meta};
pub use links::{ThemeLink, link_lines, link_urls, theme_link, theme_links};
pub use palette::{
	BUILTIN_THEMES, DEFAULT_THEME, Layer, Palette, PaletteError, Scheme, color_token, size_token,
};
pub use tokens::{AMOLED, CODE_FONTS, COLORS, LIGHT, SIZES, UI_FONTS};

const THEME_FILE: &str = "theme.json";
const ABOUT_FILE: &str = "theme.toml";
const BUILTIN_AUTHOR: &str = "Kyoko";
const SWATCH: [&str; 6] = [
	"background_tertiary",
	"background_secondary",
	"background_primary",
	"brand",
	"text_normal",
	"link",
];
const BUILTIN_ABOUT: [(&str, &str, &str); 3] = [
	("dark", "Dark", "Discord's dark colours."),
	("light", "Light", "Discord's light colours."),
	("amoled", "AMOLED", "Pure black surfaces for OLED screens."),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
	Builtin,
	Local,
	Online,
}

impl ThemeSource {
	pub fn name(self) -> &'static str {
		match self {
			ThemeSource::Builtin => "builtin",
			ThemeSource::Local => "local",
			ThemeSource::Online => "online",
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEntry {
	pub id: String,
	pub source: ThemeSource,
	pub meta: ThemeMeta,
	pub enabled: bool,
	pub error: String,
	pub note: String,
	pub scheme: Scheme,
	pub swatch: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
	Css,
	Json,
}

#[derive(Debug, Clone)]
struct Base {
	palette: Palette,
	meta: ThemeMeta,
}

#[derive(Debug, Clone)]
struct Loaded {
	id: String,
	meta: ThemeMeta,
	layer: Option<Layer>,
	error: String,
	note: String,
	scheme: Scheme,
}

type Stamp = (String, u64, Option<SystemTime>);

pub struct Themes {
	bases: Vec<Base>,
	local_dir: PathBuf,
	local: Vec<Loaded>,
	stamps: Vec<Stamp>,
	online: Vec<Loaded>,
}

impl Themes {
	pub fn load(bundled: &[PathBuf], local_dir: PathBuf) -> Self {
		let mut themes = Self {
			bases: bases(bundled),
			local_dir,
			local: Vec::new(),
			stamps: Vec::new(),
			online: Vec::new(),
		};
		themes.reload();
		themes
	}

	pub fn local_dir(&self) -> &Path {
		&self.local_dir
	}

	pub fn rescan(&mut self) -> bool {
		if stamps(&self.local_dir) == self.stamps {
			return false;
		}
		self.reload();
		true
	}

	fn reload(&mut self) {
		self.stamps = stamps(&self.local_dir);
		self.local = local_themes(&self.local_dir);
	}

	pub fn set_online(&mut self, lines: &[String], copies: &[ThemeLinkCopy]) {
		self.online = theme_links(lines)
			.into_iter()
			.map(|link| {
				let copy = copies.iter().find(|copy| copy.url == link.url);
				online_theme(&link, copy)
			})
			.collect();
	}

	pub fn base_names(&self) -> Vec<String> {
		self.bases.iter().map(|base| base.palette.name.clone()).collect()
	}

	pub fn base(&self, name: &str) -> Option<&Palette> {
		self.bases
			.iter()
			.find(|base| base.palette.name == name)
			.map(|base| &base.palette)
	}

	pub fn palettes(&self, settings: &ThemeSettings) -> Vec<Palette> {
		let active = self.active(settings);
		self.bases
			.iter()
			.map(|base| {
				let light = base.palette.is_light();
				base.palette.layered(
					active
						.iter()
						.filter(|theme| theme.scheme.admits(light))
						.filter_map(|theme| theme.layer.as_ref()),
				)
			})
			.collect()
	}

	pub fn entries(&self, settings: &ThemeSettings, applied: &str) -> Vec<ThemeEntry> {
		let current = self
			.base(applied)
			.or_else(|| self.base(DEFAULT_THEME))
			.cloned()
			.unwrap_or_else(Palette::dark);
		let mut entries: Vec<ThemeEntry> = self
			.bases
			.iter()
			.map(|base| ThemeEntry {
				id: base.palette.name.clone(),
				source: ThemeSource::Builtin,
				meta: base.meta.clone(),
				enabled: base.palette.name == applied,
				error: String::new(),
				note: String::new(),
				scheme: Scheme::Any,
				swatch: swatch(&base.palette),
			})
			.collect();
		let layered = |theme: &Loaded, source: ThemeSource, enabled: bool| ThemeEntry {
			id: theme.id.clone(),
			source,
			meta: theme.meta.clone(),
			enabled,
			error: theme.error.clone(),
			note: theme.note.clone(),
			scheme: theme.scheme,
			swatch: swatch(&current.layered(theme.layer.iter())),
		};
		entries.extend(self.local.iter().map(|theme| {
			let enabled = theme.layer.is_some() && settings.enabled.contains(&theme.id);
			layered(theme, ThemeSource::Local, enabled)
		}));
		entries.extend(
			self.online
				.iter()
				.map(|theme| layered(theme, ThemeSource::Online, theme.layer.is_some())),
		);
		entries
	}

	pub fn has_local(&self, id: &str) -> bool {
		self.local.iter().any(|theme| theme.id == id)
	}

	fn active(&self, settings: &ThemeSettings) -> Vec<&Loaded> {
		let local = settings
			.enabled
			.iter()
			.filter_map(|id| self.local.iter().find(|theme| &theme.id == id));
		local
			.chain(self.online.iter())
			.filter(|theme| theme.layer.is_some())
			.collect()
	}
}

pub fn toggle_theme(settings: &mut ThemeSettings, id: &str, on: bool) -> bool {
	let present = settings.enabled.iter().any(|enabled| enabled == id);
	match (on, present) {
		(true, false) => settings.enabled.push(id.to_owned()),
		(false, true) => settings.enabled.retain(|enabled| enabled != id),
		_ => return false,
	}
	true
}

pub fn set_theme_links(settings: &mut ThemeSettings, text: &str) -> bool {
	let lines = link_lines(text);
	if lines == settings.links {
		return false;
	}
	settings.links = lines;
	true
}

fn swatch(palette: &Palette) -> Vec<String> {
	SWATCH
		.iter()
		.map(|key| {
			palette
				.color(key)
				.and_then(parse_color)
				.map(|color| color.hex())
				.unwrap_or_default()
		})
		.collect()
}

fn builtin_meta(name: &str) -> ThemeMeta {
	let (title, description) = BUILTIN_ABOUT
		.iter()
		.find(|(id, _, _)| *id == name)
		.map_or((name, ""), |(_, title, description)| (*title, *description));
	ThemeMeta {
		name: title.to_owned(),
		author: BUILTIN_AUTHOR.to_owned(),
		description: description.to_owned(),
		version: env!("CARGO_PKG_VERSION").to_owned(),
		..ThemeMeta::default()
	}
}

fn bases(bundled: &[PathBuf]) -> Vec<Base> {
	let mut bases: Vec<Base> = Palette::builtins()
		.map(|palette| Base {
			meta: builtin_meta(&palette.name),
			palette,
		})
		.collect();
	for dir in bundled {
		for (name, path) in bundled_files(dir) {
			if bases.iter().any(|base| base.palette.name == name) {
				continue;
			}
			let text = match read_limited(&path) {
				Ok(text) => text,
				Err(error) => {
					tracing::warn!(%error, theme = %name, "could not read a bundled theme");
					continue;
				}
			};
			match Palette::from_json(&name, &text) {
				Ok(palette) => {
					let meta = path
						.parent()
						.and_then(|folder| about(&folder.join(ABOUT_FILE)))
						.unwrap_or_else(|| json_meta(&text, &name));
					bases.push(Base { palette, meta });
				}
				Err(error) => tracing::warn!(%error, theme = %name, "could not load a bundled theme"),
			}
		}
	}
	bases
}

fn bundled_files(dir: &Path) -> Vec<(String, PathBuf)> {
	let Ok(entries) = std::fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut files: Vec<(String, PathBuf)> = entries
		.flatten()
		.filter_map(|entry| {
			let path = entry.path();
			let stem = path.file_stem()?.to_str()?.to_owned();
			if path.is_dir() {
				let file = path.join(THEME_FILE);
				return file.is_file().then_some((stem, file));
			}
			(path.extension()? == "json").then_some((stem, path))
		})
		.collect();
	files.sort();
	files
}

fn about(path: &Path) -> Option<ThemeMeta> {
	let table: toml::Table = std::fs::read_to_string(path).ok()?.parse().ok()?;
	let field = |key: &str| -> String {
		table
			.get(key)
			.and_then(toml::Value::as_str)
			.map(clean_field)
			.unwrap_or_default()
	};
	let name = field("name");
	(!name.is_empty()).then(|| ThemeMeta {
		name,
		author: field("author"),
		description: field("description"),
		version: field("version"),
		source: field("source"),
		website: field("website"),
	})
}

fn json_meta(text: &str, fallback: &str) -> ThemeMeta {
	let object = serde_json::from_str::<Value>(text).ok();
	let field = |key: &str| -> String {
		object
			.as_ref()
			.and_then(|object| object.get(key))
			.and_then(Value::as_str)
			.map(clean_field)
			.unwrap_or_default()
	};
	let name = field("name");
	ThemeMeta {
		name: if name.is_empty() {
			clean_field(fallback)
		} else {
			name
		},
		author: field("author"),
		description: field("description"),
		version: field("version"),
		source: field("source"),
		website: field("website"),
	}
}

fn clean_field(value: &str) -> String {
	parse_meta("", value).name
}

fn read_limited(path: &Path) -> Result<String, String> {
	let length = std::fs::metadata(path)
		.map_err(|_| "the file could not be read".to_owned())?
		.len();
	if length as usize > MAX_THEME_BYTES {
		return Err(format!("the file is larger than {} KB", MAX_THEME_BYTES / 1024));
	}
	let bytes = std::fs::read(path).map_err(|_| "the file could not be read".to_owned())?;
	String::from_utf8(bytes).map_err(|_| "the file is not text".to_owned())
}

fn local_files(dir: &Path) -> Vec<(String, PathBuf, Format)> {
	let Ok(entries) = std::fs::read_dir(dir) else {
		return Vec::new();
	};
	let mut files: Vec<(String, PathBuf, Format)> = entries
		.flatten()
		.filter_map(|entry| {
			let path = entry.path();
			let name = path.file_name()?.to_str()?.to_owned();
			if name.starts_with('.') {
				return None;
			}
			if path.is_dir() {
				let file = path.join(THEME_FILE);
				return file.is_file().then_some((name, file, Format::Json));
			}
			let format = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
				"css" => Format::Css,
				"json" => Format::Json,
				_ => return None,
			};
			Some((name, path, format))
		})
		.collect();
	files.sort_by_key(|file| file.0.to_lowercase());
	files
}

fn stamps(dir: &Path) -> Vec<Stamp> {
	local_files(dir)
		.into_iter()
		.map(|(name, path, _)| {
			let metadata = std::fs::metadata(&path).ok();
			(
				name,
				metadata.as_ref().map_or(0, std::fs::Metadata::len),
				metadata.and_then(|metadata| metadata.modified().ok()),
			)
		})
		.collect()
}

fn local_themes(dir: &Path) -> Vec<Loaded> {
	local_files(dir)
		.into_iter()
		.map(|(id, path, format)| {
			let fallback = Path::new(&id)
				.file_stem()
				.and_then(|stem| stem.to_str())
				.unwrap_or(&id)
				.to_owned();
			let about = (format == Format::Json)
				.then(|| path.parent().and_then(|folder| about(&folder.join(ABOUT_FILE))))
				.flatten()
				.filter(|_| path.ends_with(THEME_FILE));
			match read_limited(&path) {
				Ok(text) => parsed(id, &text, format, &fallback, about, Scheme::Any),
				Err(error) => Loaded {
					meta: parse_meta("", &fallback),
					id,
					layer: None,
					error,
					note: String::new(),
					scheme: Scheme::Any,
				},
			}
		})
		.collect()
}

fn parsed(
	id: String,
	text: &str,
	format: Format,
	fallback: &str,
	about: Option<ThemeMeta>,
	scheme: Scheme,
) -> Loaded {
	let (meta, layer) = match format {
		Format::Css => (parse_meta(text, fallback), Ok(import_css(text))),
		Format::Json => (
			about.unwrap_or_else(|| json_meta(text, fallback)),
			Layer::from_json(fallback, text).map_err(|error| error.to_string()),
		),
	};
	match layer {
		Ok(layer) => Loaded {
			note: if layer.is_empty() {
				"No colours to import.".to_owned()
			} else {
				String::new()
			},
			id,
			meta,
			layer: Some(layer),
			error: String::new(),
			scheme,
		},
		Err(error) => Loaded {
			id,
			meta,
			layer: None,
			error,
			note: String::new(),
			scheme,
		},
	}
}

fn link_name(url: &str) -> String {
	let path = url.split(['?', '#']).next().unwrap_or(url);
	let file = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);
	let stem = file
		.rsplit_once('.')
		.map_or(file, |(stem, _)| stem)
		.replace(['.', '-', '_'], " ");
	if stem.trim().is_empty() {
		url.to_owned()
	} else {
		stem.trim().to_owned()
	}
}

fn link_format(url: &str, text: &str) -> Format {
	let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
	if path.ends_with(".json") || serde_json::from_str::<Value>(text).is_ok_and(|value| value.is_object()) {
		Format::Json
	} else {
		Format::Css
	}
}

fn online_theme(link: &ThemeLink, copy: Option<&ThemeLinkCopy>) -> Loaded {
	let fallback = link_name(&link.url);
	let Some(copy) = copy else {
		return Loaded {
			id: link.line.clone(),
			meta: parse_meta("", &fallback),
			layer: None,
			error: String::new(),
			note: "Not downloaded yet.".to_owned(),
			scheme: link.scheme,
		};
	};
	let error = copy.error.clone().unwrap_or_default();
	let Some(text) = &copy.text else {
		return Loaded {
			id: link.line.clone(),
			meta: parse_meta("", &fallback),
			layer: None,
			error,
			note: String::new(),
			scheme: link.scheme,
		};
	};
	let mut loaded = parsed(
		link.line.clone(),
		text,
		link_format(&link.url, text),
		&fallback,
		None,
		link.scheme,
	);
	if !error.is_empty() {
		loaded.error = error;
		if copy.cached {
			loaded.note = "Using the copy saved last time.".to_owned();
		}
	}
	loaded
}
