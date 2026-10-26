use std::io;

const MAGIC: &[u8; 8] = b"KYTWEMO1";
const LEVEL: i32 = 19;
pub const BLOCK_BYTES: usize = 512 * 1024;
const MAX_RAW_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
	pub name: Box<str>,
	pub block: u32,
	pub offset: u32,
	pub len: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Block<'a> {
	pub frame: &'a [u8],
	pub raw_len: usize,
}

#[derive(Debug, Default)]
pub struct Pack<'a> {
	pub entries: Vec<Entry>,
	pub blocks: Vec<Block<'a>>,
}

pub fn write(files: &[(String, Vec<u8>)]) -> io::Result<Vec<u8>> {
	let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
	sorted.sort_by(|left, right| left.0.cmp(&right.0));
	let mut index = String::new();
	let mut blocks: Vec<Vec<u8>> = Vec::new();
	let mut current: Vec<u8> = Vec::new();
	for (name, bytes) in sorted {
		if !current.is_empty() && current.len() + bytes.len() > BLOCK_BYTES {
			blocks.push(std::mem::take(&mut current));
		}
		index.push_str(&format!(
			"{name}\t{}\t{}\t{}\n",
			blocks.len(),
			current.len(),
			bytes.len()
		));
		current.extend_from_slice(bytes);
	}
	if !current.is_empty() {
		blocks.push(current);
	}
	let mut out = Vec::new();
	out.extend_from_slice(MAGIC);
	push_frame(&mut out, index.as_bytes())?;
	out.extend_from_slice(&u32_of(blocks.len())?.to_le_bytes());
	for block in &blocks {
		push_frame(&mut out, block)?;
	}
	Ok(out)
}

fn push_frame(out: &mut Vec<u8>, raw: &[u8]) -> io::Result<()> {
	let frame = zstd::bulk::compress(raw, LEVEL)?;
	out.extend_from_slice(&u32_of(frame.len())?.to_le_bytes());
	out.extend_from_slice(&u32_of(raw.len())?.to_le_bytes());
	out.extend_from_slice(&frame);
	Ok(())
}

fn u32_of(value: usize) -> io::Result<u32> {
	u32::try_from(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the pack is too large"))
}

pub fn read(bytes: &[u8]) -> Option<Pack<'_>> {
	let mut cursor = Cursor { bytes, at: 0 };
	if cursor.take(MAGIC.len())? != MAGIC {
		return None;
	}
	let index = cursor.frame()?;
	let raw = zstd::bulk::decompress(index.frame, index.raw_len).ok()?;
	let text = std::str::from_utf8(&raw).ok()?;
	let count = cursor.u32()? as usize;
	let mut blocks = Vec::with_capacity(count.min(1024));
	for _ in 0..count {
		blocks.push(cursor.frame()?);
	}
	let entries = text.lines().filter_map(|line| entry(line, &blocks)).collect();
	Some(Pack { entries, blocks })
}

fn entry(line: &str, blocks: &[Block<'_>]) -> Option<Entry> {
	let mut fields = line.split('\t');
	let name = fields.next()?;
	let block: u32 = fields.next()?.parse().ok()?;
	let offset: u32 = fields.next()?.parse().ok()?;
	let len: u32 = fields.next()?.parse().ok()?;
	let raw_len = blocks.get(block as usize)?.raw_len;
	if name.is_empty() || offset as usize + len as usize > raw_len {
		return None;
	}
	Some(Entry {
		name: name.into(),
		block,
		offset,
		len,
	})
}

struct Cursor<'a> {
	bytes: &'a [u8],
	at: usize,
}

impl<'a> Cursor<'a> {
	fn take(&mut self, len: usize) -> Option<&'a [u8]> {
		let end = self.at.checked_add(len)?;
		let slice = self.bytes.get(self.at..end)?;
		self.at = end;
		Some(slice)
	}

	fn u32(&mut self) -> Option<u32> {
		Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
	}

	fn frame(&mut self) -> Option<Block<'a>> {
		let len = self.u32()? as usize;
		let raw_len = self.u32()? as usize;
		if raw_len > MAX_RAW_BYTES {
			return None;
		}
		Some(Block {
			frame: self.take(len)?,
			raw_len,
		})
	}
}

pub fn unpack(block: Block<'_>) -> Option<Vec<u8>> {
	zstd::bulk::decompress(block.frame, block.raw_len).ok()
}
