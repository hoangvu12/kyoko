use dave::nal;

pub const MAX_PAYLOAD_BYTES: usize = 1200;
pub const VIDEO_HEADER_LEN: usize = 16;

const VERSION_2: u8 = 0x80;
const EXTENSION_FLAG: u8 = 0x10;
const MARKER_FLAG: u8 = 0x80;
const ONE_BYTE_PROFILE: [u8; 2] = [0xbe, 0xde];
const PLAYOUT_DELAY_ID: u8 = 6;
const PLAYOUT_DELAY_LEN: usize = 3;
const RID_ID: u8 = 11;
const MAX_ELEMENT_LEN: usize = 16;
const EXTENSION_WORD: usize = 4;
const FU_A: u8 = 28;
const FU_INDICATOR_NRI_MASK: u8 = 0xe0;
const FU_START: u8 = 0x80;
const FU_END: u8 = 0x40;
const FU_HEADER_BYTES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
	pub data: Vec<u8>,
	pub marker: bool,
}

pub fn payload_frame(frame: &[u8]) -> Vec<Payload> {
	let mut payloads = Vec::new();
	for unit in nal::units(frame) {
		if unit.len() <= MAX_PAYLOAD_BYTES {
			payloads.push(Payload {
				data: unit.to_vec(),
				marker: false,
			});
		} else {
			fragment(unit, &mut payloads);
		}
	}
	if let Some(last) = payloads.last_mut() {
		last.marker = true;
	}
	payloads
}

fn fragment(unit: &[u8], payloads: &mut Vec<Payload>) {
	let indicator = (unit[0] & FU_INDICATOR_NRI_MASK) | FU_A;
	let kind = nal::unit_type(unit[0]);
	let chunks = unit[nal::HEADER_BYTES..].chunks(MAX_PAYLOAD_BYTES - FU_HEADER_BYTES);
	let count = chunks.len();
	for (index, chunk) in chunks.enumerate() {
		let mut header = kind;
		if index == 0 {
			header |= FU_START;
		}
		if index + 1 == count {
			header |= FU_END;
		}
		let mut data = Vec::with_capacity(FU_HEADER_BYTES + chunk.len());
		data.push(indicator);
		data.push(header);
		data.extend_from_slice(chunk);
		payloads.push(Payload { data, marker: false });
	}
}

pub fn extensions(rid: &str) -> Vec<u8> {
	let mut elements = vec![element_header(PLAYOUT_DELAY_ID, PLAYOUT_DELAY_LEN)];
	elements.extend_from_slice(&[0; PLAYOUT_DELAY_LEN]);
	if (1..=MAX_ELEMENT_LEN).contains(&rid.len()) {
		elements.push(element_header(RID_ID, rid.len()));
		elements.extend_from_slice(rid.as_bytes());
	}
	while !elements.len().is_multiple_of(EXTENSION_WORD) {
		elements.push(0);
	}
	elements
}

fn element_header(id: u8, len: usize) -> u8 {
	(id << 4) | ((len - 1) as u8)
}

pub fn header(
	payload_type: u8,
	marker: bool,
	sequence: u16,
	timestamp: u32,
	ssrc: u32,
	extension_len: usize,
) -> [u8; VIDEO_HEADER_LEN] {
	let words = u16::try_from(extension_len / EXTENSION_WORD).unwrap_or(u16::MAX);
	let mut header = [0u8; VIDEO_HEADER_LEN];
	header[0] = VERSION_2 | EXTENSION_FLAG;
	header[1] = payload_type | if marker { MARKER_FLAG } else { 0 };
	header[2..4].copy_from_slice(&sequence.to_be_bytes());
	header[4..8].copy_from_slice(&timestamp.to_be_bytes());
	header[8..12].copy_from_slice(&ssrc.to_be_bytes());
	header[12..14].copy_from_slice(&ONE_BYTE_PROFILE);
	header[14..16].copy_from_slice(&words.to_be_bytes());
	header
}
