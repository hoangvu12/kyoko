use protocol::Snowflake;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Formatting {
	Names,
	RelativeTimes,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Person {
	pub user_id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub username: String,
	pub global_name: Option<String>,
	pub nick: Option<String>,
}

impl Person {
	pub fn name(&self) -> &str {
		[self.nick.as_deref(), self.global_name.as_deref()]
			.into_iter()
			.flatten()
			.find(|name| !name.is_empty())
			.unwrap_or(&self.username)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShownName {
	pub name: String,
	pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelativeTime {
	pub seconds: u64,
	pub past: bool,
	pub unit: &'static str,
	pub value: f64,
	pub text: String,
}

pub trait DisplayHooks: Send + Sync {
	fn formatters(&self, formatting: Formatting) -> Vec<String>;

	fn format_name(&self, person: &Person) -> Option<ShownName>;

	fn format_relative(&self, time: &RelativeTime) -> Option<String>;
}
