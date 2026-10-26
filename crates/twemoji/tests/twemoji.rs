use protocol::expressions::unicode::unicode_emoji_list;
use twemoji::{Piece, file, split, svg};

fn whole(text: &str) -> Option<&'static str> {
	let pieces: Vec<Piece<'_>> = split(text).collect();
	match pieces.as_slice() {
		[Piece::Emoji { text: matched, file }] if *matched == text => Some(file),
		_ => None,
	}
}

const NEWER_THAN_THE_RELEASE: [&str; 9] = [
	"cracking_face",
	"leftwards_thumb_sign",
	"rightwards_thumb_sign",
	"monarch_butterfly",
	"pickle",
	"lighthouse",
	"meteor",
	"eraser",
	"net_with_handle",
];

#[test]
fn every_emoji_in_the_gemoji_table_resolves_with_every_skin_tone() {
	let mut missing = Vec::new();
	let mut checked = 0;
	for emoji in unicode_emoji_list() {
		let tones = if emoji.skin_tone_capable {
			bridge::TONES.len()
		} else {
			1
		};
		for tone in 0..tones {
			let text = bridge::toned(emoji.emoji, tone);
			checked += 1;
			let resolved = file(&text);
			if resolved.is_none() || whole(&text) != resolved || resolved.and_then(svg).is_none() {
				missing.push(emoji.shortcode);
			}
		}
	}
	missing.dedup();
	assert!(checked > 3400, "only {checked} sequences were walked");
	assert_eq!(missing, NEWER_THAN_THE_RELEASE, "of {checked} sequences");
}

#[test]
fn sequences_resolve_to_twemoji_file_names() {
	let cases = [
		("\u{1F600}", "1f600"),
		("\u{1F44B}\u{1F3FD}", "1f44b-1f3fd"),
		(
			"\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
			"1f468-200d-1f469-200d-1f467-200d-1f466",
		),
		(
			"\u{1F9D1}\u{1F3FB}\u{200D}\u{1F91D}\u{200D}\u{1F9D1}\u{1F3FC}",
			"1f9d1-1f3fb-200d-1f91d-200d-1f9d1-1f3fc",
		),
		("\u{1F1FA}\u{1F1F8}", "1f1fa-1f1f8"),
		(
			"\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
			"1f3f4-e0067-e0062-e0065-e006e-e0067-e007f",
		),
		("#\u{FE0F}\u{20E3}", "23-20e3"),
		("1\u{FE0F}\u{20E3}", "31-20e3"),
		("1\u{20E3}", "31-20e3"),
		("\u{2764}\u{FE0F}", "2764"),
		("\u{2764}", "2764"),
		("\u{2764}\u{FE0F}\u{200D}\u{1F525}", "2764-fe0f-200d-1f525"),
		("\u{1F441}\u{FE0F}\u{200D}\u{1F5E8}\u{FE0F}", "1f441-200d-1f5e8"),
		("\u{A9}\u{FE0F}", "a9"),
	];
	for (text, expected) in cases {
		assert_eq!(file(text), Some(expected), "{text:?}");
		assert_eq!(whole(text), Some(expected), "{text:?} split");
	}
}

#[test]
fn text_around_emoji_stays_text() {
	let pieces: Vec<Piece<'_>> =
		split("hi \u{1F44B}\u{1F3FD} there \u{A9} 1 # \u{1F1FA}\u{1F1F8}!").collect();
	assert_eq!(
		pieces,
		vec![
			Piece::Text("hi "),
			Piece::Emoji {
				text: "\u{1F44B}\u{1F3FD}",
				file: "1f44b-1f3fd"
			},
			Piece::Text(" there \u{A9} 1 # "),
			Piece::Emoji {
				text: "\u{1F1FA}\u{1F1F8}",
				file: "1f1fa-1f1f8"
			},
			Piece::Text("!"),
		]
	);
	let text_presentation: Vec<Piece<'_>> = split("\u{2764}\u{FE0E}").collect();
	assert_eq!(text_presentation, vec![Piece::Text("\u{2764}\u{FE0E}")]);
	let plain: Vec<Piece<'_>> = split("plain words").collect();
	assert_eq!(plain, vec![Piece::Text("plain words")]);
}

#[test]
fn a_family_the_set_lacks_falls_back_to_its_people() {
	let text = "\u{1F468}\u{200D}\u{1F47D}";
	let pieces: Vec<Piece<'_>> = split(text).collect();
	assert_eq!(
		pieces,
		vec![
			Piece::Emoji {
				text: "\u{1F468}",
				file: "1f468"
			},
			Piece::Text("\u{200D}"),
			Piece::Emoji {
				text: "\u{1F47D}",
				file: "1f47d"
			},
		]
	);
}

#[test]
fn the_pack_holds_the_pinned_release() {
	let stats = twemoji::stats();
	assert_eq!(stats.files, 4009);
	assert!(stats.packed_bytes < 1_500_000, "{stats:?}");
	assert!(svg("1f600").is_some_and(|bytes| bytes.starts_with(b"<svg")));
	assert_eq!(twemoji::names().count(), stats.files);
}
