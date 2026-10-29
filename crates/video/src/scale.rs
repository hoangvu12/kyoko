use super::{VideoError, VideoFrame};

const BGRA_BYTES: usize = 4;

pub struct Bgra<'a> {
	pub pixels: &'a [u8],
	pub width: u32,
	pub height: u32,
	pub pitch: usize,
}

fn luma(blue: i32, green: i32, red: i32) -> u8 {
	(((66 * red + 129 * green + 25 * blue + 128) >> 8) + 16).clamp(0, 255) as u8
}

fn chroma(blue: i32, green: i32, red: i32) -> [u8; 2] {
	let u = ((-38 * red - 74 * green + 112 * blue + 128) >> 8) + 128;
	let v = ((112 * red - 94 * green - 18 * blue + 128) >> 8) + 128;
	[u.clamp(0, 255) as u8, v.clamp(0, 255) as u8]
}

pub fn bgra_to_nv12(source: &Bgra<'_>, width: u32, height: u32) -> Result<Vec<u8>, VideoError> {
	let source_width = source.width as usize;
	let source_height = source.height as usize;
	if source_width == 0
		|| source_height == 0
		|| source.pitch < source_width * BGRA_BYTES
		|| source.pixels.len() < source.pitch * (source_height - 1) + source_width * BGRA_BYTES
	{
		return Err(VideoError::InvalidFrame("buffer does not match dimensions"));
	}
	let width = (width.max(2) & !1) as usize;
	let height = (height.max(2) & !1) as usize;
	let columns: Vec<usize> = (0..width)
		.map(|column| column * source_width / width * BGRA_BYTES)
		.collect();
	let pixel = |row: usize, column: usize| {
		let line = (row * source_height / height) * source.pitch;
		let at = line + columns[column];
		let bytes = &source.pixels[at..at + 3];
		(i32::from(bytes[0]), i32::from(bytes[1]), i32::from(bytes[2]))
	};
	let mut nv12 = vec![0u8; width * height * 3 / 2];
	let (target_luma, target_chroma) = nv12.split_at_mut(width * height);
	for (row, target) in target_luma.chunks_exact_mut(width).enumerate() {
		for (column, value) in target.iter_mut().enumerate() {
			let (blue, green, red) = pixel(row, column);
			*value = luma(blue, green, red);
		}
	}
	for (row, target) in target_chroma.chunks_exact_mut(width).enumerate() {
		for (pair, value) in target.as_chunks_mut::<2>().0.iter_mut().enumerate() {
			let (blue, green, red) = pixel(row * 2, pair * 2);
			*value = chroma(blue, green, red);
		}
	}
	Ok(nv12)
}

pub fn scale_nv12(frame: &VideoFrame, width: u32, height: u32, out: &mut Vec<u8>) -> Result<(), VideoError> {
	if frame.width < 2 || frame.height < 2 || frame.nv12.len() < frame.expected_len() {
		return Err(VideoError::InvalidFrame("buffer does not match dimensions"));
	}
	out.clear();
	if frame.width == width && frame.height == height {
		out.extend_from_slice(&frame.nv12[..frame.expected_len()]);
		return Ok(());
	}
	let source_width = frame.width as usize;
	let source_height = frame.height as usize;
	let width = width as usize;
	let height = height as usize;
	let (luma, chroma) = frame.nv12.split_at(source_width * source_height);
	let columns: Vec<usize> = (0..width).map(|column| column * source_width / width).collect();
	let pairs: Vec<usize> = (0..width / 2)
		.map(|pair| pair * (source_width / 2) / (width / 2))
		.collect();
	out.resize(width * height * 3 / 2, 0);
	let (target_luma, target_chroma) = out.split_at_mut(width * height);
	for (row, target) in target_luma.chunks_exact_mut(width).enumerate() {
		let line = &luma[(row * source_height / height) * source_width..][..source_width];
		for (pixel, column) in target.iter_mut().zip(&columns) {
			*pixel = line[*column];
		}
	}
	let chroma_rows = source_height / 2;
	for (row, target) in target_chroma.chunks_exact_mut(width).enumerate() {
		let line = &chroma[(row * chroma_rows / (height / 2)) * source_width..][..source_width];
		for (pixel, pair) in target.as_chunks_mut::<2>().0.iter_mut().zip(&pairs) {
			*pixel = [line[pair * 2], line[pair * 2 + 1]];
		}
	}
	Ok(())
}
