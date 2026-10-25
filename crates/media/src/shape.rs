use crate::types::{Decoded, DecodedImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
	#[default]
	Square,
	Circle,
}

impl Shape {
	pub fn apply(self, decoded: &mut Decoded) {
		if self == Self::Square {
			return;
		}
		match decoded {
			Decoded::Still(image) => mask_circle(image),
			Decoded::Animated(animation) => {
				for frame in &mut animation.frames {
					mask_circle(&mut frame.image);
				}
			}
		}
	}
}

pub fn mask_circle(image: &mut DecodedImage) {
	let width = image.width as usize;
	let radius = image.width.min(image.height) as f32 / 2.;
	let centre_x = image.width as f32 / 2.;
	let centre_y = image.height as f32 / 2.;
	for (index, pixel) in image.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
		let dx = (index % width) as f32 + 0.5 - centre_x;
		let dy = (index / width) as f32 + 0.5 - centre_y;
		let coverage = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0., 1.);
		pixel[3] = (f32::from(pixel[3]) * coverage).round() as u8;
	}
}
