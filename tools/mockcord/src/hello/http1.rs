use serde::{Deserialize, Serialize};

const RANDOM_HEADERS: [&str; 1] = ["sec-websocket-key"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http1Request {
	pub request_line: String,
	pub headers: Vec<(String, String)>,
}

impl Http1Request {
	pub fn parse(bytes: &[u8]) -> Option<Http1Request> {
		let end = bytes.windows(4).position(|window| window == b"\r\n\r\n")?;
		let text = String::from_utf8_lossy(&bytes[..end]);
		let mut lines = text.split("\r\n");
		let request_line = lines.next()?.to_owned();
		let headers = lines
			.filter_map(|line| line.split_once(':'))
			.map(|(name, value)| {
				let value = if RANDOM_HEADERS.contains(&name.trim().to_ascii_lowercase().as_str()) {
					"<random>".to_owned()
				} else {
					value.trim().to_owned()
				};
				(name.trim().to_owned(), value)
			})
			.collect();
		Some(Http1Request {
			request_line,
			headers,
		})
	}
}
