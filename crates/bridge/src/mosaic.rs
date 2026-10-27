use protocol::Attachment;

use crate::images::is_image;
use crate::message::{ATTACHMENT_BOX, fit_box};

pub const MOSAIC_WIDTH: f32 = 550.;
pub const MOSAIC_GAP: f32 = 4.;
pub const WIDE_PAIR_RATIO: f32 = 2.;
const ROW: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

impl Tile {
	pub fn size(&self) -> (f32, f32) {
		(self.width, self.height)
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mosaic {
	pub width: f32,
	pub height: f32,
	pub tiles: Vec<Tile>,
}

impl Mosaic {
	pub fn is_empty(&self) -> bool {
		self.tiles.is_empty()
	}
}

pub type Shape = (Option<i64>, Option<i64>);

pub fn mosaic_images(attachments: &[Attachment]) -> Vec<&Attachment> {
	attachments
		.iter()
		.filter(|attachment| is_image(attachment))
		.collect()
}

pub fn image_mosaic(images: &[&Attachment]) -> Mosaic {
	let shapes: Vec<Shape> = images
		.iter()
		.map(|attachment| (attachment.width, attachment.height))
		.collect();
	mosaic(&shapes)
}

pub fn mosaic(shapes: &[Shape]) -> Mosaic {
	match shapes {
		[] => Mosaic::default(),
		[(width, height)] => single(*width, *height),
		[first, second] if is_wide(*first) && is_wide(*second) => stacked(&[*first, *second]),
		[_, _] => rows(&[2]),
		[_, _, _] => feature_three(),
		[_, _, _, _] => rows(&[2, 2]),
		_ => rows(&row_counts(shapes.len())),
	}
}

pub fn row_counts(count: usize) -> Vec<usize> {
	let lead = match count % ROW {
		0 => None,
		rest => Some(rest),
	};
	lead.into_iter()
		.chain(std::iter::repeat_n(ROW, count / ROW))
		.collect()
}

pub fn cover_box(width: Option<i64>, height: Option<i64>, tile: (f32, f32)) -> (f32, f32) {
	let Some(ratio) = ratio((width, height)) else {
		return tile;
	};
	if ratio >= tile.0 / tile.1 {
		((tile.1 * ratio).round().max(1.), tile.1)
	} else {
		(tile.0, (tile.0 / ratio).round().max(1.))
	}
}

fn ratio((width, height): Shape) -> Option<f32> {
	match (width, height) {
		(Some(width), Some(height)) if width > 0 && height > 0 => Some(width as f32 / height as f32),
		_ => None,
	}
}

fn is_wide(shape: Shape) -> bool {
	ratio(shape).is_some_and(|ratio| ratio >= WIDE_PAIR_RATIO)
}

fn single(width: Option<i64>, height: Option<i64>) -> Mosaic {
	let (width, height) = fit_box(width, height, ATTACHMENT_BOX);
	Mosaic {
		width,
		height,
		tiles: vec![Tile {
			x: 0.,
			y: 0.,
			width,
			height,
		}],
	}
}

fn stacked(shapes: &[Shape]) -> Mosaic {
	let ceiling = (ATTACHMENT_BOX.1 - MOSAIC_GAP * (shapes.len() - 1) as f32) / shapes.len() as f32;
	let mut tiles = Vec::with_capacity(shapes.len());
	let mut y = 0.;
	for shape in shapes {
		let natural = ratio(*shape).map_or(ceiling, |ratio| (MOSAIC_WIDTH / ratio).round());
		let height = natural.clamp(1., ceiling);
		tiles.push(Tile {
			x: 0.,
			y,
			width: MOSAIC_WIDTH,
			height,
		});
		y += height + MOSAIC_GAP;
	}
	Mosaic {
		width: MOSAIC_WIDTH,
		height: y - MOSAIC_GAP,
		tiles,
	}
}

fn cell(columns: usize) -> f32 {
	(MOSAIC_WIDTH - MOSAIC_GAP * (columns - 1) as f32) / columns as f32
}

fn feature_three() -> Mosaic {
	let small = cell(ROW);
	let large = MOSAIC_WIDTH - MOSAIC_GAP - small;
	let right = large + MOSAIC_GAP;
	Mosaic {
		width: MOSAIC_WIDTH,
		height: large,
		tiles: vec![
			Tile {
				x: 0.,
				y: 0.,
				width: large,
				height: large,
			},
			Tile {
				x: right,
				y: 0.,
				width: small,
				height: small,
			},
			Tile {
				x: right,
				y: small + MOSAIC_GAP,
				width: small,
				height: large - small - MOSAIC_GAP,
			},
		],
	}
}

fn row_height(columns: usize) -> f32 {
	match columns {
		1 => cell(2),
		columns => cell(columns),
	}
}

fn rows(counts: &[usize]) -> Mosaic {
	let mut tiles = Vec::new();
	let mut y = 0.;
	for &columns in counts {
		let (width, height) = (cell(columns), row_height(columns));
		tiles.extend((0..columns).map(|column| Tile {
			x: column as f32 * (width + MOSAIC_GAP),
			y,
			width,
			height,
		}));
		y += height + MOSAIC_GAP;
	}
	Mosaic {
		width: MOSAIC_WIDTH,
		height: (y - MOSAIC_GAP).max(0.),
		tiles,
	}
}
