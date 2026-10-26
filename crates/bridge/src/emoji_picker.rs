use protocol::expressions::unicode::{by_glyph, by_shortcode, search_unicode};
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
pub const NITRO_LOCK: &str = "You need Nitro to use this emoji here";
pub const UNAVAILABLE_LOCK: &str = "This emoji isn't available right now";
pub const PERMISSION_LOCK: &str = "You can't use emoji from other servers here";
pub const FAVORITES_SECTION: &str = "Favorites";
pub const RECENT_SECTION: &str = "Frequently Used";
pub const SEARCH_SECTION: &str = "Search results";
const HANDSHAKE: char = '\u{1F91D}';
const PEOPLE: [char; 3] = ['\u{1F9D1}', '\u{1F468}', '\u{1F469}'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerSection {
	Top,
	Favorites,
	Recent,
	Guild(Snowflake),
	Unicode(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailIcon {
	Top,
	Favorites,
	Recent,
	Guild {
		guild_id: Snowflake,
		icon: Option<String>,
	},
	Category(EmojiCategory),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailGroup {
	Yours,
	Servers,
	Unicode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailEntry {
	pub section: PickerSection,
	pub title: String,
	pub icon: RailIcon,
	pub group: RailGroup,
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
	pub lock: Option<&'static str>,
	pub favorite_entry: String,
	pub favorite: bool,
}

impl EmojiCell {
	pub fn footer_note(&self) -> Option<String> {
		match (self.lock, &self.detail) {
			(Some(reason), _) => Some(reason.to_owned()),
			(None, Some(server)) => Some(format!("from {server}")),
			(None, None) => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiSection {
	pub section: PickerSection,
	pub title: String,
	pub icon: RailIcon,
	pub cells: Vec<EmojiCell>,
	pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuildSection {
	guild_id: Snowflake,
	name: String,
	icon: Option<String>,
	emojis: Vec<CatalogEmoji>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Favorite {
	stored: String,
	entry: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmojiPicker {
	guild_id: Option<Snowflake>,
	nitro: bool,
	top: Vec<Snowflake>,
	favorites: Vec<Favorite>,
	recent: Vec<EmojiRef>,
	guilds: Vec<GuildSection>,
	unicode: Vec<(EmojiCategory, Vec<&'static UnicodeEmoji>)>,
}

pub fn category_title(category: EmojiCategory) -> &'static str {
	match category {
		EmojiCategory::People => "People",
		EmojiCategory::Nature => "Nature",
		EmojiCategory::Food => "Food",
		EmojiCategory::Activities => "Activities",
		EmojiCategory::Travel => "Travel",
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
	let rest = rest.strip_prefix('\u{FE0F}').unwrap_or(&rest);
	let mut out = String::with_capacity(glyph.len() + modifier.len() * 2);
	out.push(first);
	out.push_str(modifier);
	if rest.contains(HANDSHAKE) {
		for c in rest.chars() {
			out.push(c);
			if PEOPLE.contains(&c) {
				out.push_str(modifier);
			}
		}
	} else {
		out.push_str(rest);
	}
	out
}

pub fn emoji_lock(emoji: &CatalogEmoji, nitro: bool) -> Option<&'static str> {
	if emoji.usable {
		None
	} else if !emoji.available {
		Some(UNAVAILABLE_LOCK)
	} else if !nitro {
		Some(NITRO_LOCK)
	} else {
		Some(PERMISSION_LOCK)
	}
}

pub fn custom_markup(name: &str, id: Snowflake, animated: bool) -> String {
	let prefix = if animated { "a" } else { "" };
	format!("<{prefix}:{name}:{id}>")
}

fn favorite_entry_of(stored: &str) -> String {
	if stored.parse::<u64>().is_ok() {
		return stored.to_owned();
	}
	by_shortcode(stored)
		.or_else(|| by_glyph(stored))
		.map_or_else(|| stored.to_owned(), |emoji| emoji.shortcode.to_owned())
}

impl EmojiPicker {
	pub fn of(state: &State, guild_id: Option<Snowflake>) -> Self {
		let catalog = state.emoji_catalog(guild_id);
		Self {
			guild_id,
			nitro: state.has_nitro(),
			top: guild_id
				.and_then(|guild_id| state.top_emojis(guild_id))
				.map(<[Snowflake]>::to_vec)
				.unwrap_or_default(),
			favorites: state
				.favorite_emojis()
				.iter()
				.map(|stored| Favorite {
					stored: stored.clone(),
					entry: favorite_entry_of(stored),
				})
				.collect(),
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

	fn lock_of(&self, emoji: &CatalogEmoji) -> Option<&'static str> {
		emoji_lock(emoji, self.nitro)
	}

	fn favorite_of(&self, entry: String) -> (String, bool) {
		match self.favorites.iter().find(|favorite| favorite.entry == entry) {
			Some(favorite) => (favorite.stored.clone(), true),
			None => (entry, false),
		}
	}

	fn custom_cell(&self, emoji: &CatalogEmoji, guild_name: &str) -> EmojiCell {
		let lock = self.lock_of(emoji);
		let (favorite_entry, favorite) = self.favorite_of(emoji.id.to_string());
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
			locked: lock.is_some(),
			lock,
			favorite_entry,
			favorite,
		}
	}

	fn unicode_cell(&self, glyph: &str, name: &str, toneable: bool, tone: usize) -> EmojiCell {
		let shown = if toneable {
			toned(glyph, tone)
		} else {
			glyph.to_owned()
		};
		let (favorite_entry, favorite) = self.favorite_of(name.to_owned());
		EmojiCell {
			key: format!("unicode:{shown}"),
			reaction: shown.clone(),
			markup: shown.clone(),
			glyph: Some(shown),
			custom: None,
			label: format!(":{name}:"),
			detail: None,
			locked: false,
			lock: None,
			favorite_entry,
			favorite,
		}
	}

	fn known_unicode(&self, emoji: &UnicodeEmoji, tone: usize) -> EmojiCell {
		self.unicode_cell(emoji.emoji, emoji.shortcode, emoji.skin_tone_capable, tone)
	}

	fn find_custom(&self, id: Snowflake) -> Option<EmojiCell> {
		self.guilds.iter().find_map(|guild| {
			guild
				.emojis
				.iter()
				.find(|emoji| emoji.id == id)
				.map(|emoji| self.custom_cell(emoji, &guild.name))
		})
	}

	fn unknown_custom(&self, name: &str, id: Snowflake, animated: bool) -> EmojiCell {
		let (favorite_entry, favorite) = self.favorite_of(id.to_string());
		EmojiCell {
			key: format!("custom:{id}"),
			reaction: format!("{name}:{id}"),
			markup: custom_markup(name, id, animated),
			glyph: None,
			custom: Some(CustomEmoji { id, animated }),
			label: format!(":{name}:"),
			detail: None,
			locked: true,
			lock: Some(UNAVAILABLE_LOCK),
			favorite_entry,
			favorite,
		}
	}

	fn current_guild(&self) -> Option<&GuildSection> {
		let guild_id = self.guild_id?;
		self.guilds.iter().find(|guild| guild.guild_id == guild_id)
	}

	fn top_cells(&self) -> Vec<EmojiCell> {
		let Some(guild) = self.current_guild() else {
			return Vec::new();
		};
		self.top
			.iter()
			.filter_map(|id| guild.emojis.iter().find(|emoji| emoji.id == *id))
			.map(|emoji| self.custom_cell(emoji, &guild.name))
			.collect()
	}

	fn favorite_cells(&self, tone: usize) -> Vec<EmojiCell> {
		self.favorites
			.iter()
			.filter_map(|favorite| match favorite.entry.parse::<u64>() {
				Ok(id) => self.find_custom(Snowflake(id)),
				Err(_) => by_shortcode(&favorite.entry).map(|emoji| self.known_unicode(emoji, tone)),
			})
			.collect()
	}

	fn recent_cells(&self, tone: usize) -> Vec<EmojiCell> {
		self.recent
			.iter()
			.map(|recent| match recent.id {
				Some(id) => self
					.find_custom(id)
					.unwrap_or_else(|| self.unknown_custom(&recent.name, id, recent.animated)),
				None => match by_glyph(&recent.name) {
					Some(emoji) => self.unicode_cell(&recent.name, emoji.shortcode, false, tone),
					None => self.unicode_cell(&recent.name, &recent.name, false, tone),
				},
			})
			.collect()
	}

	fn guild_cells(&self, guild: &GuildSection) -> Vec<EmojiCell> {
		guild
			.emojis
			.iter()
			.map(|emoji| self.custom_cell(emoji, &guild.name))
			.collect()
	}

	fn unicode_cells(&self, index: usize, tone: usize) -> Option<(EmojiCategory, Vec<EmojiCell>)> {
		let (category, emojis) = self.unicode.get(index)?;
		let cells = emojis
			.iter()
			.map(|emoji| self.known_unicode(emoji, tone))
			.collect();
		Some((*category, cells))
	}

	fn top_title(&self) -> String {
		match self.current_guild() {
			Some(guild) => format!("Top Emoji in {}", guild.name),
			None => "Top Emoji".to_owned(),
		}
	}

	pub fn sections(&self, tone: usize) -> Vec<EmojiSection> {
		let mut sections = Vec::new();
		let guild_icon = self.current_guild().map(|guild| RailIcon::Guild {
			guild_id: guild.guild_id,
			icon: guild.icon.clone(),
		});
		let fixed = [
			(
				PickerSection::Top,
				self.top_title(),
				guild_icon.unwrap_or(RailIcon::Top),
				self.top_cells(),
			),
			(
				PickerSection::Favorites,
				FAVORITES_SECTION.to_owned(),
				RailIcon::Favorites,
				self.favorite_cells(tone),
			),
			(
				PickerSection::Recent,
				RECENT_SECTION.to_owned(),
				RailIcon::Recent,
				self.recent_cells(tone),
			),
		];
		for (section, title, icon, cells) in fixed {
			sections.push(EmojiSection {
				section,
				title,
				icon,
				locked: false,
				cells,
			});
		}
		for guild in &self.guilds {
			let cells = self.guild_cells(guild);
			sections.push(EmojiSection {
				section: PickerSection::Guild(guild.guild_id),
				title: guild.name.clone(),
				icon: RailIcon::Guild {
					guild_id: guild.guild_id,
					icon: guild.icon.clone(),
				},
				locked: cells.iter().any(|cell| cell.locked),
				cells,
			});
		}
		for index in 0..self.unicode.len() {
			if let Some((category, cells)) = self.unicode_cells(index, tone) {
				sections.push(EmojiSection {
					section: PickerSection::Unicode(index),
					title: category_title(category).to_owned(),
					icon: RailIcon::Category(category),
					locked: false,
					cells,
				});
			}
		}
		sections.retain(|section| !section.cells.is_empty());
		sections
	}

	pub fn rail(&self) -> Vec<RailEntry> {
		self.sections(0)
			.into_iter()
			.map(|section| RailEntry {
				group: match section.section {
					PickerSection::Top | PickerSection::Favorites | PickerSection::Recent => RailGroup::Yours,
					PickerSection::Guild(_) => RailGroup::Servers,
					PickerSection::Unicode(_) => RailGroup::Unicode,
				},
				icon: match section.section {
					PickerSection::Top => RailIcon::Top,
					_ => section.icon,
				},
				section: section.section,
				title: section.title,
			})
			.collect()
	}

	pub fn default_section(&self) -> PickerSection {
		self.sections(0)
			.first()
			.map_or(PickerSection::Unicode(0), |section| section.section)
	}

	pub fn section(&self, section: PickerSection, tone: usize) -> (String, Vec<EmojiCell>) {
		self.sections(tone)
			.into_iter()
			.find(|shown| shown.section == section)
			.map(|shown| (shown.title, shown.cells))
			.unwrap_or_default()
	}

	pub fn search(&self, query: &str, tone: usize) -> Vec<EmojiCell> {
		let needle = query.trim().trim_matches(':').to_lowercase();
		if needle.is_empty() {
			return Vec::new();
		}
		let custom = self.guilds.iter().flat_map(|guild| {
			guild
				.emojis
				.iter()
				.filter(|emoji| emoji.name.to_lowercase().contains(&needle))
				.map(|emoji| self.custom_cell(emoji, &guild.name))
		});
		let unicode = search_unicode(&needle)
			.into_iter()
			.take(SEARCH_LIMIT)
			.map(|emoji| self.known_unicode(emoji, tone));
		custom.chain(unicode).collect()
	}
}
