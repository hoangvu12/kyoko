use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::lenient::{LenientVec, int_enum};
use crate::snowflake::Snowflake;

int_enum! {
	pub enum ApplicationCommandType {
		ChatInput = 1,
		User = 2,
		Message = 3,
		PrimaryEntryPoint = 4,
	}
}

int_enum! {
	pub enum CommandOptionType {
		SubCommand = 1,
		SubCommandGroup = 2,
		String = 3,
		Integer = 4,
		Boolean = 5,
		User = 6,
		Channel = 7,
		Role = 8,
		Mentionable = 9,
		Number = 10,
		Attachment = 11,
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandChoice {
	pub name: String,
	pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandOption {
	#[serde(rename = "type")]
	pub kind: CommandOptionType,
	pub name: String,
	pub description: Option<String>,
	pub required: Option<bool>,
	pub choices: LenientVec<CommandChoice>,
	pub options: LenientVec<CommandOption>,
	pub autocomplete: Option<bool>,
}

impl CommandOption {
	pub fn is_required(&self) -> bool {
		self.required == Some(true)
	}

	pub fn is_group(&self) -> bool {
		matches!(
			self.kind,
			CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
		)
	}
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
struct CommandFields {
	id: Snowflake,
	#[serde(rename = "type")]
	kind: ApplicationCommandType,
	application_id: Snowflake,
	version: Option<Snowflake>,
	name: String,
	description: Option<String>,
	options: LenientVec<CommandOption>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ApplicationCommand {
	pub id: Snowflake,
	pub kind: ApplicationCommandType,
	pub application_id: Snowflake,
	pub version: Option<Snowflake>,
	pub name: String,
	pub description: Option<String>,
	pub options: LenientVec<CommandOption>,
	pub raw: Value,
}

impl<'de> Deserialize<'de> for ApplicationCommand {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let raw = Value::deserialize(deserializer)?;
		let fields = CommandFields::deserialize(&raw).map_err(D::Error::custom)?;
		Ok(ApplicationCommand {
			id: fields.id,
			kind: fields.kind,
			application_id: fields.application_id,
			version: fields.version,
			name: fields.name,
			description: fields.description,
			options: fields.options,
			raw,
		})
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandApplication {
	pub id: Snowflake,
	pub name: String,
	pub icon: Option<String>,
	pub bot_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ApplicationCommandIndex {
	pub applications: LenientVec<CommandApplication>,
	pub application_commands: LenientVec<ApplicationCommand>,
}

impl ApplicationCommandIndex {
	pub fn merge(&mut self, other: ApplicationCommandIndex) {
		for application in other.applications {
			if !self.applications.iter().any(|known| known.id == application.id) {
				self.applications.push(application);
			}
		}
		for command in other.application_commands {
			if !self
				.application_commands
				.iter()
				.any(|known| known.id == command.id)
			{
				self.application_commands.push(command);
			}
		}
	}

	pub fn application(&self, id: Snowflake) -> Option<&CommandApplication> {
		self.applications.iter().find(|application| application.id == id)
	}

	pub fn command(&self, id: Snowflake) -> Option<&ApplicationCommand> {
		self.application_commands.iter().find(|command| command.id == id)
	}
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InteractionOption {
	#[serde(rename = "type")]
	pub kind: CommandOptionType,
	pub name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub value: Option<Value>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub options: Vec<InteractionOption>,
}
