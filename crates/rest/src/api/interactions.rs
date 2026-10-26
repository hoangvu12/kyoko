use protocol::{ApplicationCommandIndex, LenientVec, Snowflake};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::RestClient;
use crate::error::Result;
use crate::request::Request;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandScope {
	Guild(Snowflake),
	Channel(Snowflake),
	User,
}

impl CommandScope {
	fn path(self) -> String {
		match self {
			CommandScope::Guild(id) => format!("/guilds/{id}/application-command-index"),
			CommandScope::Channel(id) => format!("/channels/{id}/application-command-index"),
			CommandScope::User => "/users/@me/application-command-index".to_owned(),
		}
	}
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadSlotRequest {
	pub id: String,
	pub filename: String,
	pub file_size: u64,
	pub is_clip: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UploadSlot {
	#[serde(deserialize_with = "protocol::lenient::int")]
	pub id: i64,
	pub upload_url: String,
	pub upload_filename: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct UploadSlots {
	attachments: LenientVec<UploadSlot>,
}

#[derive(Serialize)]
struct UploadSlotsBody<'a> {
	files: &'a [UploadSlotRequest],
}

impl RestClient {
	pub async fn application_command_index(&self, scope: CommandScope) -> Result<ApplicationCommandIndex> {
		self.send(Request::get(scope.path())).await
	}

	pub async fn create_interaction(&self, body: &Value) -> Result<()> {
		self.send_empty(Request::post("/interactions").multipart(Vec::new(), body))
			.await
	}

	pub async fn post_interaction(&self, body: &Value) -> Result<()> {
		self.send_empty(Request::post("/interactions").json(body)).await
	}

	pub async fn attachment_slots(
		&self,
		channel_id: Snowflake,
		files: &[UploadSlotRequest],
	) -> Result<Vec<UploadSlot>> {
		let request =
			Request::post(format!("/channels/{channel_id}/attachments")).json(&UploadSlotsBody { files });
		let slots: UploadSlots = self.send(request).await?;
		Ok(slots.attachments.into_vec())
	}
}
