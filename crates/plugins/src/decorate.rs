use mlua::{Lua, Table, Value};
use session::DecoratedUser;

use crate::calls::{Decor, Mark};
use crate::manifest::is_icon;
use crate::vm::{label, table, word};

pub(crate) const MARK_LIMIT: usize = 4;
const TOKEN_LIMIT: usize = 48;
const TOKEN_PREFIX: &str = "$kyoko.";

pub(crate) fn user_table(lua: &Lua, user: &DecoratedUser, presence: bool) -> mlua::Result<Table> {
	let argument = table(
		lua,
		&[
			("user_id", Some(user.user_id.to_string())),
			("guild_id", user.guild_id.map(|id| id.to_string())),
		],
	)?;
	argument.raw_set("bot", user.bot)?;
	argument.raw_set("mine", user.mine)?;
	argument.raw_set("owner", user.owner)?;
	if presence {
		argument.raw_set("status", user.status)?;
		let platforms = lua.create_table()?;
		for (platform, status) in user.platforms.listed() {
			platforms.raw_set(platform, status)?;
		}
		argument.raw_set("platforms", platforms)?;
	}
	Ok(argument)
}

pub(crate) fn decor(value: &Value) -> Option<Decor> {
	let Value::Table(answer) = value else {
		return None;
	};
	let decor = Decor {
		members: marks(answer, "members"),
		messages: marks(answer, "messages"),
		profiles: marks(answer, "profiles"),
		crown: matches!(answer.raw_get::<Value>("crown"), Ok(Value::Boolean(true))),
	};
	(decor != Decor::default()).then_some(decor)
}

fn marks(answer: &Table, place: &str) -> Vec<Mark> {
	let Ok(Value::Table(list)) = answer.raw_get::<Value>(place) else {
		return Vec::new();
	};
	list.sequence_values::<Value>()
		.filter_map(|value| match value {
			Ok(Value::Table(entry)) => mark(&entry),
			_ => None,
		})
		.take(MARK_LIMIT)
		.collect()
}

fn mark(entry: &Table) -> Option<Mark> {
	let icon = word(entry, "icon").filter(|name| is_icon(name))?;
	let color = word(entry, "color")
		.and_then(|color| theme_token(&color))
		.unwrap_or_else(|| "text_4".to_owned());
	let tooltip = entry
		.raw_get::<Value>("tooltip")
		.ok()
		.and_then(|text| label(&text))
		.unwrap_or_default();
	Some(Mark { icon, color, tooltip })
}

pub(crate) fn theme_token(text: &str) -> Option<String> {
	let name = text.strip_prefix(TOKEN_PREFIX).unwrap_or(text);
	let valid = !name.is_empty()
		&& name.len() <= TOKEN_LIMIT
		&& name
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
	valid.then(|| name.to_owned())
}
