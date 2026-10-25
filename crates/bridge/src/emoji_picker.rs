use protocol::expressions::unicode::search_unicode;
use protocol::{EmojiCategory, Snowflake, UnicodeEmoji};
use state::{CatalogEmoji, EmojiRef, State};

pub const SEARCH_LIMIT: usize = 160;
pub const TONES: [&str; 6] = [
	"",
	"\u{1F3FB}",
	"\u{1F3FC}",
	"\u{1F3FD}",
	"\u{1F3FE}",
	"\u{1F3FF}",
];
pub const TONE_SAMPLE: &str = "\u{1F44B}";
const SECTION_GLYPHS: [&str; 8] = [
	"\u{1F600}",
	"\u{1F43B}",
	"\u{1F354}",
	"\u{26BD}",
	"\u{1F697}",
	"\u{1F4A1}",
	"\u{2764}",
	"\u{1F3C1}",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerSection {
	Recent,
	Guild(Snowflake),
	Unicode(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailIcon {
	Recent,
	Guild {
		guild_id: Snowflake,
		icon: Option<String>,
	},
	Glyph(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailEntry {
	pub section: PickerSection,
	pub title: String,
	pub icon: RailIcon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEmoji {
	pub id: Snowflake,
	pub animated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiCell {
	pub key: String,
	pub reaction: String,
	pub markup: String,
	pub glyph: Option<String>,
	pub custom: Option<CustomEmoji>,
	pub label: String,
	pub detail: Option<String>,
	pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildSection {
	guild_id: Snowflake,
	name: String,
	icon: Option<String>,
	emojis: Vec<CatalogEmoji>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmojiPicker {
	recent: Vec<EmojiRef>,
	guilds: Vec<GuildSection>,
	unicode: Vec<(EmojiCategory, Vec<&'static UnicodeEmoji>)>,
}

pub fn category_title(category: EmojiCategory) -> &'static str {
	match category {
		EmojiCategory::People => "Smileys & People",
		EmojiCategory::Nature => "Animals & Nature",
		EmojiCategory::Food => "Food & Drink",
		EmojiCategory::Activities => "Activities",
		EmojiCategory::Travel => "Travel & Places",
		EmojiCategory::Objects => "Objects",
		EmojiCategory::Symbols => "Symbols",
		EmojiCategory::Flags => "Flags",
	}
}

pub fn toned(glyph: &str, tone: usize) -> String {
	let Some(modifier) = TONES.get(tone).filter(|modifier| !modifier.is_empty()) else {
		return glyph.to_owned();
	};
	let mut chars = glyph.chars();
	let Some(first) = chars.next() else {
		return String::new();
	};
	let rest: String = chars.collect();
	let mut out = String::with_capacity(glyph.len() + modifier.len());
	out.push(first);
	out.push_str(modifier);
	out.push_str(rest.strip_prefix('\u{FE0F}').unwrap_or(&rest));
	out
}

pub fn custom_markup(name: &str, id: Snowflake, animated: bool) -> String {
	let prefix = if animated { "a" } else { "" };
	format!("<{prefix}:{name}:{id}>")
}

fn custom_cell(emoji: &CatalogEmoji, guild_name: &str) -> EmojiCell {
	EmojiCell {
		key: format!("custom:{}", emoji.id),
		reaction: format!("{}:{}", emoji.name, emoji.id),
		markup: custom_markup(&emoji.name, emoji.id, emoji.animated),
		glyph: None,
		custom: Some(CustomEmoji {
			id: emoji.id,
			animated: emoji.animated,
		}),
		label: format!(":{}:", emoji.name),
		detail: (!guild_name.is_empty()).then(|| guild_name.to_owned()),
		locked: !emoji.usable,
	}
}

fn unicode_cell(glyph: &str, name: &str, toneable: bool, tone: usize) -> EmojiCell {
	let shown = if toneable {
		toned(glyph, tone)
	} else {
		glyph.to_owned()
	};
	EmojiCell {
		key: format!("unicode:{shown}"),
		reaction: shown.clone(),
		markup: shown.clone(),
		glyph: Some(shown),
		custom: None,
		label: format!(":{name}:"),
		detail: None,
		locked: false,
	}
}

impl EmojiPicker {
	pub fn of(state: &State, guild_id: Option<Snowflake>) -> Self {
		let catalog = state.emoji_catalog(guild_id);
		Self {
			recent: state.recent_emoji().to_vec(),
			guilds: catalog
				.guilds
				.into_iter()
				.map(|section| GuildSection {
					guild_id: section.guild_id,
					name: section.name,
					icon: section.icon,
					emojis: section.emojis,
				})
				.collect(),
			unicode: catalog
				.unicode
				.iter()
				.map(|section| (section.category, section.emojis.clone()))
				.collect(),
		}
	}

	pub fn rail(&self) -> Vec<RailEntry> {
		let mut rail = Vec::new();
		if !self.recent.is_empty() {
			rail.push(RailEntry {
				section: PickerSection::Recent,
				title: "Frequently Used".to_owned(),
				icon: RailIcon::Recent,
			});
		}
		rail.extend(self.guilds.iter().map(|guild| RailEntry {
			section: PickerSection::Guild(guild.guild_id),
			title: guild.name.clone(),
			icon: RailIcon::Guild {
				guild_id: guild.guild_id,
				icon: guild.icon.clone(),
			},
		}));
		rail.extend(self.unicode.iter().enumerate().map(|(index, (category, _))| {
			RailEntry {
				section: PickerSection::Unicode(index),
				title: category_title(*category).to_owned(),
				icon: RailIcon::Glyph(
					SECTION_GLYPHS[EmojiCategory::ALL
						.iter()
						.position(|known| known == category)
						.unwrap_or(0)],
				),
			}
		}));
		rail
	}

	pub fn default_section(&self) -> PickerSection {
		if !self.recent.is_empty() {
			PickerSection::Recent
		} else if let Some(first) = self.guilds.first() {
			PickerSection::Guild(first.guild_id)
		} else {
			PickerSection::Unicode(0)
		}
	}

	pub fn section(&self, section: PickerSection, tone: usize) -> (String, Vec<EmojiCell>) {
		match section {
			PickerSection::Recent => ("Frequently Used".to_owned(), self.recent_cells()),
			PickerSection::Guild(guild_id) => {
				let Some(guild) = self.guilds.iter().find(|guild| guild.guild_id == guild_id) else {
					return (String::new(), Vec::new());
				};
				let cells = guild
					.emojis
					.iter()
					.map(|emoji| custom_cell(emoji, &guild.name))
					.collect();
				(guild.name.clone(), cells)
			}
			PickerSection::Unicode(index) => {
				let Some((category, emojis)) = self.unicode.get(index) else {
					return (String::new(), Vec::new());
				};
				let cells = emojis
					.iter()
					.map(|emoji| unicode_cell(emoji.emoji, emoji.shortcode, emoji.skin_tone_capable, tone))
					.collect();
				(category_title(*category).to_owned(), cells)
			}
		}
	}

	pub fn search(&self, query: &str, tone: usize) -> Vec<EmojiCell> {
		let needle = query.trim().to_lowercase();
		if needle.is_empty() {
			return Vec::new();
		}
		let custom = self.guilds.iter().flat_map(|guild| {
			guild
				.emojis
				.iter()
				.filter(|emoji| emoji.name.to_lowercase().contains(&needle))
				.map(|emoji| custom_cell(emoji, &guild.name))
		});
		let unicode = search_unicode(&needle)
			.into_iter()
			.take(SEARCH_LIMIT)
			.map(|emoji| unicode_cell(emoji.emoji, emoji.shortcode, emoji.skin_tone_capable, tone));
		custom.chain(unicode).collect()
	}

	fn recent_cells(&self) -> Vec<EmojiCell> {
		self.recent
			.iter()
			.map(|recent| match recent.id {
				Some(id) => {
					let found = self.guilds.iter().find_map(|guild| {
						guild
							.emojis
							.iter()
							.find(|emoji| emoji.id == id)
							.map(|emoji| custom_cell(emoji, &guild.name))
					});
					found.unwrap_or_else(|| EmojiCell {
						key: format!("custom:{id}"),
						reaction: format!("{}:{id}", recent.name),
						markup: custom_markup(&recent.name, id, recent.animated),
						glyph: None,
						custom: Some(CustomEmoji {
							id,
							animated: recent.animated,
						}),
						label: format!(":{}:", recent.name),
						detail: None,
						locked: true,
					})
				}
				None => unicode_cell(&recent.name, &recent.name, false, 0),
			})
			.collect()
	}
}
