use std::fmt;

use wreq::Method;

const MAJOR_PARENTS: [&str; 3] = ["channels", "guilds", "webhooks"];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteKey {
	key: String,
	major: String,
}

impl RouteKey {
	pub fn as_str(&self) -> &str {
		&self.key
	}

	pub fn major(&self) -> &str {
		&self.major
	}

	pub fn is_message_send(&self) -> bool {
		let Some(path) = self.key.strip_prefix("POST /") else {
			return false;
		};
		self.major.starts_with("channels/")
			&& ["/messages", "/threads", "/messages/{id}/threads"]
				.iter()
				.any(|suffix| path.strip_suffix(suffix) == Some(self.major.as_str()))
	}

	pub fn is_interaction(&self) -> bool {
		self.key == "POST /interactions"
	}

	pub fn is_message_delete(&self) -> bool {
		self.major.starts_with("channels/")
			&& self
				.key
				.strip_prefix("DELETE /")
				.and_then(|path| path.strip_suffix("/messages/{id}"))
				== Some(self.major.as_str())
	}

	pub fn is_bulk_ack(&self) -> bool {
		self.key == "POST /read-states/ack-bulk"
	}

	pub fn is_user_profile(&self) -> bool {
		self.key == "GET /users/{id}/profile"
	}
}

impl fmt::Display for RouteKey {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str(&self.key)
	}
}

fn is_id(segment: &str) -> bool {
	!segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
}

pub fn route_key(method: &Method, path: &str) -> RouteKey {
	let mut key = String::with_capacity(path.len() + 8);
	let mut major = String::new();
	key.push_str(method.as_str());
	key.push(' ');
	let mut previous = "";
	for segment in path.split('/').filter(|segment| !segment.is_empty()) {
		key.push('/');
		if MAJOR_PARENTS.contains(&previous) && is_id(segment) {
			key.push_str(segment);
			if !major.is_empty() {
				major.push(',');
			}
			major.push_str(previous);
			major.push('/');
			major.push_str(segment);
		} else if previous == "reactions" {
			key.push_str("{emoji}");
		} else if previous == "invites" {
			key.push_str("{code}");
		} else if is_id(segment) {
			key.push_str("{id}");
		} else {
			key.push_str(segment);
		}
		previous = segment;
	}
	RouteKey { key, major }
}
