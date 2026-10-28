use std::collections::{BTreeMap, HashMap};

use protocol::Snowflake;
use serde_json::{Map, Value, json};

use super::VideoWant;
use super::hub::RemoteVideo;

pub const FULL_QUALITY: u8 = 100;
const OFF: u8 = 0;

pub fn sink_wants(remote: &HashMap<Snowflake, RemoteVideo>, wants: &[VideoWant]) -> Value {
	let mut qualities = BTreeMap::new();
	let mut pixels = BTreeMap::new();
	for (user_id, video) in remote {
		let want = wants.iter().find(|want| want.user_id == *user_id);
		let quality = want.map_or(OFF, |want| want.quality.min(FULL_QUALITY));
		qualities.insert(video.primary, quality);
		if let Some(want) = want.filter(|want| quality > OFF && want.pixels > 0) {
			pixels.insert(video.primary, want.pixels);
		}
	}
	let mut data = Map::new();
	for (ssrc, quality) in qualities {
		data.insert(ssrc.to_string(), json!(quality));
	}
	data.insert("any".to_owned(), json!(FULL_QUALITY));
	if !pixels.is_empty() {
		let counts: Map<String, Value> = pixels
			.into_iter()
			.map(|(ssrc, count)| (ssrc.to_string(), json!(count)))
			.collect();
		data.insert("pixelCounts".to_owned(), Value::Object(counts));
	}
	Value::Object(data)
}
