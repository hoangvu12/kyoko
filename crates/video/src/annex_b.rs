const SHORT_START_CODE: usize = 3;
const TYPE_MASK: u8 = 0x1f;

pub const IDR: u8 = 5;
pub const SPS: u8 = 7;

pub fn unit_type(header: u8) -> u8 {
	header & TYPE_MASK
}

fn find_unit(buffer: &[u8], from: usize) -> Option<(usize, usize)> {
	let mut index = from;
	while index + SHORT_START_CODE <= buffer.len() {
		if buffer[index..index + SHORT_START_CODE] == [0, 0, 1] {
			let long = index >= 1 && buffer[index - 1] == 0;
			return Some((index + SHORT_START_CODE, if long { 4 } else { 3 }));
		}
		index += 1;
	}
	None
}

pub fn units(frame: &[u8]) -> Vec<&[u8]> {
	let mut units = Vec::new();
	let mut current = find_unit(frame, 0);
	while let Some((start, _)) = current {
		let next = find_unit(frame, start);
		let end = next.map_or(frame.len(), |(next_start, code)| next_start - code);
		if start < end {
			units.push(&frame[start..end]);
		}
		current = next;
	}
	units
}

pub fn is_keyframe(frame: &[u8]) -> bool {
	units(frame).iter().any(|unit| unit_type(unit[0]) == IDR)
}
