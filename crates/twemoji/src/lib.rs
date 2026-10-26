mod pack;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

pub use pack::{BLOCK_BYTES, write as pack};

pub const RELEASE: &str = "v17.0.3";
pub const RELEASE_URL: &str = "https://github.com/jdecked/twemoji/archive/refs/tags/v17.0.3.tar.gz";
pub const RELEASE_SHA256: &str = "a0855654b633045ae2337537e77f1bb4361162f7fcd910e613eaab1d6d9c5fca";
pub const PACK_PATH: &str = "crates/twemoji/assets/twemoji.pack";

const PRESENTATION: char = '\u{FE0F}';
const TEXT_PRESENTATION: char = '\u{FE0E}';
const TEXT_DEFAULT: [char; 3] = ['\u{A9}', '\u{AE}', '\u{2122}'];
const KEYCAP_BASES: &[u8] = b"0123456789#*";

static PACKED: &[u8] = include_bytes!("../assets/twemoji.pack");

static INDEX: LazyLock<Index> = LazyLock::new(|| Index::new(PACKED));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'a> {
	Text(&'a str),
	Emoji { text: &'a str, file: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
	pub files: usize,
	pub blocks: usize,
	pub packed_bytes: usize,
	pub svg_bytes: usize,
}

struct Index {
	pack: pack::Pack<'static>,
	by_name: HashMap<Box<str>, usize>,
	by_key: HashMap<Box<str>, usize>,
	prefixes: HashSet<Box<str>>,
	longest: usize,
	block: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl Index {
	fn new(bytes: &'static [u8]) -> Self {
		let pack = pack::read(bytes).unwrap_or_default();
		let mut by_name = HashMap::with_capacity(pack.entries.len());
		let mut by_key = HashMap::with_capacity(pack.entries.len());
		let mut prefixes = HashSet::new();
		let mut longest = 0;
		for (position, entry) in pack.entries.iter().enumerate() {
			by_name.insert(entry.name.clone(), position);
			let Some(chars) = chars_of(&entry.name) else {
				continue;
			};
			let key: Vec<char> = chars.into_iter().filter(|c| *c != PRESENTATION).collect();
			longest = longest.max(key.len());
			for end in 1..key.len() {
				prefixes.insert(key[..end].iter().collect::<String>().into_boxed_str());
			}
			by_key
				.entry(key.iter().collect::<String>().into_boxed_str())
				.or_insert(position);
		}
		Self {
			pack,
			by_name,
			by_key,
			prefixes,
			longest,
			block: Mutex::new(None),
		}
	}

	fn name(&self, position: usize) -> &str {
		&self.pack.entries[position].name
	}

	fn block(&self, block: u32) -> Option<Arc<Vec<u8>>> {
		let mut cached = self.block.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
		if let Some((id, bytes)) = cached.as_ref()
			&& *id == block
		{
			return Some(bytes.clone());
		}
		let bytes = Arc::new(pack::unpack(*self.pack.blocks.get(block as usize)?)?);
		*cached = Some((block, bytes.clone()));
		Some(bytes)
	}

	fn svg(&self, name: &str) -> Option<Vec<u8>> {
		let entry = &self.pack.entries[*self.by_name.get(name)?];
		let block = self.block(entry.block)?;
		let start = entry.offset as usize;
		block.get(start..start + entry.len as usize).map(<[u8]>::to_vec)
	}

	fn match_at(&self, text: &str) -> Option<(usize, usize)> {
		let first = *text.as_bytes().first()?;
		if first.is_ascii() && !KEYCAP_BASES.contains(&first) {
			return None;
		}
		let mut key = String::new();
		let mut count = 0;
		let mut best = None;
		for (offset, c) in text.char_indices() {
			if c == PRESENTATION {
				if key.is_empty() {
					return None;
				}
				continue;
			}
			key.push(c);
			count += 1;
			if count > self.longest {
				break;
			}
			if let Some(position) = self.by_key.get(key.as_str()) {
				best = Some((offset + c.len_utf8(), *position, count));
			}
			if !self.prefixes.contains(key.as_str()) {
				break;
			}
		}
		let (mut end, position, count) = best?;
		let rest = &text[end..];
		if rest.starts_with(TEXT_PRESENTATION) {
			return None;
		}
		let presented = rest.starts_with(PRESENTATION);
		if count == 1 && !presented && text.chars().next().is_some_and(|c| TEXT_DEFAULT.contains(&c)) {
			return None;
		}
		end += rest.len() - rest.trim_start_matches(PRESENTATION).len();
		Some((end, position))
	}
}

fn chars_of(name: &str) -> Option<Vec<char>> {
	name.split('-')
		.map(|part| u32::from_str_radix(part, 16).ok().and_then(char::from_u32))
		.collect()
}

pub fn file(sequence: &str) -> Option<&'static str> {
	let index = &*INDEX;
	let key: String = sequence.chars().filter(|c| *c != PRESENTATION).collect();
	index
		.by_key
		.get(key.as_str())
		.map(|position| index.name(*position))
}

pub fn svg(file: &str) -> Option<Vec<u8>> {
	INDEX.svg(file)
}

pub fn has_emoji(text: &str) -> bool {
	split(text).any(|piece| matches!(piece, Piece::Emoji { .. }))
}

pub fn split(text: &str) -> Pieces<'_> {
	Pieces { text, at: 0 }
}

pub fn stats() -> Stats {
	let index = &*INDEX;
	Stats {
		files: index.pack.entries.len(),
		blocks: index.pack.blocks.len(),
		packed_bytes: PACKED.len(),
		svg_bytes: index.pack.entries.iter().map(|entry| entry.len as usize).sum(),
	}
}

pub fn names() -> impl Iterator<Item = &'static str> {
	INDEX.pack.entries.iter().map(|entry| &*entry.name)
}

pub struct Pieces<'a> {
	text: &'a str,
	at: usize,
}

impl<'a> Iterator for Pieces<'a> {
	type Item = Piece<'a>;

	fn next(&mut self) -> Option<Piece<'a>> {
		let index = &*INDEX;
		let rest = &self.text[self.at..];
		if rest.is_empty() {
			return None;
		}
		if let Some((end, position)) = index.match_at(rest) {
			self.at += end;
			return Some(Piece::Emoji {
				text: &rest[..end],
				file: index.name(position),
			});
		}
		let mut cut = rest.len();
		for (offset, _) in rest.char_indices().skip(1) {
			if index.match_at(&rest[offset..]).is_some() {
				cut = offset;
				break;
			}
		}
		self.at += cut;
		Some(Piece::Text(&rest[..cut]))
	}
}
