use platform::push_to_talk::{
	Binding, Health, Input, MOUSE_4, MOUSE_5, Matcher, Modifiers, Verdict, verdict,
};

const KEY_V: u16 = 0x56;
const KEY_B: u16 = 0x42;
const LEFT_CONTROL: u16 = 0xa2;
const RIGHT_CONTROL: u16 = 0xa3;
const LEFT_SHIFT: u16 = 0xa0;
const F13: u16 = 0x7c;

fn down(key: u16) -> Input {
	Input {
		key,
		down: true,
		injected: false,
	}
}

fn up(key: u16) -> Input {
	Input {
		key,
		down: false,
		injected: false,
	}
}

fn injected(input: Input) -> Input {
	Input {
		injected: true,
		..input
	}
}

fn matcher(binding: &str) -> Matcher {
	Matcher::new(binding.parse().expect("the binding parses"))
}

fn stream(matcher: &mut Matcher, inputs: &[Input]) -> Vec<bool> {
	inputs.iter().filter_map(|input| matcher.feed(*input)).collect()
}

#[test]
fn bindings_from_the_recorder_parse_to_virtual_keys() {
	assert_eq!(
		"ctrl+shift+KeyV".parse(),
		Ok(Binding {
			modifiers: Modifiers {
				ctrl: true,
				shift: true,
				..Modifiers::default()
			},
			key: KEY_V,
		})
	);
	assert_eq!(
		"Mouse4".parse::<Binding>().map(|binding| binding.key),
		Ok(MOUSE_4)
	);
	assert_eq!(
		"alt+Mouse5"
			.parse::<Binding>()
			.map(|binding| (binding.key, binding.modifiers.alt)),
		Ok((MOUSE_5, true))
	);
	assert_eq!("F13".parse::<Binding>().map(|binding| binding.key), Ok(F13));
	assert_eq!("Digit7".parse::<Binding>().map(|binding| binding.key), Ok(0x37));
	assert_eq!("Numpad3".parse::<Binding>().map(|binding| binding.key), Ok(0x63));
	assert_eq!(
		"Backquote".parse::<Binding>().map(|binding| binding.key),
		Ok(0xc0)
	);
	for broken in ["", "ctrl", "ctrl+", "KeyV+KeyB", "F25", "KeyVV", "Mouse9"] {
		assert!(broken.parse::<Binding>().is_err(), "{broken:?} is refused");
	}
}

#[test]
fn a_key_reports_press_and_release_and_repeat_is_ignored() {
	let mut talk = matcher("KeyV");
	assert_eq!(
		stream(
			&mut talk,
			&[down(KEY_V), down(KEY_V), down(KEY_V), up(KEY_V), up(KEY_V)]
		),
		[true, false]
	);
	assert_eq!(
		stream(&mut talk, &[down(KEY_B), up(KEY_B)]),
		Vec::<bool>::new(),
		"other keys pass by"
	);
}

#[test]
fn a_combination_needs_every_modifier_and_releasing_any_key_releases() {
	let mut talk = matcher("ctrl+shift+KeyV");
	assert_eq!(
		stream(&mut talk, &[down(KEY_V), up(KEY_V)]),
		Vec::<bool>::new(),
		"the key alone does nothing"
	);
	assert_eq!(
		stream(
			&mut talk,
			&[
				down(LEFT_CONTROL),
				down(LEFT_SHIFT),
				down(KEY_V),
				down(KEY_V),
				up(LEFT_SHIFT)
			]
		),
		[true, false],
		"releasing a modifier releases"
	);
	assert_eq!(
		stream(
			&mut talk,
			&[down(LEFT_SHIFT), down(RIGHT_CONTROL), up(LEFT_CONTROL), up(KEY_V)]
		),
		[true, false],
		"either control counts and releasing the key releases"
	);
}

#[test]
fn extra_keys_held_with_the_binding_still_talk() {
	let mut talk = matcher("KeyV");
	assert_eq!(
		stream(
			&mut talk,
			&[down(LEFT_SHIFT), down(KEY_B), down(KEY_V), up(KEY_B), up(KEY_V)]
		),
		[true, false]
	);
}

#[test]
fn side_buttons_talk_through_the_mouse_events() {
	let mut talk = matcher("Mouse4");
	assert_eq!(
		stream(
			&mut talk,
			&[down(MOUSE_5), down(MOUSE_4), up(MOUSE_5), up(MOUSE_4)]
		),
		[true, false]
	);
}

#[test]
fn injected_input_is_ignored() {
	let mut talk = matcher("KeyV");
	assert_eq!(
		stream(&mut talk, &[injected(down(KEY_V)), injected(up(KEY_V))]),
		Vec::<bool>::new()
	);
	assert_eq!(
		stream(&mut talk, &[down(KEY_V), injected(up(KEY_V))]),
		[true],
		"an injected release does not end a real press"
	);
}

#[test]
fn losing_the_hook_releases_and_a_resync_keeps_only_the_modifiers_held() {
	let mut talk = matcher("ctrl+KeyV");
	assert_eq!(stream(&mut talk, &[down(LEFT_CONTROL), down(KEY_V)]), [true]);
	assert_eq!(talk.release(), Some(false));
	assert_eq!(talk.release(), None, "a second release is quiet");

	assert_eq!(stream(&mut talk, &[down(LEFT_CONTROL), down(KEY_V)]), [true]);
	assert_eq!(
		talk.resync(|key| key == LEFT_CONTROL || key == KEY_V),
		Some(false),
		"a resync turns push to talk off"
	);
	assert_eq!(
		stream(&mut talk, &[up(KEY_V), down(KEY_V)]),
		[true],
		"control was rebuilt from the key state, so the next press talks"
	);
}

#[test]
fn the_watchdog_releases_a_stuck_key_and_reinstalls_quiet_hooks() {
	let fine = Health {
		active: true,
		physically_held: true,
		input_after_last_event_ms: 0,
		installed_ms: 120_000,
	};
	assert_eq!(verdict(fine), Verdict::Fine, "a held key is never cut short");
	assert_eq!(
		verdict(Health {
			physically_held: false,
			..fine
		}),
		Verdict::Stuck
	);
	assert_eq!(
		verdict(Health {
			input_after_last_event_ms: 5_000,
			..fine
		}),
		Verdict::Lost
	);
	assert_eq!(
		verdict(Health {
			active: false,
			physically_held: false,
			..fine
		}),
		Verdict::Refresh
	);
	assert_eq!(
		verdict(Health {
			active: false,
			physically_held: false,
			installed_ms: 1_000,
			..fine
		}),
		Verdict::Fine
	);
}
