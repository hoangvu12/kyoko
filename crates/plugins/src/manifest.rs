use serde_json::Value as Json;
use toml::Value as Toml;

use crate::permission::Permission;

pub const API_VERSION: i64 = 1;
pub const MANIFEST_FILE: &str = "plugin.toml";
pub const LOGIC_FILE: &str = "logic.luau";
pub const UI_FILE: &str = "ui.luau";

const ID_LIMIT: usize = 48;
const SHORT_TEXT: usize = 64;
const LONG_TEXT: usize = 400;
const STRING_LIMIT: usize = 2000;
const KEY_LIMIT: usize = 48;
const SETTING_LIMIT: usize = 64;
const PERMISSION_LIMIT: usize = 32;
const OPTION_LIMIT: usize = 32;
const FIELD_LIMIT: usize = 8;
const ROWS_DEFAULT: usize = 25;
const ROW_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
	pub id: String,
	pub name: String,
	pub version: String,
	pub api: i64,
	pub description: String,
	pub authors: Vec<String>,
	pub permissions: Vec<Permission>,
	pub settings: Vec<SettingSpec>,
	pub logic: Option<String>,
	pub ui: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingSpec {
	pub key: String,
	pub label: String,
	pub description: String,
	pub kind: SettingKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingKind {
	Boolean {
		default: bool,
	},
	Number {
		default: f64,
		min: f64,
		max: f64,
		step: f64,
	},
	Text {
		default: String,
		multiline: bool,
	},
	Select {
		default: String,
		options: Vec<Choice>,
	},
	Action,
	List {
		fields: Vec<SettingSpec>,
		item: String,
		max: usize,
		default: Vec<Json>,
	},
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
	pub value: String,
	pub label: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ManifestError {
	Syntax(String),
	MissingId,
	BadId(String),
}

impl std::fmt::Display for ManifestError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ManifestError::Syntax(error) => write!(formatter, "plugin.toml does not parse: {error}"),
			ManifestError::MissingId => formatter.write_str("plugin.toml has no id"),
			ManifestError::BadId(id) => write!(
				formatter,
				"the plugin id {id:?} must be lowercase letters, digits and dashes, starting with a letter"
			),
		}
	}
}

impl Manifest {
	pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
		let root: toml::Table = text
			.parse()
			.map_err(|error: toml::de::Error| ManifestError::Syntax(error.message().to_owned()))?;
		let id = root
			.get("id")
			.and_then(Toml::as_str)
			.ok_or(ManifestError::MissingId)?
			.to_owned();
		if !is_id(&id) {
			return Err(ManifestError::BadId(id));
		}
		let name = text_field(&root, "name", SHORT_TEXT).unwrap_or_else(|| id.clone());
		let mut permissions: Vec<Permission> = list(&root, "permissions")
			.filter_map(Toml::as_str)
			.take(PERMISSION_LIMIT)
			.map(Permission::parse)
			.collect();
		permissions.sort();
		permissions.dedup();
		let mut settings: Vec<SettingSpec> = Vec::new();
		for entry in list(&root, "settings").take(SETTING_LIMIT) {
			match SettingSpec::parse(entry) {
				Some(spec) if settings.iter().all(|known| known.key != spec.key) => settings.push(spec),
				_ => tracing::warn!(plugin = %id, "a plugin setting was left out"),
			}
		}
		Ok(Manifest {
			name,
			version: text_field(&root, "version", SHORT_TEXT).unwrap_or_else(|| "0.0.0".to_owned()),
			api: root.get("api").and_then(Toml::as_integer).unwrap_or(API_VERSION),
			description: text_field(&root, "description", LONG_TEXT).unwrap_or_default(),
			authors: list(&root, "authors")
				.filter_map(Toml::as_str)
				.map(|author| clip(author, SHORT_TEXT))
				.take(8)
				.collect(),
			permissions,
			settings,
			logic: file_field(&root, "logic"),
			ui: file_field(&root, "ui"),
			id,
		})
	}

	pub fn compatible(&self) -> bool {
		self.api == API_VERSION
	}

	pub fn setting(&self, key: &str) -> Option<&SettingSpec> {
		self.settings.iter().find(|spec| spec.key == key)
	}
}

impl SettingSpec {
	fn parse(entry: &Toml) -> Option<SettingSpec> {
		let table = entry.as_table()?;
		let key = table.get("key").and_then(Toml::as_str)?;
		if !is_key(key) {
			return None;
		}
		let label = text_field(table, "label", SHORT_TEXT).unwrap_or_else(|| key.to_owned());
		let description = text_field(table, "description", LONG_TEXT).unwrap_or_default();
		let default = table.get("default");
		let kind = match table.get("type").and_then(Toml::as_str)? {
			"boolean" => SettingKind::Boolean {
				default: default.and_then(Toml::as_bool).unwrap_or(false),
			},
			"number" => {
				let min = number(table.get("min")).unwrap_or(0.);
				let max = number(table.get("max")).unwrap_or(100.).max(min);
				let step = number(table.get("step")).filter(|step| *step > 0.).unwrap_or(1.);
				SettingKind::Number {
					default: number(default).unwrap_or(min).clamp(min, max),
					min,
					max,
					step,
				}
			}
			"string" => SettingKind::Text {
				default: default
					.and_then(Toml::as_str)
					.map(|text| clip(text, STRING_LIMIT))
					.unwrap_or_default(),
				multiline: table.get("multiline").and_then(Toml::as_bool).unwrap_or(false),
			},
			"select" => {
				let options: Vec<Choice> = table
					.get("options")
					.and_then(Toml::as_array)
					.into_iter()
					.flatten()
					.filter_map(Choice::parse)
					.take(OPTION_LIMIT)
					.collect();
				let first = options.first()?.value.clone();
				let default = default
					.and_then(Toml::as_str)
					.filter(|value| options.iter().any(|option| option.value == *value))
					.map_or(first, str::to_owned);
				SettingKind::Select { default, options }
			}
			"action" => SettingKind::Action,
			"list" => {
				let mut fields: Vec<SettingSpec> = Vec::new();
				for entry in table
					.get("fields")
					.and_then(Toml::as_array)
					.into_iter()
					.flatten()
					.take(FIELD_LIMIT)
				{
					match SettingSpec::parse(entry) {
						Some(field)
							if field.fits_a_row() && fields.iter().all(|known| known.key != field.key) =>
						{
							fields.push(field)
						}
						_ => tracing::warn!(setting = %key, "a list field was left out"),
					}
				}
				if fields.is_empty() {
					return None;
				}
				let max = table
					.get("max")
					.and_then(Toml::as_integer)
					.filter(|max| *max > 0)
					.map_or(ROWS_DEFAULT, |max| {
						usize::try_from(max).unwrap_or(ROW_LIMIT).min(ROW_LIMIT)
					});
				let default = default
					.and_then(|value| serde_json::to_value(value).ok())
					.and_then(|value| rows(&fields, max, &value))
					.unwrap_or_default();
				SettingKind::List {
					item: text_field(table, "item", SHORT_TEXT).unwrap_or_else(|| "Item".to_owned()),
					fields,
					max,
					default,
				}
			}
			_ => return None,
		};
		Some(SettingSpec {
			key: key.to_owned(),
			label,
			description,
			kind,
		})
	}

	pub fn default_value(&self) -> Json {
		match &self.kind {
			SettingKind::Boolean { default } => Json::Bool(*default),
			SettingKind::Number { default, .. } => number_json(*default),
			SettingKind::Text { default, .. } => Json::String(default.clone()),
			SettingKind::Select { default, .. } => Json::String(default.clone()),
			SettingKind::Action => Json::Null,
			SettingKind::List { default, .. } => Json::Array(default.clone()),
		}
	}

	pub fn accept(&self, value: &Json) -> Option<Json> {
		match &self.kind {
			SettingKind::Boolean { .. } => value.as_bool().map(Json::Bool),
			SettingKind::Number { min, max, step, .. } => {
				let raw = value.as_f64().filter(|raw| raw.is_finite())?;
				let stepped = min + ((raw - min) / step).round() * step;
				Some(number_json(stepped.clamp(*min, *max)))
			}
			SettingKind::Text { multiline, .. } => {
				let text = value.as_str()?;
				let text = if *multiline {
					text.to_owned()
				} else {
					text.replace(['\r', '\n'], " ")
				};
				Some(Json::String(clip(&text, STRING_LIMIT)))
			}
			SettingKind::Select { options, .. } => value
				.as_str()
				.filter(|value| options.iter().any(|option| option.value == *value))
				.map(|value| Json::String(value.to_owned())),
			SettingKind::Action => None,
			SettingKind::List { fields, max, .. } => rows(fields, *max, value).map(Json::Array),
		}
	}

	pub fn type_name(&self) -> &'static str {
		match self.kind {
			SettingKind::Boolean { .. } => "boolean",
			SettingKind::Number { .. } => "number",
			SettingKind::Text { .. } => "string",
			SettingKind::Select { .. } => "select",
			SettingKind::Action => "action",
			SettingKind::List { .. } => "list",
		}
	}

	fn fits_a_row(&self) -> bool {
		!matches!(self.kind, SettingKind::Action | SettingKind::List { .. })
	}
}

fn rows(fields: &[SettingSpec], max: usize, value: &Json) -> Option<Vec<Json>> {
	let items = match value {
		Json::Array(items) => items.as_slice(),
		Json::Object(map) if map.is_empty() => &[],
		_ => return None,
	};
	Some(
		items
			.iter()
			.filter_map(Json::as_object)
			.take(max)
			.map(|row| {
				let cells = fields.iter().map(|field| {
					let cell = row
						.get(&field.key)
						.and_then(|value| field.accept(value))
						.unwrap_or_else(|| field.default_value());
					(field.key.clone(), cell)
				});
				Json::Object(cells.collect())
			})
			.collect(),
	)
}

impl Choice {
	fn parse(entry: &Toml) -> Option<Choice> {
		match entry {
			Toml::String(value) => Some(Choice {
				value: clip(value, SHORT_TEXT),
				label: clip(value, SHORT_TEXT),
			}),
			Toml::Table(table) => {
				let value = clip(table.get("value").and_then(Toml::as_str)?, SHORT_TEXT);
				let label = text_field(table, "label", SHORT_TEXT).unwrap_or_else(|| value.clone());
				Some(Choice { value, label })
			}
			_ => None,
		}
	}
}

pub fn is_id(id: &str) -> bool {
	id.len() <= ID_LIMIT
		&& id.starts_with(|first: char| first.is_ascii_lowercase())
		&& id
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_key(key: &str) -> bool {
	!key.is_empty()
		&& key.len() <= KEY_LIMIT
		&& key
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_file(name: &str) -> bool {
	!name.is_empty()
		&& name.ends_with(".luau")
		&& name
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
		&& !name.contains("..")
}

fn file_field(table: &toml::Table, key: &str) -> Option<String> {
	table
		.get(key)
		.and_then(Toml::as_str)
		.filter(|name| is_file(name))
		.map(str::to_owned)
}

fn text_field(table: &toml::Table, key: &str, limit: usize) -> Option<String> {
	table
		.get(key)
		.and_then(Toml::as_str)
		.map(str::trim)
		.filter(|text| !text.is_empty())
		.map(|text| clip(text, limit))
}

fn list<'a>(table: &'a toml::Table, key: &str) -> impl Iterator<Item = &'a Toml> {
	table.get(key).and_then(Toml::as_array).into_iter().flatten()
}

fn number(value: Option<&Toml>) -> Option<f64> {
	match value? {
		Toml::Integer(integer) => Some(*integer as f64),
		Toml::Float(float) if float.is_finite() => Some(*float),
		_ => None,
	}
}

fn number_json(value: f64) -> Json {
	serde_json::Number::from_f64(value).map_or(Json::Null, Json::Number)
}

pub(crate) fn clip(text: &str, limit: usize) -> String {
	match text.char_indices().nth(limit) {
		Some((end, _)) => text[..end].to_owned(),
		None => text.to_owned(),
	}
}
