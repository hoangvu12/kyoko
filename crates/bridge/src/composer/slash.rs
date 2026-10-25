use protocol::{ApplicationCommandIndex, CommandOption, CommandOptionType, InteractionOption, Snowflake};
use serde_json::{Number, Value};

use super::suggest::chat_commands;

#[derive(Debug, Clone, PartialEq)]
pub enum Slash {
	Text,
	Run {
		command_id: Snowflake,
		options: Vec<InteractionOption>,
	},
	Invalid(String),
}

fn markers<'a>(rest: &'a str, options: &[&'a CommandOption]) -> Vec<(usize, usize, &'a str)> {
	let mut found = Vec::new();
	for option in options {
		let marker = format!("{}:", option.name);
		let mut from = 0;
		while let Some(offset) = rest[from..].find(&marker) {
			let at = from + offset;
			let boundary = at == 0 || rest[..at].ends_with(char::is_whitespace);
			if boundary {
				found.push((at, at + marker.len(), option.name.as_str()));
				break;
			}
			from = at + marker.len();
		}
	}
	found.sort_by_key(|(at, ..)| *at);
	found
}

fn mention_id(value: &str, prefixes: &[&str]) -> Option<String> {
	let trimmed = value.trim();
	if trimmed.parse::<u64>().is_ok() {
		return Some(trimmed.to_owned());
	}
	let inner = trimmed.strip_prefix('<')?.strip_suffix('>')?;
	prefixes
		.iter()
		.find_map(|prefix| inner.strip_prefix(prefix))
		.filter(|id| id.parse::<u64>().is_ok())
		.map(str::to_owned)
}

fn option_value(command: &str, option: &CommandOption, raw: &str) -> Result<Value, String> {
	let raw = raw.trim();
	let invalid = || format!("/{command}: {} is not a valid {}", raw, option.name);
	if !option.choices.is_empty() {
		return option
			.choices
			.iter()
			.find(|choice| {
				choice.name.eq_ignore_ascii_case(raw)
					|| match &choice.value {
						Value::String(text) => text.eq_ignore_ascii_case(raw),
						other => serde_json::from_str::<Value>(raw).is_ok_and(|parsed| &parsed == other),
					}
			})
			.map(|choice| choice.value.clone())
			.ok_or_else(|| {
				let names: Vec<&str> = option.choices.iter().map(|choice| choice.name.as_str()).collect();
				format!("/{command}: {} must be one of {}", option.name, names.join(", "))
			});
	}
	match option.kind {
		CommandOptionType::String => Ok(Value::String(raw.to_owned())),
		CommandOptionType::Integer => raw.parse::<i64>().map(Value::from).map_err(|_| invalid()),
		CommandOptionType::Number => raw
			.parse::<f64>()
			.ok()
			.and_then(Number::from_f64)
			.map(Value::Number)
			.ok_or_else(invalid),
		CommandOptionType::Boolean => match raw.to_ascii_lowercase().as_str() {
			"true" | "yes" | "on" => Ok(Value::Bool(true)),
			"false" | "no" | "off" => Ok(Value::Bool(false)),
			_ => Err(invalid()),
		},
		CommandOptionType::User => mention_id(raw, &["@!", "@"])
			.map(Value::String)
			.ok_or_else(invalid),
		CommandOptionType::Channel => mention_id(raw, &["#"]).map(Value::String).ok_or_else(invalid),
		CommandOptionType::Role => mention_id(raw, &["@&"]).map(Value::String).ok_or_else(invalid),
		CommandOptionType::Mentionable => mention_id(raw, &["@&", "@!", "@"])
			.map(Value::String)
			.ok_or_else(invalid),
		CommandOptionType::Attachment => Err(format!(
			"/{command}: file options are not supported yet, use the official client"
		)),
		_ => Err(invalid()),
	}
}

fn leaf_options(
	command: &str,
	options: &[&CommandOption],
	rest: &str,
) -> Result<Vec<InteractionOption>, String> {
	let mut values: Vec<(&CommandOption, &str)> = Vec::new();
	let found = markers(rest, options);
	if found.is_empty() {
		if !rest.trim().is_empty() {
			let first = options
				.iter()
				.find(|option| option.is_required())
				.or_else(|| options.first())
				.ok_or_else(|| format!("/{command} takes no options"))?;
			values.push((first, rest));
		}
	} else {
		if !rest[..found[0].0].trim().is_empty() {
			return Err(format!("/{command}: name each option, like {}:value", found[0].2));
		}
		for (index, (_, value_start, name)) in found.iter().enumerate() {
			let value_end = found.get(index + 1).map_or(rest.len(), |next| next.0);
			if let Some(option) = options.iter().find(|option| option.name == *name) {
				values.push((option, &rest[*value_start..value_end]));
			}
		}
	}
	if let Some(missing) = options
		.iter()
		.find(|option| option.is_required() && !values.iter().any(|(given, _)| given.name == option.name))
	{
		return Err(format!("/{command} needs {}", missing.name));
	}
	values
		.into_iter()
		.filter(|(_, raw)| !raw.trim().is_empty())
		.map(|(option, raw)| {
			Ok(InteractionOption {
				kind: option.kind,
				name: option.name.clone(),
				value: Some(option_value(command, option, raw)?),
				options: Vec::new(),
			})
		})
		.collect()
}

fn split_word(text: &str) -> (&str, &str) {
	let text = text.trim_start();
	match text.find(char::is_whitespace) {
		Some(at) => (&text[..at], &text[at..]),
		None => (text, ""),
	}
}

fn nested(command: &str, options: &[&CommandOption], rest: &str) -> Result<Vec<InteractionOption>, String> {
	if !options.iter().any(|option| option.is_group()) {
		return leaf_options(command, options, rest);
	}
	let (word, rest) = split_word(rest);
	let group = options
		.iter()
		.find(|option| option.is_group() && option.name.eq_ignore_ascii_case(word))
		.ok_or_else(|| {
			let names: Vec<&str> = options.iter().map(|option| option.name.as_str()).collect();
			format!("/{command}: choose one of {}", names.join(", "))
		})?;
	let children: Vec<&CommandOption> = group.options.iter().collect();
	let path = format!("{command} {}", group.name);
	Ok(vec![InteractionOption {
		kind: group.kind,
		name: group.name.clone(),
		value: None,
		options: nested(&path, &children, rest)?,
	}])
}

pub fn parse_slash(index: Option<&ApplicationCommandIndex>, text: &str) -> Slash {
	let Some(body) = text.trim().strip_prefix('/') else {
		return Slash::Text;
	};
	let (name, rest) = split_word(body);
	let Some(command) =
		index.and_then(|index| chat_commands(index).find(|command| command.name.eq_ignore_ascii_case(name)))
	else {
		return Slash::Text;
	};
	let options: Vec<&CommandOption> = command.options.iter().collect();
	match nested(&command.name, &options, rest) {
		Ok(options) => Slash::Run {
			command_id: command.id,
			options,
		},
		Err(reason) => Slash::Invalid(reason),
	}
}
