use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::h2::Http2Summary;
use super::http1::Http1Request;
use super::summary::TlsSummary;

const PERMUTED: &str = "extensions";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
	pub client: String,
	pub kind: String,
	pub negotiated_alpn: Option<String>,
	pub tls: TlsSummary,
	pub http2: Option<Http2Summary>,
	pub http1: Option<Http1Request>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
	pub field: String,
	pub expected: String,
	pub actual: String,
}

pub fn differences(expected: &Capture, actual: &Capture) -> Vec<Difference> {
	let mut found = field_differences(
		"tls",
		&to_value(&expected.tls),
		&to_value(&actual.tls),
		&[PERMUTED],
	);
	let grease = (
		grease_positions(&expected.tls.extensions),
		grease_positions(&actual.tls.extensions),
	);
	if grease.0 != grease.1 {
		found.push(Difference {
			field: "tls.extensions.grease_positions".to_owned(),
			expected: grease.0.join(","),
			actual: grease.1.join(","),
		});
	}
	if let (Some(expected), Some(actual)) = (&expected.http2, &actual.http2) {
		found.extend(http2_differences(expected, actual));
	}
	found
}

fn http2_differences(expected: &Http2Summary, actual: &Http2Summary) -> Vec<Difference> {
	let mut found = field_differences(
		"http2",
		&to_value(expected),
		&to_value(actual),
		&["requests", "akamai", "preface"],
	);
	if let (Some(expected), Some(actual)) = (expected.requests.first(), actual.requests.first()) {
		let pairs = [
			(
				"http2.requests[0].flags",
				to_value(&expected.flags),
				to_value(&actual.flags),
			),
			(
				"http2.requests[0].priority",
				to_value(&expected.priority),
				to_value(&actual.priority),
			),
			(
				"http2.requests[0].pseudo_order",
				to_value(&expected.pseudo_order),
				to_value(&actual.pseudo_order),
			),
		];
		found.extend(
			pairs
				.into_iter()
				.filter(|(_, expected, actual)| expected != actual)
				.map(|(field, expected, actual)| Difference {
					field: field.to_owned(),
					expected: expected.to_string(),
					actual: actual.to_string(),
				}),
		);
	}
	found
}

fn field_differences(prefix: &str, expected: &Value, actual: &Value, skipped: &[&str]) -> Vec<Difference> {
	let (Some(expected), Some(actual)) = (expected.as_object(), actual.as_object()) else {
		return Vec::new();
	};
	let mut keys = expected.keys().chain(actual.keys()).collect::<Vec<_>>();
	keys.sort_unstable();
	keys.dedup();
	keys.into_iter()
		.filter(|key| !skipped.contains(&key.as_str()))
		.filter_map(|key| {
			let left = expected.get(key).unwrap_or(&Value::Null);
			let right = actual.get(key).unwrap_or(&Value::Null);
			(left != right).then(|| Difference {
				field: format!("{prefix}.{key}"),
				expected: left.to_string(),
				actual: right.to_string(),
			})
		})
		.collect()
}

fn grease_positions(extensions: &[String]) -> Vec<&'static str> {
	let last = extensions.len().saturating_sub(1);
	extensions
		.iter()
		.enumerate()
		.filter(|(_, name)| *name == "GREASE")
		.map(|(index, _)| match index {
			0 => "first",
			index if index == last => "last",
			_ => "middle",
		})
		.collect()
}

fn to_value<T: Serialize>(value: &T) -> Value {
	serde_json::to_value(value).unwrap_or(Value::Null)
}
