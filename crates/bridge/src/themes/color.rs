#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
	pub red: u8,
	pub green: u8,
	pub blue: u8,
	pub alpha: f32,
}

const NAMED: &[(&str, [u8; 3])] = &[
	("black", [0, 0, 0]),
	("white", [255, 255, 255]),
	("red", [255, 0, 0]),
	("green", [0, 128, 0]),
	("lime", [0, 255, 0]),
	("blue", [0, 0, 255]),
	("yellow", [255, 255, 0]),
	("orange", [255, 165, 0]),
	("purple", [128, 0, 128]),
	("pink", [255, 192, 203]),
	("cyan", [0, 255, 255]),
	("aqua", [0, 255, 255]),
	("magenta", [255, 0, 255]),
	("fuchsia", [255, 0, 255]),
	("gray", [128, 128, 128]),
	("grey", [128, 128, 128]),
	("silver", [192, 192, 192]),
	("maroon", [128, 0, 0]),
	("navy", [0, 0, 128]),
	("teal", [0, 128, 128]),
	("olive", [128, 128, 0]),
	("crimson", [220, 20, 60]),
	("gold", [255, 215, 0]),
	("indigo", [75, 0, 130]),
	("violet", [238, 130, 238]),
];

impl Rgba {
	pub fn hex(&self) -> String {
		let alpha = (self.alpha.clamp(0., 1.) * 255.).round() as u8;
		if alpha == u8::MAX {
			return format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue);
		}
		format!("#{:02x}{:02x}{:02x}{alpha:02x}", self.red, self.green, self.blue)
	}

	pub fn lightness(&self) -> f32 {
		let channels = [self.red, self.green, self.blue];
		let high = channels.iter().max().copied().unwrap_or(0) as f32;
		let low = channels.iter().min().copied().unwrap_or(0) as f32;
		(high + low) / 2. / 255.
	}
}

pub fn parse_color(text: &str) -> Option<Rgba> {
	let text = text.trim();
	if let Some(digits) = text.strip_prefix('#') {
		return hex(digits);
	}
	let lower = text.to_ascii_lowercase();
	if lower == "transparent" {
		return Some(Rgba {
			red: 0,
			green: 0,
			blue: 0,
			alpha: 0.,
		});
	}
	if let Some((_, [red, green, blue])) = NAMED.iter().find(|(name, _)| *name == lower) {
		return Some(Rgba {
			red: *red,
			green: *green,
			blue: *blue,
			alpha: 1.,
		});
	}
	let (function, inner) = lower.strip_suffix(')')?.split_once('(')?;
	let parts = arguments(inner)?;
	match function.trim() {
		"rgb" | "rgba" => rgb(&parts),
		"hsl" | "hsla" => hsl(&parts),
		_ => None,
	}
}

fn hex(digits: &str) -> Option<Rgba> {
	if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
		return None;
	}
	let digit = |index: usize| u8::from_str_radix(digits.get(index..=index)?, 16).ok();
	let pair = |index: usize| u8::from_str_radix(digits.get(index..index + 2)?, 16).ok();
	let (red, green, blue, alpha) = match digits.len() {
		3 | 4 => (
			digit(0)? * 17,
			digit(1)? * 17,
			digit(2)? * 17,
			if digits.len() == 4 { digit(3)? * 17 } else { 255 },
		),
		6 | 8 => (
			pair(0)?,
			pair(2)?,
			pair(4)?,
			if digits.len() == 8 { pair(6)? } else { 255 },
		),
		_ => return None,
	};
	Some(Rgba {
		red,
		green,
		blue,
		alpha: f32::from(alpha) / 255.,
	})
}

fn arguments(inner: &str) -> Option<Vec<&str>> {
	let (channels, alpha) = match inner.split_once('/') {
		Some((channels, alpha)) => (channels, Some(alpha.trim())),
		None => (inner, None),
	};
	let mut parts: Vec<&str> = channels
		.split([',', ' ', '\t', '\n'])
		.map(str::trim)
		.filter(|part| !part.is_empty())
		.collect();
	if let Some(alpha) = alpha {
		if parts.len() != 3 || alpha.is_empty() {
			return None;
		}
		parts.push(alpha);
	}
	(3..=4).contains(&parts.len()).then_some(parts)
}

fn number(part: &str) -> Option<f32> {
	part.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn alpha(parts: &[&str]) -> Option<f32> {
	let Some(part) = parts.get(3) else {
		return Some(1.);
	};
	let value = match part.strip_suffix('%') {
		Some(percent) => number(percent)? / 100.,
		None => number(part)?,
	};
	Some(value.clamp(0., 1.))
}

fn rgb(parts: &[&str]) -> Option<Rgba> {
	let channel = |part: &str| -> Option<u8> {
		let value = match part.strip_suffix('%') {
			Some(percent) => number(percent)? * 2.55,
			None => number(part)?,
		};
		Some(value.round().clamp(0., 255.) as u8)
	};
	Some(Rgba {
		red: channel(parts[0])?,
		green: channel(parts[1])?,
		blue: channel(parts[2])?,
		alpha: alpha(parts)?,
	})
}

fn hue(part: &str) -> Option<f32> {
	let degrees = if let Some(value) = part.strip_suffix("deg") {
		number(value)?
	} else if let Some(value) = part.strip_suffix("grad") {
		number(value)? * 0.9
	} else if let Some(value) = part.strip_suffix("rad") {
		number(value)?.to_degrees()
	} else if let Some(value) = part.strip_suffix("turn") {
		number(value)? * 360.
	} else {
		number(part)?
	};
	Some(degrees.rem_euclid(360.))
}

fn percent(part: &str) -> Option<f32> {
	let value = number(part.strip_suffix('%').unwrap_or(part))?;
	Some((value / 100.).clamp(0., 1.))
}

fn hsl(parts: &[&str]) -> Option<Rgba> {
	let hue = hue(parts[0])? / 360.;
	let saturation = percent(parts[1])?;
	let lightness = percent(parts[2])?;
	let high = if lightness < 0.5 {
		lightness * (1. + saturation)
	} else {
		lightness + saturation - lightness * saturation
	};
	let low = 2. * lightness - high;
	let channel = |offset: f32| -> u8 {
		let mut t = hue + offset;
		if t < 0. {
			t += 1.;
		}
		if t > 1. {
			t -= 1.;
		}
		let value = if t < 1. / 6. {
			low + (high - low) * 6. * t
		} else if t < 0.5 {
			high
		} else if t < 2. / 3. {
			low + (high - low) * (2. / 3. - t) * 6.
		} else {
			low
		};
		(value * 255.).round().clamp(0., 255.) as u8
	};
	Some(Rgba {
		red: channel(1. / 3.),
		green: channel(0.),
		blue: channel(-1. / 3.),
		alpha: alpha(parts)?,
	})
}
