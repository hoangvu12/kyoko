use serde::{Deserialize, Serialize};

use crate::emoji::PartialEmoji;
use crate::lenient::{LenientVec, int_enum};
use crate::snowflake::Snowflake;

int_enum! {
	pub enum ComponentType {
		ActionRow = 1,
		Button = 2,
		StringSelect = 3,
		TextInput = 4,
		UserSelect = 5,
		RoleSelect = 6,
		MentionableSelect = 7,
		ChannelSelect = 8,
		Section = 9,
		TextDisplay = 10,
		Thumbnail = 11,
		MediaGallery = 12,
		File = 13,
		Separator = 14,
		Container = 17,
		Label = 18,
	}
}

int_enum! {
	pub enum ButtonStyle {
		Primary = 1,
		Secondary = 2,
		Success = 3,
		Danger = 4,
		Link = 5,
		Premium = 6,
	}
}

int_enum! {
	pub enum TextInputStyle {
		Short = 1,
		Paragraph = 2,
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UnfurledMedia {
	pub url: String,
	pub proxy_url: Option<String>,
	pub width: Option<i64>,
	pub height: Option<i64>,
	pub content_type: Option<String>,
	pub placeholder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectOption {
	pub label: String,
	pub value: String,
	pub description: Option<String>,
	pub emoji: Option<PartialEmoji>,
	pub default: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectDefault {
	pub id: Snowflake,
	#[serde(rename = "type")]
	pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GalleryItem {
	pub media: UnfurledMedia,
	pub description: Option<String>,
	pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Component {
	#[serde(rename = "type")]
	pub kind: ComponentType,
	pub id: Option<i64>,
	pub custom_id: Option<String>,
	pub style: Option<i64>,
	pub label: Option<String>,
	pub emoji: Option<PartialEmoji>,
	pub url: Option<String>,
	pub sku_id: Option<Snowflake>,
	pub disabled: bool,
	pub placeholder: Option<String>,
	pub min_values: Option<i64>,
	pub max_values: Option<i64>,
	pub options: LenientVec<SelectOption>,
	pub default_values: LenientVec<SelectDefault>,
	pub channel_types: LenientVec<i64>,
	pub components: LenientVec<Component>,
	pub component: Option<Box<Component>>,
	pub accessory: Option<Box<Component>>,
	pub content: Option<String>,
	pub media: Option<UnfurledMedia>,
	pub description: Option<String>,
	pub spoiler: bool,
	pub items: LenientVec<GalleryItem>,
	pub file: Option<UnfurledMedia>,
	pub name: Option<String>,
	pub size: Option<i64>,
	pub divider: Option<bool>,
	pub spacing: Option<i64>,
	pub accent_color: Option<i64>,
	pub min_length: Option<i64>,
	pub max_length: Option<i64>,
	pub required: Option<bool>,
	pub value: Option<String>,
}

impl Component {
	pub fn button_style(&self) -> ButtonStyle {
		ButtonStyle::from(self.style.unwrap_or(1))
	}

	pub fn text_style(&self) -> TextInputStyle {
		TextInputStyle::from(self.style.unwrap_or(1))
	}

	pub fn is_select(&self) -> bool {
		matches!(
			self.kind,
			ComponentType::StringSelect
				| ComponentType::UserSelect
				| ComponentType::RoleSelect
				| ComponentType::MentionableSelect
				| ComponentType::ChannelSelect
		)
	}

	pub fn is_required(&self) -> bool {
		self.required.unwrap_or(true)
	}

	pub fn unknown_kinds(components: &[Component]) -> Vec<i64> {
		let mut found = Vec::new();
		for component in components {
			component.collect_unknown(&mut found);
		}
		found
	}

	fn collect_unknown(&self, found: &mut Vec<i64>) {
		if let ComponentType::Unknown(kind) = self.kind {
			found.push(kind);
			return;
		}
		for child in self
			.components
			.iter()
			.chain(self.component.as_deref())
			.chain(self.accessory.as_deref())
		{
			child.collect_unknown(found);
		}
	}
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModalApplication {
	pub id: Snowflake,
	pub name: String,
	pub icon: Option<String>,
	pub bot_id: Option<Snowflake>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InteractionModal {
	pub id: Snowflake,
	pub channel_id: Snowflake,
	pub custom_id: String,
	pub application: ModalApplication,
	pub title: String,
	pub components: LenientVec<Component>,
	pub nonce: Option<String>,
}
