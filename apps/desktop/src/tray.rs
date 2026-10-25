use std::cell::Cell;

use image::imageops::{self, FilterType};
use platform::tray::{Tray, TrayEvent, TrayImage, TrayLabels};
use state::Attention;

const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");
const SIZE: u32 = 32;
const BADGE_CENTRE: f32 = 24.5;
const DOT_RADIUS: f32 = 5.5;
const MENTION_RADIUS: f32 = 7.5;
const RING: f32 = 1.5;
const RING_COLOUR: [u8; 3] = [0x1e, 0x1f, 0x22];
const DOT_COLOUR: [u8; 3] = [0xff, 0xff, 0xff];
const MENTION_COLOUR: [u8; 3] = [0xf2, 0x3f, 0x43];
const NAME: &str = "Kyoko";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Badge {
	None,
	Unread,
	Mentions,
}

pub struct TrayIcon {
	tray: Tray,
	base: TrayImage,
	shown: Cell<Option<Attention>>,
}

impl TrayIcon {
	pub fn create(on_event: impl Fn(TrayEvent) + Send + Sync + 'static) -> Option<TrayIcon> {
		let base = match base_image() {
			Ok(base) => base,
			Err(error) => {
				tracing::warn!(%error, "the tray icon image did not decode");
				return None;
			}
		};
		let labels = TrayLabels {
			show: "Show Kyoko".to_owned(),
			hide: "Hide Kyoko".to_owned(),
			quit: "Quit Kyoko".to_owned(),
		};
		match Tray::new(NAME, base.clone(), &labels, on_event) {
			Ok(tray) => Some(TrayIcon {
				tray,
				base,
				shown: Cell::new(None),
			}),
			Err(error) => {
				tracing::warn!(%error, "the tray icon could not be created");
				None
			}
		}
	}

	pub fn show(&self, attention: Attention) {
		let before = self.shown.replace(Some(attention));
		if before == Some(attention) {
			return;
		}
		let wanted = badge(attention);
		if before.map(badge) != Some(wanted)
			&& let Err(error) = self.tray.set_image(badged(&self.base, wanted))
		{
			tracing::warn!(%error, "the tray icon could not be updated");
		}
		if let Err(error) = self.tray.set_tooltip(&tooltip(attention)) {
			tracing::warn!(%error, "the tray tooltip could not be updated");
		}
	}
}

fn badge(attention: Attention) -> Badge {
	if attention.mentions > 0 {
		Badge::Mentions
	} else if attention.unread {
		Badge::Unread
	} else {
		Badge::None
	}
}

fn tooltip(attention: Attention) -> String {
	match (attention.mentions, attention.unread) {
		(0, false) => NAME.to_owned(),
		(0, true) => format!("{NAME}: unread messages"),
		(1, _) => format!("{NAME}: 1 mention"),
		(count, _) => format!("{NAME}: {count} mentions"),
	}
}

fn base_image() -> image::ImageResult<TrayImage> {
	let decoded = image::load_from_memory(ICON_PNG)?.into_rgba8();
	let resized = imageops::resize(&decoded, SIZE, SIZE, FilterType::Lanczos3);
	Ok(TrayImage {
		rgba: resized.into_raw(),
		width: SIZE,
		height: SIZE,
	})
}

fn badged(base: &TrayImage, badge: Badge) -> TrayImage {
	let (radius, colour) = match badge {
		Badge::None => return base.clone(),
		Badge::Unread => (DOT_RADIUS, DOT_COLOUR),
		Badge::Mentions => (MENTION_RADIUS, MENTION_COLOUR),
	};
	let mut image = base.clone();
	paint_disc(&mut image, radius + RING, RING_COLOUR);
	paint_disc(&mut image, radius, colour);
	image
}

fn paint_disc(image: &mut TrayImage, radius: f32, colour: [u8; 3]) {
	for y in 0..image.height {
		for x in 0..image.width {
			let dx = x as f32 + 0.5 - BADGE_CENTRE;
			let dy = y as f32 + 0.5 - BADGE_CENTRE;
			let coverage = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0., 1.);
			if coverage <= 0. {
				continue;
			}
			let index = ((y * image.width + x) * 4) as usize;
			let pixel = &mut image.rgba[index..index + 4];
			for (channel, target) in pixel.iter_mut().zip(colour) {
				*channel =
					(f32::from(*channel) * (1. - coverage) + f32::from(target) * coverage).round() as u8;
			}
			pixel[3] = (f32::from(pixel[3]) * (1. - coverage) + 255. * coverage).round() as u8;
		}
	}
}
