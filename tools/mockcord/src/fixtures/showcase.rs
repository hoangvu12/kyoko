use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use protocol::{
	Attachment, Channel, ChannelType, Embed, EmbedAuthor, EmbedField, EmbedFooter, EmbedKind, EmbedMedia,
	EmbedProvider, Emoji, LenientVec, Message, MessageFlags, MessageReference, MessageReferenceKind,
	MessageType, PartialEmoji, Poll, PollAnswer, PollAnswerCount, PollLayout, PollMedia, PollResults,
	Reaction, ReactionCountDetails, ReadState, ReadStateKind, Role, Snowflake, StickerFormat, StickerItem,
	User,
};
use serde_json::Value;

use crate::audio;
use crate::clip;
use crate::components;
use crate::ids::snowflake_at;
use crate::json::to_json;

pub const CHANNEL_NAME: &str = "showcase";
pub const POSITION: i64 = 100;
const START_BEFORE_ANCHOR_MINUTES: i64 = 180;
const STEP_SECONDS: i64 = 40;
const SEQUENCE: u64 = 7;
const MISSING_ORIGINAL: u64 = 4_242;
const STICKER_APNG: u64 = 816_087_792_291_282_944;
const STICKER_GIF: u64 = 1_166_440_446_734_381_076;
const STICKER_LOTTIE: u64 = 749_054_660_769_218_631;
const COMMAND_ID: u64 = 1_234_567_890_123_456_789;

pub const SECTIONS: [(&str, &str); 8] = [
	("timeline-markdown", "showcase: markdown"),
	("timeline-inline", "showcase: inline"),
	("timeline-embeds", "showcase: embeds"),
	("timeline-embeds-media", "showcase: embed media"),
	("timeline-attachments", "showcase: attachments"),
	("timeline-files", "showcase: files and stickers"),
	("timeline-replies", "showcase: replies, polls and reactions"),
	("timeline-system", "showcase: system messages"),
];

pub const IMAGE_GRID_SECTIONS: [(&str, &str); 4] = [
	("timeline-image-grid", "showcase: image grid"),
	("timeline-image-grid-three", "showcase: image grid of three"),
	("timeline-image-grid-four", "showcase: image grid of four"),
	("timeline-image-grid-ten", "showcase: image grid of ten"),
];
const WIDE_PAIR: [(i64, i64); 2] = [(1650, 144), (1650, 150)];
const MIXED_PAIR: [(i64, i64); 2] = [(1280, 720), (720, 1280)];
const MIXED_THREE: [(i64, i64); 3] = [(1920, 1080), (1080, 1920), (1000, 1000)];
const MIXED_FOUR: [(i64, i64); 4] = [(1600, 900), (900, 1600), (800, 800), (2400, 600)];
const MIXED_FIVE: [(i64, i64); 5] = [(1600, 900), (900, 1600), (800, 800), (2400, 600), (600, 2400)];
const TEN: [(i64, i64); 10] = [
	(1920, 1080),
	(1080, 1920),
	(1000, 1000),
	(1600, 900),
	(900, 1600),
	(2400, 600),
	(600, 2400),
	(1280, 720),
	(720, 1280),
	(800, 800),
];

pub const VIDEO_SECTIONS: [(&str, &str); 1] = [("timeline-video", "showcase: video")];
pub const VIDEO_CAPTION: &str = "a clip from today";
pub const VIDEO_EMBED_URL: &str = "https://clips.example.com/v/kyoko-showcase";

pub const COMPONENTS_SECTION: &str = "showcase: components";
pub const RELEASE_QUESTION: &str = "did the release post go out";
pub const REACTIONS_SECTION: &str = "showcase: reactions";
pub const VOICE_SECTION: &str = "showcase: voice messages";
pub const POLLS_SECTION: &str = "showcase: polls";
pub const ENDED_POLL_SECTION: &str = "showcase: ended poll";
pub const POLL_QUESTION: &str = "What should we play tonight?";
pub const ENDED_POLL_LEAD: &str = "vote before it closes";
const POLL_ANSWERS: [(&str, &str); 3] = [
	("\u{1f3af}", "Valorant"),
	("\u{26cf}\u{fe0f}", "Minecraft"),
	("\u{1f526}", "Lethal Company"),
];
const POLL_COUNTS: [i64; 3] = [7, 3, 2];
const VOTED_ANSWER: usize = 1;
pub const AUDIO_CAPTION: &str = "here is the loop as a file";
pub const REACTIONS_MESSAGE: &str =
	"midnight 2.1.1 is out: picker shadow fixed, reaction chips use the theme tokens now";
pub const TOOLTIP_EMOJI: &str = "\u{1f44d}";
pub const LISTED_EMOJI: &str = "\u{1f525}";

pub fn screen_section(screen: &str) -> Option<&'static str> {
	SECTIONS
		.iter()
		.chain(IMAGE_GRID_SECTIONS.iter())
		.chain(VIDEO_SECTIONS.iter())
		.find(|(name, _)| *name == screen)
		.map(|(_, section)| *section)
}

pub fn marker(section: &str) -> String {
	format!("-# {section}")
}

pub struct Showcase<'a> {
	pub channel_id: u64,
	pub guild_id: u64,
	pub parent: u64,
	pub anchor: DateTime<Utc>,
	pub people: &'a [&'a User],
	pub me: &'a User,
	pub emojis: &'a [Emoji],
	pub roles: &'a [Role],
	pub mention_channel: u64,
	pub thread: Option<&'a Channel>,
}

pub struct Built {
	pub channel: Channel,
	pub messages: BTreeMap<u64, Value>,
	pub read_state: ReadState,
}

struct Writer<'a> {
	spec: &'a Showcase<'a>,
	messages: BTreeMap<u64, Value>,
	index: i64,
	last_read: Option<u64>,
	ids: Vec<u64>,
}

impl Writer<'_> {
	fn time(&self) -> DateTime<Utc> {
		self.spec.anchor - Duration::minutes(START_BEFORE_ANCHOR_MINUTES)
			+ Duration::seconds(self.index * STEP_SECONDS)
	}

	fn person(&self, slot: usize) -> &User {
		self.spec.people[slot % self.spec.people.len()]
	}

	fn post(&mut self, slot: usize, edit: impl FnOnce(&mut Message)) -> u64 {
		let timestamp = self.time();
		let id = snowflake_at(timestamp.timestamp_millis(), SEQUENCE);
		let mut message = Message {
			id: Snowflake(id),
			channel_id: Snowflake(self.spec.channel_id),
			guild_id: Some(Snowflake(self.spec.guild_id)),
			author: self.person(slot).clone(),
			timestamp: Some(timestamp),
			..Message::default()
		};
		edit(&mut message);
		self.messages.insert(id, to_json(&message));
		self.ids.push(id);
		self.index += 1;
		id
	}

	fn say(&mut self, slot: usize, content: &str) -> u64 {
		let content = content.to_owned();
		self.post(slot, move |message| message.content = content)
	}

	fn section(&mut self, title: &str) -> u64 {
		self.index += 10;
		self.say(0, &marker(title))
	}

	fn components(&mut self, components: Vec<Value>, flags: MessageFlags) -> u64 {
		let id = self.post(0, |message| {
			message.author = crate::rest::mockbot();
			message.flags = flags;
		});
		if let Some(stored) = self.messages.get_mut(&id) {
			stored["components"] = Value::Array(components);
		}
		id
	}

	fn components_said(&mut self, content: &str, components: Vec<Value>) -> u64 {
		let id = self.components(components, MessageFlags::empty());
		if let Some(stored) = self.messages.get_mut(&id) {
			stored["content"] = Value::String(content.to_owned());
		}
		id
	}

	fn stored(&self, id: u64) -> Option<Message> {
		let mut value = self.messages.get(&id)?.clone();
		value["referenced_message"] = Value::Null;
		serde_json::from_value(value).ok()
	}
}

fn media(url: &str, width: i64, height: i64) -> EmbedMedia {
	EmbedMedia {
		url: Some(url.to_owned()),
		proxy_url: Some(url.replace(
			"https://",
			"https://images-ext-1.discordapp.net/external/showcase/https/",
		)),
		width: Some(width),
		height: Some(height),
		content_type: None,
		placeholder: None,
		flags: None,
	}
}

fn attachment(
	id: u64,
	channel: u64,
	name: &str,
	kind: &str,
	size: i64,
	dimensions: Option<(i64, i64)>,
) -> Attachment {
	let url = format!("https://cdn.discordapp.com/attachments/{channel}/{id}/{name}");
	Attachment {
		id: Snowflake(id),
		filename: name.to_owned(),
		content_type: Some(kind.to_owned()),
		size: Some(size),
		proxy_url: Some(url.replace("cdn.discordapp.com", "media.discordapp.net")),
		url: Some(url),
		width: dimensions.map(|(width, _)| width),
		height: dimensions.map(|(_, height)| height),
		flags: Some(0),
		..Attachment::default()
	}
}

fn images(base: u64, channel: u64, prefix: &str, shapes: &[(i64, i64)]) -> Vec<Attachment> {
	shapes
		.iter()
		.zip(1..)
		.map(|(shape, index)| {
			attachment(
				base + index,
				channel,
				&format!("{prefix}-{index}.png"),
				"image/png",
				64_000,
				Some(*shape),
			)
		})
		.collect()
}

fn voice_attachment(id: u64, channel: u64, name: &str, seconds: f64) -> Attachment {
	Attachment {
		duration_secs: Some(seconds),
		waveform: Some(audio::VOICE_WAVEFORM.to_owned()),
		..attachment(id, channel, name, audio::OGG, audio::size_of(name) as i64, None)
	}
}

fn reaction(name: &str, count: i64, me: bool) -> Reaction {
	custom_reaction(None, name, false, count, me)
}

fn custom_reaction(id: Option<Snowflake>, name: &str, animated: bool, count: i64, me: bool) -> Reaction {
	Reaction {
		count,
		count_details: Some(ReactionCountDetails {
			burst: 0,
			normal: count,
		}),
		me,
		me_burst: false,
		emoji: PartialEmoji {
			id,
			name: Some(name.to_owned()),
			animated,
		},
		burst_colors: LenientVec::default(),
	}
}

fn super_reaction(name: &str, count: i64, me: bool) -> Reaction {
	Reaction {
		count,
		count_details: Some(ReactionCountDetails {
			burst: count,
			normal: 0,
		}),
		me: false,
		me_burst: me,
		emoji: PartialEmoji {
			id: None,
			name: Some(name.to_owned()),
			animated: false,
		},
		burst_colors: LenientVec::default(),
	}
}

fn poll(
	question: &str,
	answers: &[&str],
	counts: &[(i64, bool)],
	multi: bool,
	finished: bool,
	expiry: DateTime<Utc>,
) -> Poll {
	Poll {
		question: PollMedia {
			text: Some(question.to_owned()),
			emoji: None,
		},
		answers: LenientVec::from(
			answers
				.iter()
				.enumerate()
				.map(|(index, text)| PollAnswer {
					answer_id: Some(index as i64 + 1),
					poll_media: PollMedia {
						text: Some((*text).to_owned()),
						emoji: None,
					},
				})
				.collect::<Vec<_>>(),
		),
		expiry: Some(expiry),
		allow_multiselect: multi,
		layout_type: Some(PollLayout::Default),
		results: Some(PollResults {
			is_finalized: finished,
			answer_counts: LenientVec::from(
				counts
					.iter()
					.enumerate()
					.map(|(index, (count, me_voted))| PollAnswerCount {
						id: index as i64 + 1,
						count: *count,
						me_voted: *me_voted,
					})
					.collect::<Vec<_>>(),
			),
		}),
	}
}

fn open_poll_expiry() -> DateTime<Utc> {
	let now = Utc::now().timestamp();
	let hour = Duration::hours(1).num_seconds();
	DateTime::from_timestamp(now - now % hour, 0).unwrap_or_default() + Duration::hours(24)
}

fn game_poll(voted: Option<usize>, finished: bool, expiry: DateTime<Utc>) -> Poll {
	Poll {
		question: PollMedia {
			text: Some(POLL_QUESTION.to_owned()),
			emoji: None,
		},
		answers: LenientVec::from(
			POLL_ANSWERS
				.iter()
				.enumerate()
				.map(|(index, (emoji, text))| PollAnswer {
					answer_id: Some(index as i64 + 1),
					poll_media: PollMedia {
						text: Some((*text).to_owned()),
						emoji: Some(PartialEmoji {
							id: None,
							name: Some((*emoji).to_owned()),
							animated: false,
						}),
					},
				})
				.collect::<Vec<_>>(),
		),
		expiry: Some(expiry),
		allow_multiselect: false,
		layout_type: Some(PollLayout::Default),
		results: Some(PollResults {
			is_finalized: finished,
			answer_counts: LenientVec::from(
				POLL_COUNTS
					.iter()
					.enumerate()
					.map(|(index, count)| PollAnswerCount {
						id: index as i64 + 1,
						count: *count,
						me_voted: voted == Some(index),
					})
					.collect::<Vec<_>>(),
			),
		}),
	}
}

fn emoji_tag(emoji: Option<&Emoji>) -> String {
	match emoji {
		Some(emoji) => format!(
			"<{}:{}:{}>",
			if emoji.animated == Some(true) { "a" } else { "" },
			emoji.name.as_deref().unwrap_or("emoji"),
			emoji.id.map(Snowflake::get).unwrap_or_default()
		),
		None => String::new(),
	}
}

pub fn build(spec: &Showcase) -> Built {
	let mut writer = Writer {
		spec,
		messages: BTreeMap::new(),
		index: 0,
		last_read: None,
		ids: Vec::new(),
	};
	let still = spec.emojis.iter().find(|emoji| emoji.animated != Some(true));
	let moving = spec.emojis.iter().find(|emoji| emoji.animated == Some(true));
	let channel = spec.channel_id;
	let unix = spec.anchor.timestamp();

	writer.section(IMAGE_GRID_SECTIONS[0].1);
	let grid = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.content = "task manager before and after".to_owned();
		message.attachments = LenientVec::from(images(grid, channel, "task-manager", &WIDE_PAIR));
	});
	writer.post(2, |message| {
		message.content = "a wide one and a tall one".to_owned();
		message.attachments = LenientVec::from(images(grid + 10, channel, "shapes", &MIXED_PAIR));
	});

	writer.section(IMAGE_GRID_SECTIONS[1].1);
	let grid = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.content = "three from the trip".to_owned();
		message.attachments = LenientVec::from(images(grid, channel, "trip", &MIXED_THREE));
	});
	writer.post(2, |message| {
		message.attachments = LenientVec::from(images(grid + 10, channel, "five", &MIXED_FIVE));
	});

	writer.section(IMAGE_GRID_SECTIONS[2].1);
	let grid = writer.time().timestamp_millis() as u64;
	writer.post(2, |message| {
		message.content = "four mixed shapes".to_owned();
		message.attachments = LenientVec::from(images(grid, channel, "four", &MIXED_FOUR));
	});

	writer.section(IMAGE_GRID_SECTIONS[3].1);
	let grid = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.content = "the whole album".to_owned();
		message.attachments = LenientVec::from(images(grid, channel, "album", &TEN));
	});

	writer.section(SECTIONS[0].1);
	writer.say(
		1,
		"this is **bold**, *italic*, __underlined__, ~~struck through~~, ***bold italic*** and `inline code`",
	);
	writer.say(
		2,
		"# Heading one\n## Heading two\n### Heading three\n-# subtext under the headings",
	);
	writer.say(1, "todo before friday:\n- fix the build\n- ship the release\n  - tag it\n  - write notes\n1. first\n2. second");
	writer.say(
		2,
		"> the best code is the code you never have to write\n> - someone wise\nagreed, deleting it now",
	);
	writer.say(1, "check this out:\n```rust\nfn main() {\n\tlet answer = 42; // the answer\n\tprintln!(\"{answer}\");\n}\n```");

	writer.section(SECTIONS[1].1);
	writer.say(
		2,
		"the ending is ||everyone survives|| and the sequel is ||already announced||",
	);
	writer.say(1, "docs at https://example.com/docs, a [masked link](https://example.com/masked) and <https://example.com/no-embed>");
	let role = spec.roles.get(2).map(|role| role.id.get()).unwrap_or_default();
	let target = writer.person(2).clone();
	writer.post(1, |message| {
		message.content = format!(
			"<@{}> and <@&{role}> please check <#{}>, @everyone and </deploy:{COMMAND_ID}>",
			target.id, spec.mention_channel
		);
		message.mentions = LenientVec::from(vec![target.clone()]);
		message.mention_roles = vec![Snowflake(role)];
		message.mention_everyone = true;
	});
	writer.say(
		2,
		&format!("meet at <t:{unix}:f>, that is <t:{unix}:R>, on <t:{unix}:D>"),
	);
	writer.say(
		1,
		&format!(
			"custom emoji {} and animated {} in a sentence",
			emoji_tag(still),
			emoji_tag(moving)
		),
	);
	writer.say(
		2,
		&format!("{} {} \u{1f389}", emoji_tag(still), emoji_tag(moving)),
	);
	writer.say(1, "ship it \u{1f680} \u{1f525} \u{2728} \u{1f44d}");

	writer.section(SECTIONS[2].1);
	let timestamp = writer.time();
	writer.post(1, |message| {
		message.content = "new release is out".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Rich),
			title: Some("Release 2.4.0".to_owned()),
			description: Some("Faster startup, fewer crashes, and a **brand new** settings page.".to_owned()),
			url: Some("https://example.com/releases/2.4.0".to_owned()),
			timestamp: Some(timestamp),
			color: Some(0x6666ff),
			footer: Some(EmbedFooter {
				text: "release bot".to_owned(),
				icon_url: None,
				proxy_icon_url: None,
			}),
			author: Some(EmbedAuthor {
				name: "Release Bot".to_owned(),
				url: Some("https://example.com".to_owned()),
				icon_url: Some("https://example.com/static/bot.png".to_owned()),
				proxy_icon_url: None,
			}),
			thumbnail: Some(media("https://example.com/static/logo.png", 256, 256)),
			fields: LenientVec::from(vec![
				EmbedField {
					name: "Version".to_owned(),
					value: "2.4.0".to_owned(),
					inline: true,
				},
				EmbedField {
					name: "Channel".to_owned(),
					value: "stable".to_owned(),
					inline: true,
				},
				EmbedField {
					name: "Changelog".to_owned(),
					value: "- startup is 40% faster\n- fixed the flicker".to_owned(),
					inline: false,
				},
			]),
			..Embed::default()
		}]);
	});
	writer.post(2, |message| {
		message.content = "https://example.com/articles/lenient-decoding".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Article),
			title: Some("Why lenient decoding saves your client".to_owned()),
			description: Some("A bad element should never take the whole payload down with it.".to_owned()),
			url: Some("https://example.com/articles/lenient-decoding".to_owned()),
			provider: Some(EmbedProvider {
				name: Some("Example Blog".to_owned()),
				url: None,
			}),
			thumbnail: Some(media("https://example.com/static/lenient.png", 600, 315)),
			..Embed::default()
		}]);
	});
	writer.post(1, |message| {
		message.content = "https://example.org/plain-link".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Link),
			title: Some("A plain link preview".to_owned()),
			url: Some("https://example.org/plain-link".to_owned()),
			description: Some("No image, no author, just a title and a line of text.".to_owned()),
			..Embed::default()
		}]);
	});

	writer.section(SECTIONS[3].1);
	writer.post(2, |message| {
		message.content = "https://www.youtube.com/watch?v=showcase".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Video),
			title: Some("A video to watch later".to_owned()),
			url: Some("https://www.youtube.com/watch?v=showcase".to_owned()),
			color: Some(0xFF0000),
			provider: Some(EmbedProvider {
				name: Some("YouTube".to_owned()),
				url: Some("https://www.youtube.com".to_owned()),
			}),
			thumbnail: Some(media(
				"https://i.ytimg.com/vi/showcase/maxresdefault.jpg",
				640,
				360,
			)),
			video: Some(media("https://www.youtube.com/embed/showcase", 640, 360)),
			..Embed::default()
		}]);
	});
	writer.post(1, |message| {
		message.content = "https://example.com/pictures/photo.png".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Image),
			url: Some("https://example.com/pictures/photo.png".to_owned()),
			thumbnail: Some(media("https://example.com/pictures/photo.png", 360, 180)),
			..Embed::default()
		}]);
	});
	writer.post(2, |message| {
		message.content = "https://tenor.com/view/showcase-gif-1".to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Gifv),
			url: Some("https://tenor.com/view/showcase-gif-1".to_owned()),
			thumbnail: Some(media(
				"https://media.tenor.com/sHoWcAsEgIfAAAAe/showcase.png",
				240,
				135,
			)),
			video: Some(media(
				"https://media.tenor.com/sHoWcAsEgIfAAAPo/showcase.mp4",
				240,
				135,
			)),
			..Embed::default()
		}]);
	});

	writer.section(VIDEO_SECTIONS[0].1);
	let clips = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.content = VIDEO_CAPTION.to_owned();
		message.attachments = LenientVec::from(vec![Attachment {
			duration_secs: Some(clip::CLIP_SECS),
			..attachment(
				clips + 1,
				channel,
				clip::CLIP_FILENAME,
				clip::MP4,
				clip::CLIP.len() as i64,
				Some((clip::CLIP_WIDTH, clip::CLIP_HEIGHT)),
			)
		}]);
	});
	writer.post(2, |message| {
		message.content = VIDEO_EMBED_URL.to_owned();
		message.embeds = LenientVec::from(vec![Embed {
			kind: Some(EmbedKind::Video),
			url: Some(VIDEO_EMBED_URL.to_owned()),
			description: Some("a short test clip, played inline".to_owned()),
			author: Some(EmbedAuthor {
				name: "kyoko clips (@kyoko.clips)".to_owned(),
				url: Some("https://clips.example.com/@kyoko.clips".to_owned()),
				..EmbedAuthor::default()
			}),
			thumbnail: Some(media(
				"https://clips.example.com/v/kyoko-showcase/cover.jpg",
				clip::CLIP_WIDTH,
				clip::CLIP_HEIGHT,
			)),
			video: Some(media(
				"https://clips.example.com/v/kyoko-showcase/video.mp4",
				clip::CLIP_WIDTH,
				clip::CLIP_HEIGHT,
			)),
			..Embed::default()
		}]);
	});

	writer.section(SECTIONS[4].1);
	let base = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.content = "screenshots from today".to_owned();
		message.attachments = LenientVec::from(vec![
			attachment(
				base + 1,
				channel,
				"wide.png",
				"image/png",
				48_000,
				Some((640, 360)),
			),
			attachment(
				base + 2,
				channel,
				"tall.png",
				"image/png",
				32_000,
				Some((300, 450)),
			),
		]);
	});
	writer.post(2, |message| {
		message.attachments = LenientVec::from(vec![attachment(
			base + 3,
			channel,
			"dance.gif",
			"image/gif",
			120_000,
			Some((240, 135)),
		)]);
	});

	writer.section(SECTIONS[5].1);
	writer.post(1, |message| {
		message.content = "the notes and the recording".to_owned();
		message.attachments = LenientVec::from(vec![
			attachment(
				base + 4,
				channel,
				"release-notes.pdf",
				"application/pdf",
				1_572_864,
				None,
			),
			attachment(
				base + 5,
				channel,
				"demo.mp4",
				"video/mp4",
				24_117_248,
				Some((1280, 720)),
			),
		]);
	});
	writer.post(2, |message| {
		message.sticker_items = LenientVec::from(vec![StickerItem {
			id: Snowflake(STICKER_APNG),
			name: "Wave".to_owned(),
			format_type: Some(StickerFormat::Apng),
		}]);
	});
	writer.post(1, |message| {
		message.sticker_items = LenientVec::from(vec![StickerItem {
			id: Snowflake(STICKER_GIF),
			name: "Dance".to_owned(),
			format_type: Some(StickerFormat::Gif),
		}]);
	});
	writer.post(2, |message| {
		message.content = "a lottie sticker plays too".to_owned();
		message.sticker_items = LenientVec::from(vec![StickerItem {
			id: Snowflake(STICKER_LOTTIE),
			name: "Lottie".to_owned(),
			format_type: Some(StickerFormat::Lottie),
		}]);
	});

	writer.section(SECTIONS[6].1);
	let quoted = writer.say(2, "can someone review the release notes?");
	let original = writer.stored(quoted);
	writer.post(1, |message| {
		message.kind = MessageType::Reply;
		message.content = "on it".to_owned();
		message.message_reference = Some(writer_reference(spec, quoted));
		message.referenced_message = original.map(Box::new);
	});
	writer.post(2, |message| {
		message.kind = MessageType::Reply;
		message.content = "replying to something that is gone".to_owned();
		message.message_reference = Some(writer_reference(spec, MISSING_ORIGINAL));
	});
	let expiry = spec.anchor + Duration::days(2);
	writer.post(1, |message| {
		message.poll = Some(poll(
			"What should we write the next tool in?",
			&["Rust", "Zig", "Go"],
			&[(12, true), (3, false), (1, false)],
			false,
			false,
			expiry,
		));
	});
	writer.post(2, |message| {
		message.content = "react if you agree".to_owned();
		let mut reactions = vec![reaction("\u{1f44d}", 12, true), reaction("\u{1f525}", 4, false)];
		if let Some(emoji) = still {
			reactions.push(custom_reaction(
				emoji.id,
				emoji.name.as_deref().unwrap_or("emoji"),
				false,
				7,
				true,
			));
		}
		if let Some(emoji) = moving {
			reactions.push(custom_reaction(
				emoji.id,
				emoji.name.as_deref().unwrap_or("emoji"),
				true,
				2,
				false,
			));
		}
		message.reactions = LenientVec::from(reactions);
	});
	let edited_at = writer.time() + Duration::minutes(3);
	writer.post(1, |message| {
		message.content = "this message was edited after the fact".to_owned();
		message.edited_timestamp = Some(edited_at);
	});
	writer.last_read = writer.ids.last().copied();

	writer.section(SECTIONS[7].1);
	writer.post(2, |message| message.kind = MessageType::UserJoin);
	writer.post(1, |message| message.kind = MessageType::PremiumGuildSubscription);
	writer.post(2, |message| {
		message.kind = MessageType::PremiumGuildSubscriptionTier2
	});
	writer.post(1, |message| {
		message.kind = MessageType::ChannelPinnedMessage;
		message.message_reference = Some(writer_reference(spec, quoted));
	});
	if let Some(thread) = spec.thread {
		writer.post(2, |message| {
			message.kind = MessageType::ThreadCreated;
			message.content = thread.name.clone().unwrap_or_default();
			message.message_reference = Some(MessageReference {
				kind: None,
				message_id: None,
				channel_id: Some(thread.id),
				guild_id: Some(Snowflake(spec.guild_id)),
				fail_if_not_exists: None,
			});
		});
	}
	writer.post(1, |message| {
		message.kind = MessageType::ChannelNameChange;
		message.content = CHANNEL_NAME.to_owned();
	});
	writer.say(2, "and that is every kind of message");

	writer.section(VOICE_SECTION);
	let base = writer.time().timestamp_millis() as u64;
	writer.post(1, |message| {
		message.flags = MessageFlags::IS_VOICE_MESSAGE;
		message.attachments = LenientVec::from(vec![voice_attachment(
			base + 1,
			channel,
			audio::VOICE_MESSAGE_FILENAME,
			audio::VOICE_MESSAGE_SECS,
		)]);
	});
	writer.post(2, |message| {
		message.flags = MessageFlags::IS_VOICE_MESSAGE;
		message.attachments = LenientVec::from(vec![voice_attachment(
			base + 2,
			channel,
			audio::VOICE_REPLY_FILENAME,
			audio::VOICE_REPLY_SECS,
		)]);
	});
	writer.post(0, |message| {
		message.content = AUDIO_CAPTION.to_owned();
		message.attachments = LenientVec::from(vec![attachment(
			base + 3,
			channel,
			audio::AUDIO_FILENAME,
			audio::MP3,
			audio::AUDIO_FILE_SIZE,
			None,
		)]);
	});

	writer.section(COMPONENTS_SECTION);
	writer.components_said(components::BUTTONS_PROMPT, components::buttons());
	writer.components_said(components::FRUIT_PROMPT, components::fruit_select("fruit", false));
	writer.components_said(
		components::FRUITS_PROMPT,
		components::fruit_select("fruits", true),
	);
	writer.say(1, "the select menus look right now");
	writer.say(2, RELEASE_QUESTION);
	writer.components(
		components::release(
			"https://example.com/static/refact0r.png",
			[
				"https://example.com/static/midnight-hero.png",
				"https://example.com/static/midnight-picker.png",
				"https://example.com/static/midnight-chips.png",
			],
		),
		MessageFlags::IS_COMPONENTS_V2,
	);
	writer.section(POLLS_SECTION);
	let open_until = open_poll_expiry();
	writer.post(1, |message| {
		message.poll = Some(game_poll(None, false, open_until));
	});
	writer.post(1, |message| {
		message.poll = Some(game_poll(Some(VOTED_ANSWER), false, open_until));
	});

	writer.section(ENDED_POLL_SECTION);
	writer.say(2, ENDED_POLL_LEAD);
	let closed_at = writer.time();
	let ended = writer.post(1, |message| {
		message.poll = Some(game_poll(None, true, closed_at));
	});
	let ended_message = writer.stored(ended);
	writer.post(1, |message| {
		message.kind = MessageType::PollResult;
		message.message_reference = Some(writer_reference(spec, ended));
		if let Some(poll) = ended_message.as_ref().and_then(|stored| stored.poll.as_ref()) {
			message.embeds = LenientVec::from(vec![crate::polls::result_embed(poll)]);
		}
	});

	writer.section(REACTIONS_SECTION);
	writer.post(1, |message| {
		message.content = "is the new picker build on canary yet".to_owned();
		message.reactions = LenientVec::from(vec![reaction("\u{1f440}", 2, false)]);
	});
	writer.say(2, "yes, since this morning");
	writer.post(0, |message| {
		message.content = REACTIONS_MESSAGE.to_owned();
		let mut reactions = vec![reaction(LISTED_EMOJI, 7, true), reaction(TOOLTIP_EMOJI, 4, false)];
		if let Some(emoji) = moving {
			reactions.push(custom_reaction(
				emoji.id,
				emoji.name.as_deref().unwrap_or("emoji"),
				true,
				3,
				false,
			));
		}
		reactions.push(super_reaction("\u{1f389}", 3, false));
		reactions.push(super_reaction("\u{2764}\u{fe0f}", 2, true));
		message.reactions = LenientVec::from(reactions);
	});
	writer.post(0, |message| {
		message.content = "changelog is in #announcements".to_owned();
		message.reactions = LenientVec::from(vec![
			reaction("\u{1f64f}", 2, false),
			reaction("\u{1f480}", 1, true),
		]);
	});
	writer.post(1, |message| {
		message.content = "the gradient border on super reactions is so pretty".to_owned();
		message.reactions = LenientVec::from(vec![
			super_reaction("\u{1f62d}", 1, false),
			reaction("\u{2764}\u{fe0f}", 1, false),
		]);
	});

	let last = writer.ids.last().copied();
	let read = writer.last_read.or(last);
	let mut channel = Channel {
		id: Snowflake(channel),
		kind: ChannelType::GuildText,
		guild_id: Some(Snowflake(spec.guild_id)),
		position: Some(POSITION),
		name: Some(CHANNEL_NAME.to_owned()),
		parent_id: Some(Snowflake(spec.parent)),
		nsfw: Some(false),
		topic: Some("every kind of message, for parity".to_owned()),
		rate_limit_per_user: Some(0),
		..Channel::default()
	};
	channel.last_message_id = last.map(Snowflake);
	channel.message_count = Some(writer.messages.len() as i64);
	channel.total_message_sent = channel.message_count;
	Built {
		channel,
		messages: writer.messages,
		read_state: ReadState {
			id: Snowflake(spec.channel_id),
			last_message_id: read.map(Snowflake),
			last_pin_timestamp: None,
			mention_count: Some(0),
			flags: Some(0),
			last_viewed: Some(4000),
			read_state_type: Some(ReadStateKind::Channel),
		},
	}
}

fn writer_reference(spec: &Showcase, id: u64) -> MessageReference {
	MessageReference {
		kind: Some(MessageReferenceKind::Default),
		message_id: Some(Snowflake(id)),
		channel_id: Some(Snowflake(spec.channel_id)),
		guild_id: Some(Snowflake(spec.guild_id)),
		fail_if_not_exists: None,
	}
}
