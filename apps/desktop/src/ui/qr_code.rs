use bridge::themes::Rgba;
use qrcode::{Color, EcLevel, QrCode};
use zgx::image::ImageData;

pub fn render(url: &str, side: u32, dark: Rgba) -> Option<ImageData> {
	let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::Q).ok()?;
	let modules = code.width();
	let colors = code.into_colors();
	let pixels = side as usize;
	let ink = [dark.red, dark.green, dark.blue, u8::MAX];
	let mut rgba = vec![0; pixels * pixels * 4];
	for row in 0..pixels {
		let module_row = row * modules / pixels;
		for column in 0..pixels {
			if colors[module_row * modules + column * modules / pixels] == Color::Dark {
				let at = (row * pixels + column) * 4;
				rgba[at..at + 4].copy_from_slice(&ink);
			}
		}
	}
	ImageData::new(side, side, rgba).ok()
}
