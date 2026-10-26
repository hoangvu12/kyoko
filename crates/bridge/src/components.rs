use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;

use protocol::{
	ChannelType, Component, ComponentType, ModalApplication, PartialEmoji, Snowflake, TextInputStyle,
};
use state::{AppModal, ModalStatus, State};

use crate::message::person_name;

const LISTED: usize = 25;

static NOTED: Mutex<Option<HashSet<i64>>> = Mutex::new(None);

pub fn note_unknown(components: &[Component]) {
	let kinds = Component::unknown_kinds(components);
	if kinds.is_empty() {
		return;
	}
	let Ok(mut noted) = NOTED.lock() else {
		return;
	};
	let noted = noted.get_or_insert_with(HashSet::new);
	for kind in kinds {
		if noted.insert(kind) {
			tracing::debug!(kind, "unknown message component type skipped");
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceOption {
	pub value: String,
	pub label: String,
	pub description: Option<String>,
	pub emoji: Option<PartialEmoji>,
	pub default: bool,
}

pub type SelectOptions = BTreeMap<String, Vec<ChoiceOption>>;

pub fn string_options(component: &Component) -> Vec<ChoiceOption> {
	component
		.options
		.iter()
		.map(|option| ChoiceOption {
			value: option.value.clone(),
			label: option.label.clone(),
			description: option.description.clone(),
			emoji: option.emoji.clone(),
			default: option.default,
		})
		.collect()
}

fn defaults(component: &Component) -> HashSet<Snowflake> {
	component.default_values.iter().map(|value| value.id).collect()
}

fn users(state: &State, guild_id: Option<Snowflake>, chosen: &HashSet<Snowflake>) -> Vec<ChoiceOption> {
	let mut people: Vec<_> = match guild_id.and_then(|guild_id| state.guild(guild_id)) {
		Some(guild) => guild
			.members
			.iter()
			.filter_map(|(_, member)| member.user.clone())
			.collect(),
		None => state.current_user().cloned().into_iter().collect(),
	};
	people.sort_by_key(|user| person_name(state, guild_id, user).to_lowercase());
	people
		.into_iter()
		.take(LISTED)
		.map(|user| ChoiceOption {
			value: user.id.to_string(),
			label: person_name(state, guild_id, &user),
			description: Some(user.username.clone()),
			emoji: None,
			default: chosen.contains(&user.id),
		})
		.collect()
}

fn roles(state: &State, guild_id: Option<Snowflake>, chosen: &HashSet<Snowflake>) -> Vec<ChoiceOption> {
	let Some(guild) = guild_id.and_then(|guild_id| state.guild(guild_id)) else {
		return Vec::new();
	};
	let mut roles: Vec<_> = guild.roles.values().filter(|role| role.id != guild.id).collect();
	roles.sort_by_key(|role| std::cmp::Reverse(role.position));
	roles
		.into_iter()
		.take(LISTED)
		.map(|role| ChoiceOption {
			value: role.id.to_string(),
			label: role.name.clone(),
			description: None,
			emoji: None,
			default: chosen.contains(&role.id),
		})
		.collect()
}

fn channels(
	state: &State,
	guild_id: Option<Snowflake>,
	component: &Component,
	chosen: &HashSet<Snowflake>,
) -> Vec<ChoiceOption> {
	let Some(guild) = guild_id.and_then(|guild_id| state.guild(guild_id)) else {
		return Vec::new();
	};
	let wanted: Vec<ChannelType> = component
		.channel_types
		.iter()
		.map(|kind| ChannelType::from(*kind))
		.collect();
	guild
		.channels
		.values()
		.filter(|channel| {
			if wanted.is_empty() {
				channel.kind != ChannelType::GuildCategory
			} else {
				wanted.contains(&channel.kind)
			}
		})
		.take(LISTED)
		.map(|channel| ChoiceOption {
			value: channel.id.to_string(),
			label: format!("#{}", channel.name.clone().unwrap_or_default()),
			description: None,
			emoji: None,
			default: chosen.contains(&channel.id),
		})
		.collect()
}

pub fn options_for(state: &State, guild_id: Option<Snowflake>, component: &Component) -> Vec<ChoiceOption> {
	let chosen = defaults(component);
	match component.kind {
		ComponentType::StringSelect => string_options(component),
		ComponentType::UserSelect => users(state, guild_id, &chosen),
		ComponentType::RoleSelect => roles(state, guild_id, &chosen),
		ComponentType::MentionableSelect => {
			let mut options = users(state, guild_id, &chosen);
			options.extend(roles(state, guild_id, &chosen));
			options
		}
		ComponentType::ChannelSelect => channels(state, guild_id, component, &chosen),
		_ => Vec::new(),
	}
}

fn walk<'a>(components: &'a [Component], out: &mut Vec<&'a Component>) {
	for component in components {
		out.push(component);
		walk(&component.components, out);
		if let Some(child) = component.component.as_deref() {
			walk(std::slice::from_ref(child), out);
		}
		if let Some(child) = component.accessory.as_deref() {
			walk(std::slice::from_ref(child), out);
		}
	}
}

pub fn select_options(state: &State, guild_id: Option<Snowflake>, components: &[Component]) -> SelectOptions {
	let mut all = Vec::new();
	walk(components, &mut all);
	all.into_iter()
		.filter(|component| component.is_select() && component.kind != ComponentType::StringSelect)
		.filter_map(|component| {
			let custom_id = component.custom_id.clone()?;
			Some((custom_id, options_for(state, guild_id, component)))
		})
		.collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
	Short,
	Paragraph,
	Select { multi: bool, options: Vec<ChoiceOption> },
	Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModalField {
	pub custom_id: String,
	pub label: String,
	pub description: Option<String>,
	pub required: bool,
	pub kind: FieldKind,
	pub placeholder: Option<String>,
	pub min_length: Option<i64>,
	pub max_length: Option<i64>,
	pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModalForm {
	pub serial: u64,
	pub title: String,
	pub application: ModalApplication,
	pub fields: Vec<ModalField>,
	pub status: ModalStatus,
	pub missing: Vec<String>,
}

impl ModalForm {
	pub fn of(state: &State) -> Option<Self> {
		let open: &AppModal = state.app_modal()?;
		let guild_id = state.channel_guild(open.modal.channel_id);
		let mut fields = Vec::new();
		for component in open.modal.components.iter() {
			collect_fields(state, guild_id, component, None, &mut fields);
		}
		Some(Self {
			serial: open.serial,
			title: open.modal.title.clone(),
			application: open.modal.application.clone(),
			fields,
			status: open.status.clone(),
			missing: open.missing.clone(),
		})
	}

	pub fn failure(&self) -> Option<&'static str> {
		(self.status == ModalStatus::Failed).then_some(state::MODAL_FAILED)
	}
}

fn collect_fields(
	state: &State,
	guild_id: Option<Snowflake>,
	component: &Component,
	label: Option<(&str, Option<&str>)>,
	out: &mut Vec<ModalField>,
) {
	let kind = match component.kind {
		ComponentType::ActionRow => {
			for child in component.components.iter() {
				collect_fields(state, guild_id, child, label, out);
			}
			return;
		}
		ComponentType::Label => {
			if let Some(child) = component.component.as_deref() {
				let named = component.label.as_deref().unwrap_or_default();
				collect_fields(
					state,
					guild_id,
					child,
					Some((named, component.description.as_deref())),
					out,
				);
			}
			return;
		}
		ComponentType::TextDisplay => FieldKind::Text(component.content.clone().unwrap_or_default()),
		ComponentType::TextInput => match component.text_style() {
			TextInputStyle::Paragraph => FieldKind::Paragraph,
			_ => FieldKind::Short,
		},
		_ if component.is_select() => FieldKind::Select {
			multi: component.max_values.unwrap_or(1) > 1,
			options: options_for(state, guild_id, component),
		},
		_ => return,
	};
	let (label, description) = match label {
		Some((label, description)) => (label.to_owned(), description.map(str::to_owned)),
		None => (component.label.clone().unwrap_or_default(), None),
	};
	out.push(ModalField {
		custom_id: component.custom_id.clone().unwrap_or_default(),
		label,
		description,
		required: !matches!(kind, FieldKind::Text(_)) && component.is_required(),
		kind,
		placeholder: component.placeholder.clone(),
		min_length: component.min_length,
		max_length: component.max_length,
		value: component.value.clone(),
	});
}
