use std::collections::HashMap;

const START_CODE: [u8; 4] = [0, 0, 0, 1];
const TYPE_MASK: u8 = 0x1f;
const NRI_MASK: u8 = 0xe0;
const FU_A: u8 = 28;
const FU_START: u8 = 0x80;
const FU_HEADER_BYTES: usize = 2;
const EXTENSION_STOP: u8 = 15;

pub struct Assembled {
	pub timestamp: u32,
	pub data: Vec<u8>,
	pub extensions: Vec<(u8, Vec<u8>)>,
	pub complete: bool,
}

#[derive(Default)]
struct Pending {
	timestamp: u32,
	data: Vec<u8>,
	extensions: Vec<(u8, Vec<u8>)>,
	next_sequence: Option<u16>,
	broken: bool,
}

#[derive(Default)]
pub struct Assembler {
	pending: HashMap<u32, Pending>,
}

impl Assembler {
	pub fn push(
		&mut self,
		ssrc: u32,
		sequence: u16,
		timestamp: u32,
		marker: bool,
		extensions: Vec<(u8, Vec<u8>)>,
		payload: &[u8],
	) -> Option<Assembled> {
		let pending = self.pending.entry(ssrc).or_default();
		let continuous = pending.next_sequence.is_none_or(|next| next == sequence);
		if pending.data.is_empty() || pending.timestamp != timestamp {
			*pending = Pending {
				timestamp,
				extensions,
				broken: !continuous,
				..Pending::default()
			};
		} else if !continuous {
			pending.broken = true;
		}
		pending.next_sequence = Some(sequence.wrapping_add(1));
		append(&mut pending.data, payload);
		if !marker {
			return None;
		}
		let done = std::mem::take(pending);
		pending.next_sequence = done.next_sequence;
		Some(Assembled {
			timestamp: done.timestamp,
			data: done.data,
			extensions: done.extensions,
			complete: !done.broken,
		})
	}
}

fn append(frame: &mut Vec<u8>, payload: &[u8]) {
	let Some(&header) = payload.first() else {
		return;
	};
	if header & TYPE_MASK != FU_A {
		frame.extend_from_slice(&START_CODE);
		frame.extend_from_slice(payload);
		return;
	}
	let Some((&[indicator, fragment], body)) = payload.split_first_chunk::<FU_HEADER_BYTES>() else {
		return;
	};
	if fragment & FU_START != 0 {
		frame.extend_from_slice(&START_CODE);
		frame.push((indicator & NRI_MASK) | (fragment & TYPE_MASK));
	}
	frame.extend_from_slice(body);
}

pub fn extensions(elements: &[u8]) -> Vec<(u8, Vec<u8>)> {
	let mut found = Vec::new();
	let mut index = 0;
	while index < elements.len() {
		let byte = elements[index];
		index += 1;
		if byte == 0 {
			continue;
		}
		let id = byte >> 4;
		if id == EXTENSION_STOP {
			break;
		}
		let len = usize::from(byte & 0x0f) + 1;
		let Some(data) = elements.get(index..index + len) else {
			break;
		};
		found.push((id, data.to_vec()));
		index += len;
	}
	found
}
