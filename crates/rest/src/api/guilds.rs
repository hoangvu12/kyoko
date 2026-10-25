use protocol::{Channel, Emoji, Guild, LenientVec, Member, Role, Snowflake};
use serde_json::{Value, json};

use crate::client::RestClient;
use crate::error::Result;
use crate::request::Request;

const DEFAULT_GUILD_TEMPLATE: &str = "2TffvPucqHkN";

impl RestClient {
	pub async fn guild(&self, id: Snowflake) -> Result<Guild> {
		self.send(Request::get(format!("/guilds/{id}"))).await
	}

	pub async fn guild_channels(&self, id: Snowflake) -> Result<LenientVec<Channel>> {
		self.send(Request::get(format!("/guilds/{id}/channels"))).await
	}

	pub async fn guild_members_search(
		&self,
		id: Snowflake,
		query: &str,
		limit: u8,
	) -> Result<LenientVec<Member>> {
		let request = Request::get(format!("/guilds/{id}/members/search"))
			.query(&[("query", query.to_owned()), ("limit", limit.to_string())]);
		self.send(request).await
	}

	pub async fn guild_member(&self, id: Snowflake, user_id: Snowflake) -> Result<Member> {
		self.send(Request::get(format!("/guilds/{id}/members/{user_id}")))
			.await
	}

	pub async fn guild_roles(&self, id: Snowflake) -> Result<LenientVec<Role>> {
		self.send(Request::get(format!("/guilds/{id}/roles"))).await
	}

	pub async fn guild_emojis(&self, id: Snowflake) -> Result<LenientVec<Emoji>> {
		self.send(Request::get(format!("/guilds/{id}/emojis"))).await
	}

	pub async fn create_guild(&self, name: &str, icon: Option<&str>) -> Result<Guild> {
		let request = Request::post("/guilds").json(&json!({
			"name": name,
			"icon": icon,
			"channels": [],
			"system_channel_id": null,
			"guild_template_code": DEFAULT_GUILD_TEMPLATE,
		}));
		self.send(request).await
	}

	pub async fn guild_onboarding(&self, id: Snowflake) -> Result<Value> {
		self.send(Request::get(format!("/guilds/{id}/onboarding"))).await
	}

	pub async fn onboarding_responses(&self, id: Snowflake, body: Value) -> Result<()> {
		self.send_empty(Request::put(format!("/guilds/{id}/onboarding-responses")).json(&body))
			.await
	}

	pub async fn member_verification(&self, id: Snowflake, invite_code: Option<&str>) -> Result<Value> {
		let mut pairs = vec![("with_guild", "false".to_owned())];
		if let Some(code) = invite_code {
			pairs.push(("invite_code", code.to_owned()));
		}
		self.send(Request::get(format!("/guilds/{id}/member-verification")).query(&pairs))
			.await
	}

	pub async fn submit_join_request(&self, id: Snowflake, body: Value) -> Result<()> {
		self.send_empty(Request::put(format!("/guilds/{id}/requests/@me")).json(&body))
			.await
	}

	pub async fn leave_guild(&self, id: Snowflake) -> Result<()> {
		self.send_empty(Request::delete(format!("/users/@me/guilds/{id}")))
			.await
	}

	pub async fn guild_preview(&self, id: Snowflake) -> Result<Guild> {
		self.send(Request::get(format!("/guilds/{id}/preview"))).await
	}
}
