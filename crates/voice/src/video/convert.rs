use std::sync::{Arc, OnceLock};

use super::{VideoError, VideoFrame};

const RGBA_BYTES: usize = 4;
const LUMA_FLOOR: i32 = 16;
const CHROMA_MIDDLE: i32 = 128;
const LUMA_SCALE: i32 = 298;
const RED_FROM_V: i32 = 409;
const GREEN_FROM_U: i32 = 100;
const GREEN_FROM_V: i32 = 208;
const BLUE_FROM_U: i32 = 516;
const ROUNDING: i32 = 128;
const OPAQUE: u8 = 255;
const LEVELS: usize = 256;
const CLAMP_OFFSET: i32 = 1024;
const CLAMP_LEVELS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
	pub width: u32,
	pub height: u32,
	pub rgba: Arc<[u8]>,
}

struct Tables {
	luma: [i32; LEVELS],
	red_v: [i32; LEVELS],
	green_u: [i32; LEVELS],
	green_v: [i32; LEVELS],
	blue_u: [i32; LEVELS],
	clamp: [u8; CLAMP_LEVELS],
}

impl Tables {
	fn build() -> Self {
		let mut tables = Self {
			luma: [0; LEVELS],
			red_v: [0; LEVELS],
			green_u: [0; LEVELS],
			green_v: [0; LEVELS],
			blue_u: [0; LEVELS],
			clamp: [0; CLAMP_LEVELS],
		};
		for level in 0..LEVELS {
			let value = level as i32;
			tables.luma[level] = LUMA_SCALE * (value - LUMA_FLOOR) + ROUNDING;
			tables.red_v[level] = RED_FROM_V * (value - CHROMA_MIDDLE);
			tables.green_u[level] = -GREEN_FROM_U * (value - CHROMA_MIDDLE);
			tables.green_v[level] = -GREEN_FROM_V * (value - CHROMA_MIDDLE);
			tables.blue_u[level] = BLUE_FROM_U * (value - CHROMA_MIDDLE);
		}
		for (index, slot) in tables.clamp.iter_mut().enumerate() {
			*slot = (index as i32 - CLAMP_OFFSET).clamp(0, 255) as u8;
		}
		tables
	}

	fn channel(&self, value: i32) -> u8 {
		self.clamp[((value >> 8) + CLAMP_OFFSET) as usize & (CLAMP_LEVELS - 1)]
	}
}

fn tables() -> &'static Tables {
	static TABLES: OnceLock<Tables> = OnceLock::new();
	TABLES.get_or_init(Tables::build)
}

impl Picture {
	pub fn from_nv12(frame: &VideoFrame, max_width: u32, max_height: u32) -> Result<Self, VideoError> {
		if frame.width < 2 || frame.height < 2 || frame.nv12.len() < frame.expected_len() {
			return Err(VideoError::InvalidFrame("buffer does not match dimensions"));
		}
		let (width, height) = fit(frame.width, frame.height, max_width, max_height);
		let source_width = frame.width as usize;
		let source_height = frame.height as usize;
		let (luma, chroma) = frame.nv12.split_at(source_width * source_height);
		let columns: Vec<(usize, usize)> = (0..width as usize)
			.map(|column| {
				let source = column * source_width / width as usize;
				(source, source & !1)
			})
			.collect();
		let tables = tables();
		let row_bytes = width as usize * RGBA_BYTES;
		let mut rgba = vec![OPAQUE; row_bytes * height as usize];
		for (row, target) in rgba.chunks_exact_mut(row_bytes).enumerate() {
			let source_row = row * source_height / height as usize;
			let luma_line = &luma[source_row * source_width..][..source_width];
			let chroma_line = &chroma[(source_row / 2) * source_width..][..source_width];
			convert_row(tables, luma_line, chroma_line, &columns, target);
		}
		Ok(Self {
			width,
			height,
			rgba: rgba.into(),
		})
	}
}

fn convert_row(tables: &Tables, luma: &[u8], chroma: &[u8], columns: &[(usize, usize)], target: &mut [u8]) {
	let mut offset = 0;
	for &(column, pair) in columns {
		let base = tables.luma[usize::from(luma[column])];
		let u = usize::from(chroma[pair]);
		let v = usize::from(chroma[pair + 1]);
		target[offset] = tables.channel(base + tables.red_v[v]);
		target[offset + 1] = tables.channel(base + tables.green_u[u] + tables.green_v[v]);
		target[offset + 2] = tables.channel(base + tables.blue_u[u]);
		offset += RGBA_BYTES;
	}
}

pub fn fit(source_width: u32, source_height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
	if source_width == 0 || source_height == 0 {
		return (0, 0);
	}
	let scale = (f64::from(max_width.max(1)) / f64::from(source_width))
		.min(f64::from(max_height.max(1)) / f64::from(source_height))
		.min(1.0);
	let even = |value: u32| (((f64::from(value) * scale).round() as u32).max(2)) & !1;
	(even(source_width), even(source_height))
}
