use std::collections::{HashMap, HashSet};

use protocol::{LenientVec, PartialEmoji, Snowflake};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::changes::Changes;
use crate::state::State;

const ONBOARDING_FEATURE: &str = "GUILD_ONBOARDING";
const TERMS: &str = "TERMS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
	MultipleChoice,
	Dropdown,
	Unknown(i64),
}

impl From<i64> for PromptKind {
	fn from(value: i64) -> Self {
		match value {
			0 => Self::MultipleChoice,
			1 => Self::Dropdown,
			other => Self::Unknown(other),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnboardingOption {
	pub id: Snowflake,
	pub title: String,
	pub description: Option<String>,
	pub emoji: Option<PartialEmoji>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnboardingPrompt {
	pub id: Snowflake,
	pub kind: PromptKind,
	pub title: String,
	pub single_select: bool,
	pub required: bool,
	pub options: Vec<OnboardingOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Onboarding {
	pub guild_id: Snowflake,
	pub enabled: bool,
	pub prompts: Vec<OnboardingPrompt>,
	pub chosen: Vec<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
	Terms,
	TextInput,
	Paragraph,
	MultipleChoice,
	Unknown(String),
}

impl From<&str> for FieldKind {
	fn from(value: &str) -> Self {
		match value {
			TERMS => Self::Terms,
			"TEXT_INPUT" => Self::TextInput,
			"PARAGRAPH" => Self::Paragraph,
			"MULTIPLE_CHOICE" => Self::MultipleChoice,
			other => Self::Unknown(other.to_owned()),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerificationField {
	pub kind: FieldKind,
	pub label: String,
	pub rules: Vec<String>,
	pub required: bool,
	pub description: Option<String>,
	raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberVerification {
	pub guild_id: Snowflake,
	pub description: Option<String>,
	pub fields: Vec<VerificationField>,
	version: Value,
}

impl MemberVerification {
	pub fn only_terms(&self) -> bool {
		self.fields.iter().all(|field| field.kind == FieldKind::Terms)
	}

	pub fn agreement(&self) -> Option<Value> {
		if !self.only_terms() {
			return None;
		}
		let fields: Vec<Value> = self
			.fields
			.iter()
			.map(|field| {
				let mut answered = field.raw.clone();
				answered["response"] = Value::Bool(true);
				answered
			})
			.collect();
		Some(json!({ "version": self.version, "form_fields": fields }))
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
	pub serial: u64,
	pub code: String,
	pub guild_id: Option<Snowflake>,
	pub onboarding: bool,
	pub verification: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Community {
	unknown_invites: HashSet<String>,
	last_join: Option<Join>,
	joins: u64,
	onboarding: HashMap<Snowflake, Onboarding>,
	verification: HashMap<Snowflake, MemberVerification>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawOption {
	id: Option<Snowflake>,
	title: String,
	description: Option<String>,
	emoji: Option<PartialEmoji>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPrompt {
	id: Option<Snowflake>,
	#[serde(rename = "type")]
	kind: i64,
	options: LenientVec<RawOption>,
	title: String,
	single_select: bool,
	required: bool,
	in_onboarding: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawOnboarding {
	prompts: LenientVec<RawPrompt>,
	enabled: bool,
	responses: LenientVec<Snowflake>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawField {
	field_type: String,
	label: String,
	values: Option<Vec<String>>,
	required: bool,
	description: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawVerification {
	version: Value,
	description: Option<String>,
	form_fields: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawJoinedGuild {
	id: Option<Snowflake>,
	features: LenientVec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawAccepted {
	guild_id: Option<Snowflake>,
	guild: Option<RawJoinedGuild>,
	show_verification_form: bool,
}

fn prompt(raw: RawPrompt) -> Option<OnboardingPrompt> {
	if raw.in_onboarding == Some(false) {
		return None;
	}
	let options: Vec<OnboardingOption> = raw
		.options
		.into_iter()
		.filter_map(|option| {
			Some(OnboardingOption {
				id: option.id?,
				title: option.title,
				description: option.description.filter(|text| !text.is_empty()),
				emoji: option
					.emoji
					.filter(|emoji| emoji.id.is_some() || emoji.name.is_some()),
			})
		})
		.collect();
	Some(OnboardingPrompt {
		id: raw.id?,
		kind: PromptKind::from(raw.kind),
		title: raw.title,
		single_select: raw.single_select,
		required: raw.required,
		options,
	})
	.filter(|prompt| !prompt.options.is_empty())
}

fn field(raw: Value) -> Option<VerificationField> {
	let parsed = RawField::deserialize(&raw).ok()?;
	Some(VerificationField {
		kind: FieldKind::from(parsed.field_type.as_str()),
		label: parsed.label,
		rules: parsed.values.unwrap_or_default(),
		required: parsed.required,
		description: parsed.description.filter(|text| !text.is_empty()),
		raw,
	})
}

impl State {
	pub(crate) fn invite_unknown(&mut self, code: &str) -> Changes {
		self.lazy.community.unknown_invites.insert(code.to_owned());
		let mut changes = Changes::default();
		changes.loaded.insert(Snowflake(0));
		changes
	}

	pub(crate) fn invite_resolved(&mut self, code: &str) {
		self.lazy.community.unknown_invites.remove(code);
	}

	pub(crate) fn invite_accepted(&mut self, code: &str, raw: &Value) -> Changes {
		let accepted = RawAccepted::deserialize(raw).unwrap_or_default();
		let guild = accepted.guild.unwrap_or_default();
		let guild_id = accepted.guild_id.or(guild.id);
		let community = &mut self.lazy.community;
		community.joins += 1;
		community.last_join = Some(Join {
			serial: community.joins,
			code: code.to_owned(),
			guild_id,
			onboarding: guild.features.iter().any(|feature| feature == ONBOARDING_FEATURE),
			verification: accepted.show_verification_form,
		});
		let mut changes = Changes::default();
		changes.loaded.insert(guild_id.unwrap_or_default());
		changes
	}

	pub(crate) fn onboarding_loaded(&mut self, guild_id: Snowflake, raw: &Value) -> Changes {
		let parsed = RawOnboarding::deserialize(raw).unwrap_or_default();
		let onboarding = Onboarding {
			guild_id,
			enabled: parsed.enabled,
			prompts: parsed.prompts.into_iter().filter_map(prompt).collect(),
			chosen: parsed.responses.into_vec(),
		};
		self.lazy.community.onboarding.insert(guild_id, onboarding);
		let mut changes = Changes::default();
		changes.loaded.insert(guild_id);
		changes
	}

	pub(crate) fn onboarding_finished(&mut self, guild_id: Snowflake) -> Changes {
		self.lazy.community.onboarding.remove(&guild_id);
		let mut changes = Changes::default();
		changes.loaded.insert(guild_id);
		changes
	}

	pub(crate) fn verification_loaded(&mut self, guild_id: Snowflake, raw: &Value) -> Changes {
		let parsed = RawVerification::deserialize(raw).unwrap_or_default();
		let verification = MemberVerification {
			guild_id,
			description: parsed.description.filter(|text| !text.is_empty()),
			fields: parsed.form_fields.into_iter().filter_map(field).collect(),
			version: parsed.version,
		};
		self.lazy.community.verification.insert(guild_id, verification);
		let mut changes = Changes::default();
		changes.loaded.insert(guild_id);
		changes
	}

	pub(crate) fn verification_finished(&mut self, guild_id: Snowflake) -> Changes {
		self.lazy.community.verification.remove(&guild_id);
		let mut changes = Changes::default();
		changes.loaded.insert(guild_id);
		changes
	}

	pub fn is_invite_unknown(&self, code: &str) -> bool {
		self.lazy.community.unknown_invites.contains(code)
	}

	pub fn last_join(&self) -> Option<&Join> {
		self.lazy.community.last_join.as_ref()
	}

	pub fn onboarding(&self, guild_id: Snowflake) -> Option<&Onboarding> {
		self.lazy.community.onboarding.get(&guild_id)
	}

	pub fn member_verification(&self, guild_id: Snowflake) -> Option<&MemberVerification> {
		self.lazy.community.verification.get(&guild_id)
	}
}
