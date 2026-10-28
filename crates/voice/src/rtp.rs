pub const HEADER_LEN: usize = 12;

const VERSION_2: u8 = 0x80;
const RTCP_PAYLOAD_TYPES: std::ops::RangeInclusive<u8> = 192..=223;
const VERSION_MASK: u8 = 0xc0;
const PADDING_FLAG: u8 = 0x20;
const EXTENSION_FLAG: u8 = 0x10;
const CSRC_COUNT_MASK: u8 = 0x0f;
const PAYLOAD_TYPE_MASK: u8 = 0x7f;
const MARKER_FLAG: u8 = 0x80;
const CSRC_LEN: usize = 4;
const RTCP_HEADER_LEN: usize = 8;
const EXTENSION_PREAMBLE_LEN: usize = 4;
const EXTENSION_WORD_LEN: usize = 4;

pub fn write_header(payload_type: u8, sequence: u16, timestamp: u32, ssrc: u32) -> [u8; HEADER_LEN] {
	let mut header = [0u8; HEADER_LEN];
	header[0] = VERSION_2;
	header[1] = payload_type;
	header[2..4].copy_from_slice(&sequence.to_be_bytes());
	header[4..8].copy_from_slice(&timestamp.to_be_bytes());
	header[8..12].copy_from_slice(&ssrc.to_be_bytes());
	header
}

pub fn is_rtcp(packet: &[u8]) -> bool {
	packet.len() >= RTCP_HEADER_LEN
		&& packet[0] & VERSION_MASK == VERSION_2
		&& RTCP_PAYLOAD_TYPES.contains(&packet[1])
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
	pub payload_type: u8,
	pub marker: bool,
	pub sequence: u16,
	pub timestamp: u32,
	pub ssrc: u32,
	pub header_len: usize,
	extension_len: usize,
	padded: bool,
}

pub fn parse(packet: &[u8]) -> Option<Header> {
	if packet.len() < HEADER_LEN || packet[0] & VERSION_MASK != VERSION_2 {
		return None;
	}
	let mut header_len = HEADER_LEN + CSRC_LEN * usize::from(packet[0] & CSRC_COUNT_MASK);
	let mut extension_len = 0;
	if packet[0] & EXTENSION_FLAG != 0 {
		let preamble = packet.get(header_len..header_len + EXTENSION_PREAMBLE_LEN)?;
		extension_len = usize::from(u16::from_be_bytes([preamble[2], preamble[3]])) * EXTENSION_WORD_LEN;
		header_len += EXTENSION_PREAMBLE_LEN;
	}
	if packet.len() < header_len {
		return None;
	}
	Some(Header {
		payload_type: packet[1] & PAYLOAD_TYPE_MASK,
		marker: packet[1] & MARKER_FLAG != 0,
		sequence: u16::from_be_bytes([packet[2], packet[3]]),
		timestamp: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
		ssrc: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
		header_len,
		extension_len,
		padded: packet[0] & PADDING_FLAG != 0,
	})
}

impl Header {
	pub fn into_payload(self, mut decrypted: Vec<u8>) -> Option<Vec<u8>> {
		if self.padded {
			let padding = usize::from(*decrypted.last()?);
			if padding == 0 || padding > decrypted.len() {
				return None;
			}
			decrypted.truncate(decrypted.len() - padding);
		}
		if self.extension_len > 0 {
			if self.extension_len > decrypted.len() {
				return None;
			}
			decrypted.drain(..self.extension_len);
		}
		(!decrypted.is_empty()).then_some(decrypted)
	}
}
