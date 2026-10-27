use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const RANDOM_VALUES: [&str; 8] = [
	"ice-ufrag",
	"ice-pwd",
	"fingerprint",
	"msid",
	"ssrc",
	"candidate",
	"ssrc-group",
	"msid-semantic",
];
const NAMED_IDS: [&str; 2] = ["group", "mid"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferSummary {
	pub client: String,
	pub session: Vec<String>,
	pub media: Vec<MediaSummary>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaSummary {
	pub kind: String,
	pub protocol: String,
	pub attributes: Vec<String>,
	pub payloads: Vec<Payload>,
	pub extmaps: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
	pub pt: u8,
	pub rtpmap: String,
	pub fmtp: Option<String>,
	pub rtcp_fb: Vec<String>,
}

pub fn summarize(client: &str, sdp: &str) -> OfferSummary {
	let mut summary = OfferSummary {
		client: client.to_owned(),
		..OfferSummary::default()
	};
	for line in sdp.lines().map(str::trim_end) {
		if let Some(media) = line.strip_prefix("m=") {
			summary.media.push(media_line(media));
			continue;
		}
		let Some(attribute) = line.strip_prefix("a=") else {
			if summary.media.is_empty() {
				summary
					.session
					.push(line.split('=').next().unwrap_or(line).to_owned());
			}
			continue;
		};
		let (name, value) = attribute.split_once(':').unwrap_or((attribute, ""));
		let Some(media) = summary.media.last_mut() else {
			summary.session.push(shown(name, value));
			continue;
		};
		match name {
			"rtpmap" | "fmtp" | "rtcp-fb" => payload_attribute(media, name, value),
			"extmap" => media.extmaps.push(value.to_owned()),
			_ => media.attributes.push(shown(name, value)),
		}
	}
	summary
}

fn media_line(line: &str) -> MediaSummary {
	let mut parts = line.split(' ');
	let kind = parts.next().unwrap_or_default().to_owned();
	parts.next();
	let protocol = parts.next().unwrap_or_default().to_owned();
	let payloads = parts
		.filter_map(|pt| pt.parse().ok())
		.map(|pt| Payload {
			pt,
			..Payload::default()
		})
		.collect();
	MediaSummary {
		kind,
		protocol,
		payloads,
		..MediaSummary::default()
	}
}

fn payload_attribute(media: &mut MediaSummary, name: &str, value: &str) {
	let (pt, rest) = value.split_once(' ').unwrap_or((value, ""));
	let Some(payload) = pt
		.parse::<u8>()
		.ok()
		.and_then(|pt| media.payloads.iter_mut().find(|payload| payload.pt == pt))
	else {
		media.attributes.push(shown(name, value));
		return;
	};
	match name {
		"rtpmap" => payload.rtpmap = rest.to_owned(),
		"fmtp" => payload.fmtp = Some(rest.to_owned()),
		_ => payload.rtcp_fb.push(rest.to_owned()),
	}
}

fn shown(name: &str, value: &str) -> String {
	if value.is_empty() {
		return name.to_owned();
	}
	if RANDOM_VALUES.contains(&name) {
		let kept = match name {
			"fingerprint" => value.split(' ').next().unwrap_or_default(),
			"ssrc" => value
				.split_once(' ')
				.map_or("", |(_, rest)| rest.split(':').next().unwrap_or("")),
			"ssrc-group" | "msid-semantic" => value.split_whitespace().next().unwrap_or_default(),
			_ => "",
		};
		return if kept.is_empty() {
			name.to_owned()
		} else {
			format!("{name}:{kept}")
		};
	}
	if NAMED_IDS.contains(&name) {
		let ids = value
			.split(' ')
			.map(|token| {
				if token.chars().all(|character| character.is_ascii_digit()) || token == "BUNDLE" {
					token
				} else {
					"*"
				}
			})
			.collect::<Vec<_>>();
		return format!("{name}:{}", ids.join(" "));
	}
	format!("{name}:{value}")
}

pub fn differences(expected: &OfferSummary, actual: &OfferSummary) -> Vec<String> {
	let mut found = Vec::new();
	if expected.session != actual.session {
		found.push(format!(
			"session\n  expected {:?}\n  actual   {:?}",
			expected.session, actual.session
		));
	}
	if expected.media.len() != actual.media.len() {
		found.push(format!(
			"media sections\n  expected {}\n  actual   {}",
			expected.media.len(),
			actual.media.len()
		));
	}
	for (index, (left, right)) in expected.media.iter().zip(&actual.media).enumerate() {
		let fields = [
			("kind", vec![left.kind.clone()], vec![right.kind.clone()]),
			(
				"protocol",
				vec![left.protocol.clone()],
				vec![right.protocol.clone()],
			),
			("attributes", left.attributes.clone(), right.attributes.clone()),
			("extmaps", left.extmaps.clone(), right.extmaps.clone()),
		];
		for (field, expected, actual) in fields {
			if expected != actual {
				found.push(format!(
					"media[{index}].{field}\n  expected {expected:?}\n  actual   {actual:?}"
				));
			}
		}
		let codecs = |media: &MediaSummary| {
			media
				.payloads
				.iter()
				.map(|payload| (payload.pt, payload.clone()))
				.collect::<BTreeMap<_, _>>()
		};
		let order = |media: &MediaSummary| {
			media
				.payloads
				.iter()
				.map(|payload| payload.pt)
				.collect::<Vec<_>>()
		};
		if order(left) != order(right) {
			found.push(format!(
				"media[{index}].payload_order\n  expected {:?}\n  actual   {:?}",
				order(left),
				order(right)
			));
		}
		let (left, right) = (codecs(left), codecs(right));
		for (pt, payload) in &left {
			if right.get(pt) != Some(payload) {
				found.push(format!(
					"media[{index}].pt {pt}\n  expected {payload:?}\n  actual   {:?}",
					right.get(pt)
				));
			}
		}
	}
	found
}
