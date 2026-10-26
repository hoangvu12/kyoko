use base64::Engine;
use base64::alphabet::STANDARD as STANDARD_ALPHABET;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};

use crate::snowflake::Snowflake;

const PRELOADED_STATUS: u32 = 11;
const STATUS_VALUE: u32 = 1;
const STATUS_CUSTOM: u32 = 2;
const STATUS_EXPIRES_AT: u32 = 4;
const CUSTOM_TEXT: u32 = 1;
const CUSTOM_EMOJI_ID: u32 = 2;
const CUSTOM_EMOJI_NAME: u32 = 3;
const CUSTOM_EXPIRES_AT: u32 = 4;
const CUSTOM_CREATED_AT: u32 = 5;
const PRELOADED_GUILD_FOLDERS: u32 = 14;
const FOLDERS: u32 = 1;
const POSITIONS: u32 = 2;
const FOLDER_GUILD_IDS: u32 = 1;
const FOLDER_ID: u32 = 2;
const FOLDER_NAME: u32 = 3;
const FOLDER_COLOR: u32 = 4;
const WRAPPED_VALUE: u32 = 1;
const FRECENCY_FAVORITE_EMOJIS: u32 = 5;
const FAVORITE_EMOJIS: u32 = 1;
const FAVORITE_LIMIT: usize = 512;

const LENIENT: GeneralPurpose = GeneralPurpose::new(
	&STANDARD_ALPHABET,
	GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoGuildFolder {
	pub id: Option<i64>,
	pub name: Option<String>,
	pub color: Option<u64>,
	pub guild_ids: Vec<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoGuildFolders {
	pub folders: Vec<ProtoGuildFolder>,
	pub positions: Vec<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoCustomStatus {
	pub text: String,
	pub emoji_id: Option<Snowflake>,
	pub emoji_name: Option<String>,
	pub expires_at_ms: Option<u64>,
	pub created_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProtoStatusSettings {
	pub status: Option<String>,
	pub custom_status: Option<ProtoCustomStatus>,
	pub status_expires_at_ms: Option<u64>,
	pub other: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Wire<'a> {
	Varint(u64),
	Fixed64(u64),
	Bytes(&'a [u8]),
	Skipped,
}

struct Fields<'a> {
	bytes: &'a [u8],
}

impl<'a> Fields<'a> {
	fn new(bytes: &'a [u8]) -> Self {
		Self { bytes }
	}

	fn varint(&mut self) -> Option<u64> {
		let mut value = 0u64;
		for (index, byte) in self.bytes.iter().enumerate().take(10) {
			value |= u64::from(byte & 0x7f) << (7 * index);
			if byte & 0x80 == 0 {
				self.bytes = &self.bytes[index + 1..];
				return Some(value);
			}
		}
		None
	}

	fn take(&mut self, length: usize) -> Option<&'a [u8]> {
		let (head, tail) = self.bytes.split_at_checked(length)?;
		self.bytes = tail;
		Some(head)
	}
}

impl<'a> Iterator for Fields<'a> {
	type Item = (u32, Wire<'a>);

	fn next(&mut self) -> Option<Self::Item> {
		if self.bytes.is_empty() {
			return None;
		}
		let key = self.varint()?;
		let number = u32::try_from(key >> 3).ok()?;
		let wire = match key & 7 {
			0 => Wire::Varint(self.varint()?),
			1 => Wire::Fixed64(u64::from_le_bytes(self.take(8)?.try_into().ok()?)),
			2 => {
				let length = usize::try_from(self.varint()?).ok()?;
				Wire::Bytes(self.take(length)?)
			}
			5 => {
				self.take(4)?;
				Wire::Skipped
			}
			_ => return None,
		};
		Some((number, wire))
	}
}

fn wrapped(bytes: &[u8]) -> Option<Wire<'_>> {
	Fields::new(bytes)
		.find(|(number, _)| *number == WRAPPED_VALUE)
		.map(|(_, wire)| wire)
}

fn push_ids(ids: &mut Vec<Snowflake>, wire: Wire) {
	match wire {
		Wire::Fixed64(id) => ids.push(Snowflake(id)),
		Wire::Bytes(packed) => ids.extend(
			packed
				.as_chunks::<8>()
				.0
				.iter()
				.map(|chunk| Snowflake(u64::from_le_bytes(*chunk))),
		),
		Wire::Varint(_) | Wire::Skipped => {}
	}
}

fn decode_folder(bytes: &[u8]) -> ProtoGuildFolder {
	let mut folder = ProtoGuildFolder::default();
	for (number, wire) in Fields::new(bytes) {
		match (number, wire) {
			(FOLDER_GUILD_IDS, wire) => push_ids(&mut folder.guild_ids, wire),
			(FOLDER_ID, Wire::Bytes(inner)) => {
				if let Some(Wire::Varint(id)) = wrapped(inner) {
					folder.id = Some(id as i64);
				}
			}
			(FOLDER_NAME, Wire::Bytes(inner)) => {
				if let Some(Wire::Bytes(name)) = wrapped(inner) {
					folder.name = Some(String::from_utf8_lossy(name).into_owned());
				}
			}
			(FOLDER_COLOR, Wire::Bytes(inner)) => {
				if let Some(Wire::Varint(color)) = wrapped(inner) {
					folder.color = Some(color);
				}
			}
			_ => {}
		}
	}
	folder
}

fn decode_folders(bytes: &[u8]) -> ProtoGuildFolders {
	let mut decoded = ProtoGuildFolders::default();
	for (number, wire) in Fields::new(bytes) {
		match (number, wire) {
			(FOLDERS, Wire::Bytes(inner)) => decoded.folders.push(decode_folder(inner)),
			(POSITIONS, wire) => push_ids(&mut decoded.positions, wire),
			_ => {}
		}
	}
	decoded
}

pub fn decode_guild_folders(encoded: &str) -> Option<ProtoGuildFolders> {
	let bytes = LENIENT.decode(encoded.trim()).ok()?;
	Fields::new(&bytes).find_map(|(number, wire)| match (number, wire) {
		(PRELOADED_GUILD_FOLDERS, Wire::Bytes(inner)) => Some(decode_folders(inner)),
		_ => None,
	})
}

fn fixed(wire: Wire) -> Option<u64> {
	match wire {
		Wire::Fixed64(value) if value != 0 => Some(value),
		_ => None,
	}
}

fn text(bytes: &[u8]) -> String {
	String::from_utf8_lossy(bytes).into_owned()
}

fn decode_custom_status(bytes: &[u8]) -> ProtoCustomStatus {
	let mut custom = ProtoCustomStatus::default();
	for (number, wire) in Fields::new(bytes) {
		match (number, wire) {
			(CUSTOM_TEXT, Wire::Bytes(value)) => custom.text = text(value),
			(CUSTOM_EMOJI_ID, wire) => custom.emoji_id = fixed(wire).map(Snowflake),
			(CUSTOM_EMOJI_NAME, Wire::Bytes(value)) if !value.is_empty() => {
				custom.emoji_name = Some(text(value));
			}
			(CUSTOM_EXPIRES_AT, wire) => custom.expires_at_ms = fixed(wire),
			(CUSTOM_CREATED_AT, wire) => custom.created_at_ms = fixed(wire),
			_ => {}
		}
	}
	custom
}

fn decode_status(bytes: &[u8]) -> ProtoStatusSettings {
	let mut settings = ProtoStatusSettings::default();
	let mut fields = Fields::new(bytes);
	loop {
		let before = fields.bytes;
		let Some((number, wire)) = fields.next() else {
			break;
		};
		let raw = &before[..before.len() - fields.bytes.len()];
		match (number, wire) {
			(STATUS_VALUE, Wire::Bytes(inner)) => {
				if let Some(Wire::Bytes(value)) = wrapped(inner) {
					settings.status = Some(text(value)).filter(|status| !status.is_empty());
				}
			}
			(STATUS_CUSTOM, Wire::Bytes(inner)) => {
				settings.custom_status = Some(decode_custom_status(inner));
			}
			(STATUS_EXPIRES_AT, wire) => settings.status_expires_at_ms = fixed(wire),
			_ => settings.other.extend_from_slice(raw),
		}
	}
	settings
}

pub fn decode_status_settings(encoded: &str) -> Option<ProtoStatusSettings> {
	let bytes = LENIENT.decode(encoded.trim()).ok()?;
	Fields::new(&bytes).find_map(|(number, wire)| match (number, wire) {
		(PRELOADED_STATUS, Wire::Bytes(inner)) => Some(decode_status(inner)),
		_ => None,
	})
}

fn decode_favorites(bytes: &[u8]) -> Vec<String> {
	let mut favorites: Vec<String> = Vec::new();
	for (number, wire) in Fields::new(bytes) {
		if let (FAVORITE_EMOJIS, Wire::Bytes(value)) = (number, wire)
			&& let Ok(emoji) = std::str::from_utf8(value)
			&& !emoji.is_empty()
			&& !favorites.iter().any(|known| known == emoji)
			&& favorites.len() < FAVORITE_LIMIT
		{
			favorites.push(emoji.to_owned());
		}
	}
	favorites
}

pub fn decode_favorite_emojis(encoded: &str) -> Option<Vec<String>> {
	let bytes = LENIENT.decode(encoded.trim()).ok()?;
	Fields::new(&bytes).find_map(|(number, wire)| match (number, wire) {
		(FRECENCY_FAVORITE_EMOJIS, Wire::Bytes(inner)) => Some(decode_favorites(inner)),
		_ => None,
	})
}

pub fn encode_favorite_emojis(favorites: &[String]) -> String {
	let mut inner = Writer::new();
	for emoji in favorites {
		inner.bytes_field(FAVORITE_EMOJIS, emoji.as_bytes());
	}
	let mut settings = Writer::new();
	settings.bytes_field(FRECENCY_FAVORITE_EMOJIS, &inner.bytes);
	base64::engine::general_purpose::STANDARD.encode(settings.bytes)
}

fn encode_custom_status(custom: &ProtoCustomStatus) -> Vec<u8> {
	let mut writer = Writer::new();
	if !custom.text.is_empty() {
		writer.bytes_field(CUSTOM_TEXT, custom.text.as_bytes());
	}
	if let Some(id) = custom.emoji_id {
		writer.fixed_field(CUSTOM_EMOJI_ID, id.0);
	}
	if let Some(name) = custom.emoji_name.as_deref().filter(|name| !name.is_empty()) {
		writer.bytes_field(CUSTOM_EMOJI_NAME, name.as_bytes());
	}
	if let Some(expires) = custom.expires_at_ms {
		writer.fixed_field(CUSTOM_EXPIRES_AT, expires);
	}
	if let Some(created) = custom.created_at_ms {
		writer.fixed_field(CUSTOM_CREATED_AT, created);
	}
	writer.bytes
}

pub fn encode_status_settings(settings: &ProtoStatusSettings) -> String {
	let mut inner = Writer::new();
	if let Some(status) = &settings.status {
		inner.wrapped_text(STATUS_VALUE, status);
	}
	if let Some(custom) = &settings.custom_status {
		inner.bytes_field(STATUS_CUSTOM, &encode_custom_status(custom));
	}
	if let Some(expires) = settings.status_expires_at_ms {
		inner.fixed_field(STATUS_EXPIRES_AT, expires);
	}
	inner.bytes.extend_from_slice(&settings.other);
	let mut preloaded = Writer::new();
	preloaded.bytes_field(PRELOADED_STATUS, &inner.bytes);
	base64::engine::general_purpose::STANDARD.encode(preloaded.bytes)
}

struct Writer {
	bytes: Vec<u8>,
}

impl Writer {
	fn new() -> Self {
		Self { bytes: Vec::new() }
	}

	fn varint(&mut self, mut value: u64) {
		while value >= 0x80 {
			self.bytes.push((value as u8 & 0x7f) | 0x80);
			value >>= 7;
		}
		self.bytes.push(value as u8);
	}

	fn key(&mut self, number: u32, wire: u64) {
		self.varint(u64::from(number) << 3 | wire);
	}

	fn varint_field(&mut self, number: u32, value: u64) {
		self.key(number, 0);
		self.varint(value);
	}

	fn fixed_field(&mut self, number: u32, value: u64) {
		self.key(number, 1);
		self.bytes.extend_from_slice(&value.to_le_bytes());
	}

	fn bytes_field(&mut self, number: u32, bytes: &[u8]) {
		self.key(number, 2);
		self.varint(bytes.len() as u64);
		self.bytes.extend_from_slice(bytes);
	}

	fn packed_ids(&mut self, number: u32, ids: &[Snowflake]) {
		if ids.is_empty() {
			return;
		}
		let packed: Vec<u8> = ids.iter().flat_map(|id| id.0.to_le_bytes()).collect();
		self.bytes_field(number, &packed);
	}

	fn wrapped_varint(&mut self, number: u32, value: u64) {
		let mut inner = Writer::new();
		inner.varint_field(WRAPPED_VALUE, value);
		self.bytes_field(number, &inner.bytes);
	}

	fn wrapped_text(&mut self, number: u32, text: &str) {
		let mut inner = Writer::new();
		inner.bytes_field(WRAPPED_VALUE, text.as_bytes());
		self.bytes_field(number, &inner.bytes);
	}
}

fn encode_folder(folder: &ProtoGuildFolder) -> Vec<u8> {
	let mut writer = Writer::new();
	writer.packed_ids(FOLDER_GUILD_IDS, &folder.guild_ids);
	if let Some(id) = folder.id {
		writer.wrapped_varint(FOLDER_ID, id as u64);
	}
	if let Some(name) = &folder.name {
		writer.wrapped_text(FOLDER_NAME, name);
	}
	if let Some(color) = folder.color {
		writer.wrapped_varint(FOLDER_COLOR, color);
	}
	writer.bytes
}

pub fn encode_guild_folders(folders: &ProtoGuildFolders) -> String {
	let mut inner = Writer::new();
	for folder in &folders.folders {
		inner.bytes_field(FOLDERS, &encode_folder(folder));
	}
	inner.packed_ids(POSITIONS, &folders.positions);
	let mut settings = Writer::new();
	settings.bytes_field(PRELOADED_GUILD_FOLDERS, &inner.bytes);
	base64::engine::general_purpose::STANDARD.encode(settings.bytes)
}
