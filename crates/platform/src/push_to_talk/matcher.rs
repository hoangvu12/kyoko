use std::str::FromStr;

pub const MOUSE_4: u16 = 0x05;
pub const MOUSE_5: u16 = 0x06;
const SHIFT: [u16; 3] = [0x10, 0xa0, 0xa1];
const CONTROL: [u16; 3] = [0x11, 0xa2, 0xa3];
const ALT: [u16; 3] = [0x12, 0xa4, 0xa5];
const META: [u16; 2] = [0x5b, 0x5c];
const SIDED_MODIFIERS: [u16; 8] = [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x5b, 0x5c];
const KEYS: usize = 256;
pub const LOST_AFTER_MS: u32 = 1_000;
pub const REFRESH_AFTER_MS: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
	pub ctrl: bool,
	pub alt: bool,
	pub shift: bool,
	pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
	pub modifiers: Modifiers,
	pub key: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBinding;

impl FromStr for Binding {
	type Err = InvalidBinding;

	fn from_str(text: &str) -> Result<Binding, InvalidBinding> {
		let mut modifiers = Modifiers::default();
		let mut key = None;
		for part in text.split('+').map(str::trim) {
			let modifier = match part.to_ascii_lowercase().as_str() {
				"ctrl" | "control" => &mut modifiers.ctrl,
				"alt" | "option" => &mut modifiers.alt,
				"shift" => &mut modifiers.shift,
				"super" | "meta" | "win" | "cmd" | "command" => &mut modifiers.meta,
				_ => {
					if key.replace(virtual_key(part).ok_or(InvalidBinding)?).is_some() {
						return Err(InvalidBinding);
					}
					continue;
				}
			};
			*modifier = true;
		}
		key.map(|key| Binding { modifiers, key }).ok_or(InvalidBinding)
	}
}

impl Binding {
	pub fn keys(&self) -> Vec<u16> {
		let wanted = [
			(self.modifiers.ctrl, CONTROL[0]),
			(self.modifiers.alt, ALT[0]),
			(self.modifiers.shift, SHIFT[0]),
			(self.modifiers.meta, META[0]),
		];
		wanted
			.into_iter()
			.filter_map(|(wanted, key)| wanted.then_some(key))
			.chain([self.key])
			.collect()
	}
}

pub fn virtual_key(code: &str) -> Option<u16> {
	let named = match code {
		"Mouse4" => MOUSE_4,
		"Mouse5" => MOUSE_5,
		"Backspace" => 0x08,
		"Tab" => 0x09,
		"Enter" | "NumpadEnter" => 0x0d,
		"Pause" => 0x13,
		"CapsLock" => 0x14,
		"Escape" => 0x1b,
		"Space" | " " => 0x20,
		"PageUp" => 0x21,
		"PageDown" => 0x22,
		"End" => 0x23,
		"Home" => 0x24,
		"ArrowLeft" => 0x25,
		"ArrowUp" => 0x26,
		"ArrowRight" => 0x27,
		"ArrowDown" => 0x28,
		"PrintScreen" => 0x2c,
		"Insert" => 0x2d,
		"Delete" => 0x2e,
		"ContextMenu" => 0x5d,
		"NumpadMultiply" => 0x6a,
		"NumpadAdd" => 0x6b,
		"NumpadSubtract" => 0x6d,
		"NumpadDecimal" => 0x6e,
		"NumpadDivide" => 0x6f,
		"NumLock" => 0x90,
		"ScrollLock" => 0x91,
		"ShiftLeft" => SHIFT[1],
		"ShiftRight" => SHIFT[2],
		"ControlLeft" => CONTROL[1],
		"ControlRight" => CONTROL[2],
		"AltLeft" => ALT[1],
		"AltRight" => ALT[2],
		"MetaLeft" => META[0],
		"MetaRight" => META[1],
		"Semicolon" | ";" => 0xba,
		"Equal" | "=" => 0xbb,
		"Comma" | "," => 0xbc,
		"Minus" | "-" => 0xbd,
		"Period" | "." => 0xbe,
		"Slash" | "/" => 0xbf,
		"Backquote" | "`" => 0xc0,
		"BracketLeft" | "[" => 0xdb,
		"Backslash" | "\\" => 0xdc,
		"BracketRight" | "]" => 0xdd,
		"Quote" | "'" => 0xde,
		"IntlBackslash" => 0xe2,
		_ => return numbered(code),
	};
	Some(named)
}

fn numbered(code: &str) -> Option<u16> {
	let letters = u16::from(b'A')..=u16::from(b'Z');
	let digits = u16::from(b'0')..=u16::from(b'9');
	if let Some(letter) = code.strip_prefix("Key") {
		return single(letter).filter(|key| letters.contains(key));
	}
	if let Some(digit) = code.strip_prefix("Digit") {
		return single(digit).filter(|key| digits.contains(key));
	}
	if let Some(digit) = code.strip_prefix("Numpad") {
		return single(digit)
			.filter(|key| digits.contains(key))
			.map(|key| key - u16::from(b'0') + 0x60);
	}
	if let Some(number) = code
		.strip_prefix('F')
		.and_then(|number| number.parse::<u16>().ok())
	{
		return (1..=24).contains(&number).then(|| 0x6f + number);
	}
	single(code)
}

fn single(text: &str) -> Option<u16> {
	let mut chars = text.chars();
	match (chars.next(), chars.next()) {
		(Some(character), None) if character.is_ascii_alphanumeric() => {
			Some(character.to_ascii_uppercase() as u16)
		}
		_ => None,
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
	pub key: u16,
	pub down: bool,
	pub injected: bool,
}

pub struct Matcher {
	binding: Binding,
	held: [bool; KEYS],
	active: bool,
}

impl Matcher {
	pub fn new(binding: Binding) -> Matcher {
		Matcher {
			binding,
			held: [false; KEYS],
			active: false,
		}
	}

	pub fn binding(&self) -> Binding {
		self.binding
	}

	pub fn active(&self) -> bool {
		self.active
	}

	pub fn feed(&mut self, input: Input) -> Option<bool> {
		if input.injected {
			return None;
		}
		let slot = self.held.get_mut(usize::from(input.key))?;
		if *slot == input.down {
			return None;
		}
		*slot = input.down;
		self.settle()
	}

	pub fn release(&mut self) -> Option<bool> {
		self.held = [false; KEYS];
		self.settle()
	}

	pub fn resync(&mut self, pressed: impl Fn(u16) -> bool) -> Option<bool> {
		let released = self.release();
		for key in SIDED_MODIFIERS {
			self.held[usize::from(key)] = pressed(key);
		}
		released
	}

	fn settle(&mut self) -> Option<bool> {
		let now = self.combination_held();
		(now != self.active).then(|| {
			self.active = now;
			now
		})
	}

	fn combination_held(&self) -> bool {
		let any = |keys: &[u16]| keys.iter().any(|key| self.held[usize::from(*key)]);
		let modifiers = self.binding.modifiers;
		self.held[usize::from(self.binding.key)]
			&& (!modifiers.ctrl || any(&CONTROL))
			&& (!modifiers.alt || any(&ALT))
			&& (!modifiers.shift || any(&SHIFT))
			&& (!modifiers.meta || any(&META))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
	pub active: bool,
	pub physically_held: bool,
	pub input_after_last_event_ms: u32,
	pub installed_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	Fine,
	Stuck,
	Lost,
	Refresh,
}

pub fn verdict(health: Health) -> Verdict {
	if health.active && !health.physically_held {
		Verdict::Stuck
	} else if health.input_after_last_event_ms > LOST_AFTER_MS {
		Verdict::Lost
	} else if !health.active && health.installed_ms > REFRESH_AFTER_MS {
		Verdict::Refresh
	} else {
		Verdict::Fine
	}
}
