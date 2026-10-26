use protocol::{Permissions, Snowflake, Sticker, StickerItem};
use session::Command;
use state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StickerSectionKey {
	Recent,
	Guild(Snowflake),
	Pack(Snowflake),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StickerSectionIcon {
	Recent,
	Guild {
		guild_id: Snowflake,
		icon: Option<String>,
	},
	Sticker(StickerItem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StickerCell {
	pub sticker: Sticker,
	pub locked: bool,
	pub lock: Option<&'static str>,
}

pub const STICKER_NITRO_LOCK: &str = "You need Nitro to use this sticker here";
pub const STICKER_UNAVAILABLE_LOCK: &str = "This sticker isn't available right now";
pub const STICKER_PERMISSION_LOCK: &str = "You can't use stickers from other servers here";

fn sticker_cell(sticker: &Sticker, usable: bool, nitro: bool) -> StickerCell {
	let lock = if sticker.available == Some(false) {
		Some(STICKER_UNAVAILABLE_LOCK)
	} else if usable {
		None
	} else if nitro {
		Some(STICKER_PERMISSION_LOCK)
	} else {
		Some(STICKER_NITRO_LOCK)
	};
	StickerCell {
		sticker: sticker.clone(),
		locked: lock.is_some(),
		lock,
	}
}

impl StickerCell {
	pub fn item(&self) -> StickerItem {
		sticker_item(&self.sticker)
	}

	pub fn tags(&self) -> String {
		self.sticker.tags.clone().unwrap_or_default()
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct StickerSection {
	pub key: StickerSectionKey,
	pub name: String,
	pub icon: StickerSectionIcon,
	pub stickers: Vec<StickerCell>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StickerPicker {
	sections: Vec<StickerSection>,
}

pub fn sticker_item(sticker: &Sticker) -> StickerItem {
	StickerItem {
		id: sticker.id,
		name: sticker.name.clone(),
		format_type: sticker.format_type,
	}
}

fn guild_sticker_usable(state: &State, owner: Snowflake, channel_id: Snowflake) -> bool {
	state.channel_guild(channel_id) == Some(owner)
		|| (state.has_nitro()
			&& state
				.my_permissions(channel_id)
				.contains(Permissions::USE_EXTERNAL_STICKERS))
}

impl StickerPicker {
	pub fn of(state: &State, guild_id: Option<Snowflake>, channel_id: Snowflake) -> Self {
		let nitro = state.has_nitro();
		let mut order: Vec<Snowflake> = guild_id.into_iter().collect();
		order.extend(
			state
				.guilds()
				.map(|guild| guild.id)
				.filter(|id| Some(*id) != guild_id),
		);
		let mut sections: Vec<StickerSection> = order
			.into_iter()
			.filter_map(|id| {
				let stickers = state.guild_stickers(id)?;
				if stickers.is_empty() {
					return None;
				}
				let guild = state.guild(id)?;
				let usable = guild_sticker_usable(state, id, channel_id);
				Some(StickerSection {
					key: StickerSectionKey::Guild(id),
					name: guild.name.clone(),
					icon: StickerSectionIcon::Guild {
						guild_id: id,
						icon: guild.icon.clone(),
					},
					stickers: stickers
						.iter()
						.map(|sticker| sticker_cell(sticker, usable, nitro))
						.collect(),
				})
			})
			.collect();
		for pack in state.sticker_packs().unwrap_or_default() {
			let cover = pack
				.cover_sticker_id
				.and_then(|id| pack.stickers.iter().find(|sticker| sticker.id == id))
				.or_else(|| pack.stickers.first());
			let Some(cover) = cover else {
				continue;
			};
			sections.push(StickerSection {
				key: StickerSectionKey::Pack(pack.id),
				name: pack.name.clone(),
				icon: StickerSectionIcon::Sticker(sticker_item(cover)),
				stickers: pack
					.stickers
					.iter()
					.map(|sticker| sticker_cell(sticker, true, nitro))
					.collect(),
			});
		}
		let recent: Vec<StickerCell> = state
			.recent_stickers()
			.iter()
			.filter_map(|used| {
				sections
					.iter()
					.flat_map(|section| section.stickers.iter())
					.find(|cell| cell.sticker.id == used.id)
					.cloned()
			})
			.collect();
		if !recent.is_empty() {
			sections.insert(
				0,
				StickerSection {
					key: StickerSectionKey::Recent,
					name: "Frequently Used".to_owned(),
					icon: StickerSectionIcon::Recent,
					stickers: recent,
				},
			);
		}
		Self { sections }
	}

	pub fn needs(state: &State, guild_id: Option<Snowflake>) -> Vec<Command> {
		let mut commands = Vec::new();
		if state.sticker_packs().is_none() {
			commands.push(Command::LoadStickerPacks);
		}
		if let Some(guild_id) = guild_id
			&& state.guild_stickers(guild_id).is_none()
		{
			commands.push(Command::LoadGuildStickers { guild_id });
		}
		commands
	}

	pub fn sections(&self) -> &[StickerSection] {
		&self.sections
	}

	pub fn default_section(&self) -> Option<StickerSectionKey> {
		self.sections.first().map(|section| section.key)
	}

	pub fn section(&self, key: Option<StickerSectionKey>) -> Option<&StickerSection> {
		let key = key
			.filter(|key| self.sections.iter().any(|section| section.key == *key))
			.or_else(|| self.default_section())?;
		self.sections.iter().find(|section| section.key == key)
	}

	pub fn search(&self, query: &str) -> Vec<StickerCell> {
		let needle = query.trim().to_lowercase();
		if needle.is_empty() {
			return Vec::new();
		}
		self.sections
			.iter()
			.flat_map(|section| section.stickers.iter())
			.filter(|cell| {
				cell.sticker.name.to_lowercase().contains(&needle)
					|| cell
						.sticker
						.tags
						.as_deref()
						.is_some_and(|tags| tags.to_lowercase().contains(&needle))
			})
			.cloned()
			.collect()
	}
}
