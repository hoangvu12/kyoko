use protocol::{Permissions, PremiumTier, Snowflake, StickerFormat};

use crate::state::State;

pub const EMOJI_NAME_MIN: usize = 2;
pub const EMOJI_NAME_MAX: usize = 32;
pub const EMOJI_MAX_BYTES: usize = 256 * 1024;
pub const STICKER_NAME_MIN: usize = 2;
pub const STICKER_NAME_MAX: usize = 30;
pub const STICKER_DESCRIPTION_MIN: usize = 2;
pub const STICKER_DESCRIPTION_MAX: usize = 100;
pub const STICKER_TAGS_MAX: usize = 200;
pub const STICKER_MAX_BYTES: usize = 512 * 1024;

const EMOJI_SLOTS: [usize; 4] = [50, 100, 150, 250];
const STICKER_SLOTS: [usize; 4] = [5, 15, 30, 60];
const MORE_EMOJI: &str = "MORE_EMOJI";
const MORE_EMOJI_SLOTS: usize = 200;
const MORE_STICKERS: &str = "MORE_STICKERS";
const MORE_STICKERS_SLOTS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
	Emoji {
		id: Snowflake,
		name: String,
		animated: bool,
	},
	Sticker {
		id: Snowflake,
		name: String,
		format: StickerFormat,
	},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionKind {
	Emoji,
	AnimatedEmoji,
	Sticker,
}

impl Expression {
	pub fn id(&self) -> Snowflake {
		match self {
			Self::Emoji { id, .. } | Self::Sticker { id, .. } => *id,
		}
	}

	pub fn name(&self) -> &str {
		match self {
			Self::Emoji { name, .. } | Self::Sticker { name, .. } => name,
		}
	}

	pub fn kind(&self) -> ExpressionKind {
		match self {
			Self::Emoji { animated: true, .. } => ExpressionKind::AnimatedEmoji,
			Self::Emoji { .. } => ExpressionKind::Emoji,
			Self::Sticker { .. } => ExpressionKind::Sticker,
		}
	}

	pub fn is_sticker(&self) -> bool {
		matches!(self, Self::Sticker { .. })
	}

	pub fn is_image(&self) -> bool {
		match self {
			Self::Emoji { .. } => true,
			Self::Sticker { format, .. } => {
				matches!(
					format,
					StickerFormat::Png | StickerFormat::Apng | StickerFormat::Gif
				)
			}
		}
	}
}

impl ExpressionKind {
	pub fn noun(self) -> &'static str {
		match self {
			Self::Emoji => "emoji",
			Self::AnimatedEmoji => "animated emoji",
			Self::Sticker => "sticker",
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slots {
	pub used: usize,
	pub max: usize,
}

impl Slots {
	pub fn full(self) -> bool {
		self.used >= self.max
	}

	pub fn free(self) -> usize {
		self.max.saturating_sub(self.used)
	}
}

pub fn slot_limit(tier: Option<PremiumTier>, features: &[String], kind: ExpressionKind) -> usize {
	let level = match tier {
		Some(PremiumTier::Tier1) => 1,
		Some(PremiumTier::Tier2) => 2,
		Some(PremiumTier::Tier3) => 3,
		_ => 0,
	};
	let has = |feature: &str| features.iter().any(|known| known == feature);
	match kind {
		ExpressionKind::Emoji | ExpressionKind::AnimatedEmoji => {
			let floor = if has(MORE_EMOJI) { MORE_EMOJI_SLOTS } else { 0 };
			EMOJI_SLOTS[level].max(floor)
		}
		ExpressionKind::Sticker => {
			let floor = if has(MORE_STICKERS) {
				MORE_STICKERS_SLOTS
			} else {
				0
			};
			STICKER_SLOTS[level].max(floor)
		}
	}
}

impl State {
	pub fn may_create_expressions(&self, guild_id: Snowflake) -> bool {
		self.current_user_id().is_some_and(|me| {
			self.guild_permissions(guild_id, me)
				.intersects(Permissions::CREATE_GUILD_EXPRESSIONS | Permissions::MANAGE_GUILD_EXPRESSIONS)
		})
	}

	pub fn may_create_expressions_anywhere(&self) -> bool {
		self.guilds()
			.any(|guild| !guild.unavailable && self.may_create_expressions(guild.id))
	}

	pub fn expression_slots(&self, guild_id: Snowflake, kind: ExpressionKind) -> Option<Slots> {
		let guild = self.guild(guild_id)?;
		let used = match kind {
			ExpressionKind::Emoji | ExpressionKind::AnimatedEmoji => {
				let animated = kind == ExpressionKind::AnimatedEmoji;
				guild
					.emojis
					.iter()
					.filter(|emoji| emoji.animated.unwrap_or(false) == animated)
					.count()
			}
			ExpressionKind::Sticker => self.guild_stickers(guild_id).map_or(0, <[_]>::len),
		};
		Some(Slots {
			used,
			max: slot_limit(guild.premium_tier, &guild.features, kind),
		})
	}
}

pub fn emoji_name_problem(name: &str) -> Option<String> {
	let length = name.chars().count();
	if !(EMOJI_NAME_MIN..=EMOJI_NAME_MAX).contains(&length) {
		return Some(format!(
			"An emoji name must be {EMOJI_NAME_MIN} to {EMOJI_NAME_MAX} characters"
		));
	}
	if !name
		.chars()
		.all(|character| character.is_ascii_alphanumeric() || character == '_')
	{
		return Some("An emoji name may only use letters, numbers and underscores".to_owned());
	}
	None
}

pub fn sticker_problem(name: &str, description: &str, tags: &str) -> Option<String> {
	let length = name.trim().chars().count();
	if !(STICKER_NAME_MIN..=STICKER_NAME_MAX).contains(&length) {
		return Some(format!(
			"A sticker name must be {STICKER_NAME_MIN} to {STICKER_NAME_MAX} characters"
		));
	}
	let description = description.trim().chars().count();
	if description != 0 && !(STICKER_DESCRIPTION_MIN..=STICKER_DESCRIPTION_MAX).contains(&description) {
		return Some(format!(
			"A sticker description must be empty or {STICKER_DESCRIPTION_MIN} to {STICKER_DESCRIPTION_MAX} characters"
		));
	}
	let tags = tags.trim().chars().count();
	if !(1..=STICKER_TAGS_MAX).contains(&tags) {
		return Some(format!(
			"A sticker needs tags of at most {STICKER_TAGS_MAX} characters"
		));
	}
	None
}

pub fn emoji_name_from(name: &str) -> String {
	let mut cleaned: String = name
		.chars()
		.filter(|character| character.is_ascii_alphanumeric() || *character == '_')
		.take(EMOJI_NAME_MAX)
		.collect();
	while cleaned.chars().count() < EMOJI_NAME_MIN {
		cleaned.push('_');
	}
	cleaned
}

pub fn sticker_name_from(name: &str) -> String {
	let mut cleaned: String = name.trim().chars().take(STICKER_NAME_MAX).collect();
	while cleaned.chars().count() < STICKER_NAME_MIN {
		cleaned.push('_');
	}
	cleaned
}
