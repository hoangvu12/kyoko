use std::sync::Arc;

use media::cdn;
use protocol::Snowflake;
use session::Command;
use state::{Expression, ExpressionKind, Slots, State, emoji_name_from, sticker_name_from};

use crate::actions::MenuEntry;
use crate::emoji_picker::EmojiCell;
use crate::sticker_picker::StickerCell;

pub const CUSTOM_EMOJI: &str = "custom_emoji";
pub const IMAGE_STICKER: &str = "image_sticker";
pub const CAN_CREATE_EXPRESSIONS: &str = "can_create_expressions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMenuItem {
	pub plugin: String,
	pub key: String,
	pub label: String,
	pub icon: Option<Arc<str>>,
	pub when: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuFacts {
	pub target: Option<Expression>,
	pub can_create_expressions: bool,
}

impl MenuFacts {
	pub fn of(state: &State, target: Option<Expression>) -> Self {
		Self {
			can_create_expressions: state.may_create_expressions_anywhere(),
			target,
		}
	}

	fn holds(&self, condition: &str) -> bool {
		match condition {
			CUSTOM_EMOJI => matches!(self.target, Some(Expression::Emoji { .. })),
			IMAGE_STICKER => self
				.target
				.as_ref()
				.is_some_and(|target| target.is_sticker() && target.is_image()),
			CAN_CREATE_EXPRESSIONS => self.can_create_expressions,
			_ => false,
		}
	}
}

pub fn shown_plugin_items(items: &[PluginMenuItem], facts: &MenuFacts) -> Vec<usize> {
	items
		.iter()
		.enumerate()
		.filter(|(_, item)| item.when.iter().all(|condition| facts.holds(condition)))
		.map(|(index, _)| index)
		.collect()
}

pub fn plugin_entries(items: &[PluginMenuItem], facts: &MenuFacts) -> Vec<MenuEntry> {
	shown_plugin_items(items, facts)
		.into_iter()
		.map(MenuEntry::Plugin)
		.collect()
}

pub fn append_plugin_entries(menu: &mut Vec<MenuEntry>, items: &[PluginMenuItem], facts: &MenuFacts) {
	let extra = plugin_entries(items, facts);
	if extra.is_empty() || menu.is_empty() {
		return;
	}
	if !matches!(menu.last(), Some(MenuEntry::Separator)) {
		menu.push(MenuEntry::Separator);
	}
	menu.extend(extra);
}

pub fn emoji_expression(name: &str, id: Snowflake, animated: bool) -> Expression {
	Expression::Emoji {
		id,
		name: name.to_owned(),
		animated,
	}
}

pub fn reaction_expression(emoji: &protocol::PartialEmoji) -> Option<Expression> {
	Some(emoji_expression(
		emoji.name.as_deref().unwrap_or_default(),
		emoji.id?,
		emoji.animated,
	))
}

pub fn sticker_expression(item: &protocol::StickerItem) -> Option<Expression> {
	Some(Expression::Sticker {
		id: item.id,
		name: item.name.clone(),
		format: item.format_type?,
	})
}

pub fn cell_expression(cell: &EmojiCell) -> Option<Expression> {
	let custom = cell.custom.as_ref()?;
	let name = cell
		.markup
		.trim_start_matches('<')
		.trim_start_matches('a')
		.trim_start_matches(':')
		.split(':')
		.next()
		.unwrap_or_default();
	Some(emoji_expression(name, custom.id, custom.animated))
}

pub fn sticker_cell_expression(cell: &StickerCell) -> Option<Expression> {
	sticker_expression(&cell.item())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneTarget {
	pub guild_id: Snowflake,
	pub name: String,
	pub icon: Option<String>,
	pub slots: Option<Slots>,
	pub blocked: Option<String>,
}

pub fn clone_targets(state: &State, expression: &Expression) -> Vec<CloneTarget> {
	let kind = expression.kind();
	let mut targets: Vec<CloneTarget> = state
		.guilds()
		.filter(|guild| !guild.unavailable && state.may_create_expressions(guild.id))
		.map(|guild| {
			let slots = state.expression_slots(guild.id, kind);
			let blocked = slots
				.filter(|slots| slots.full())
				.map(|slots| format!("No free {} slots ({} of {})", kind.noun(), slots.used, slots.max));
			CloneTarget {
				guild_id: guild.id,
				name: guild.name.clone(),
				icon: guild.icon.clone(),
				slots,
				blocked,
			}
		})
		.collect();
	targets.sort_by_cached_key(|target| target.name.to_lowercase());
	targets
}

pub fn clone_source(cdn_base: &str, expression: &Expression) -> Option<String> {
	match expression {
		Expression::Emoji { id, animated, .. } => Some(cdn::emoji_source_url(cdn_base, id.0, *animated)),
		Expression::Sticker { id, format, .. } => {
			cdn::sticker_source_url(id.0, cdn::StickerFormat::from(i64::from(*format)))
		}
	}
}

pub fn clone_name(expression: &Expression) -> String {
	match expression {
		Expression::Emoji { name, .. } => emoji_name_from(name),
		Expression::Sticker { name, .. } => sticker_name_from(name),
	}
}

pub fn clone_command(
	state: &State,
	expression: &Expression,
	guild_id: Snowflake,
	name: &str,
	bytes: Vec<u8>,
) -> Command {
	match expression {
		Expression::Emoji { .. } => Command::CreateEmoji {
			guild_id,
			name: name.trim().to_owned(),
			image: bytes,
		},
		Expression::Sticker { id, .. } => {
			let known = state.find_sticker(*id);
			let tags = known
				.and_then(|sticker| sticker.tags.clone())
				.filter(|tags| !tags.trim().is_empty())
				.unwrap_or_else(|| name.trim().to_owned());
			let description = known
				.and_then(|sticker| sticker.description.clone())
				.filter(|description| state::sticker_problem(name, description, &tags).is_none())
				.unwrap_or_default();
			Command::CreateSticker {
				guild_id,
				name: name.trim().to_owned(),
				description,
				tags,
				file: bytes,
			}
		}
	}
}

pub fn clone_name_problem(expression: &Expression, name: &str) -> Option<String> {
	match expression.kind() {
		ExpressionKind::Emoji | ExpressionKind::AnimatedEmoji => state::emoji_name_problem(name.trim()),
		ExpressionKind::Sticker => state::sticker_problem(name, "", "tags"),
	}
}
