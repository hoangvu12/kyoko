use super::{VideoError, VideoFrame};

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
