use protocol::{
	ProtoCustomStatus, ProtoGuildFolder, ProtoGuildFolders, ProtoStatusSettings, Snowflake,
	decode_guild_folders, decode_status_settings, encode_guild_folders, encode_status_settings,
};

fn sample() -> ProtoGuildFolders {
	ProtoGuildFolders {
		folders: vec![
			ProtoGuildFolder {
				id: None,
				name: None,
				color: None,
				guild_ids: vec![Snowflake(1_200_000_000_000_000_001)],
			},
			ProtoGuildFolder {
				id: Some(1_761_000_000_000),
				name: Some("Friends é".to_owned()),
				color: Some(0x5865f2),
				guild_ids: vec![Snowflake(1_200_000_000_000_000_002), Snowflake(3)],
			},
			ProtoGuildFolder {
				id: Some(-4),
				name: None,
				color: None,
				guild_ids: vec![Snowflake(u64::MAX)],
			},
		],
		positions: Vec::new(),
	}
}

#[test]
fn guild_folders_survive_an_encode_and_decode() {
	let folders = sample();
	let encoded = encode_guild_folders(&folders);
	assert_eq!(decode_guild_folders(&encoded), Some(folders));
}

#[test]
fn the_encoding_is_field_fourteen_with_packed_ids_and_wrapped_values() {
	let folders = ProtoGuildFolders {
		folders: vec![ProtoGuildFolder {
			id: Some(5),
			name: Some("A".to_owned()),
			color: Some(1),
			guild_ids: vec![Snowflake(2)],
		}],
		positions: Vec::new(),
	};
	let folder: Vec<u8> = [
		vec![0x0a, 8, 2, 0, 0, 0, 0, 0, 0, 0],
		vec![0x12, 2, 0x08, 5],
		vec![0x1a, 3, 0x0a, 1, b'A'],
		vec![0x22, 2, 0x08, 1],
	]
	.concat();
	let mut inner = vec![0x0a, folder.len() as u8];
	inner.extend(folder);
	let mut expected = vec![0x72, inner.len() as u8];
	expected.extend(inner);
	let expected = base64_standard(&expected);
	assert_eq!(encode_guild_folders(&folders), expected);
}

#[test]
fn an_empty_layout_still_names_the_guild_folders_field() {
	let encoded = encode_guild_folders(&ProtoGuildFolders::default());
	assert_eq!(encoded, "cgA=");
	assert_eq!(decode_guild_folders(&encoded), Some(ProtoGuildFolders::default()));
}

#[test]
fn status_settings_survive_an_encode_and_decode_keeping_unknown_fields() {
	let settings = ProtoStatusSettings {
		status: Some("dnd".to_owned()),
		custom_status: Some(ProtoCustomStatus {
			text: "brb coffee".to_owned(),
			emoji_id: None,
			emoji_name: Some("\u{2615}".to_owned()),
			expires_at_ms: Some(1_790_000_000_000),
			created_at_ms: Some(1_780_000_000_000),
		}),
		status_expires_at_ms: None,
		other: vec![0x1a, 2, 0x08, 1],
	};
	let encoded = encode_status_settings(&settings);
	assert_eq!(decode_status_settings(&encoded), Some(settings));
	assert_eq!(decode_guild_folders(&encoded), None);
}

#[test]
fn status_settings_are_field_eleven_with_a_wrapped_status() {
	let settings = ProtoStatusSettings {
		status: Some("idle".to_owned()),
		..ProtoStatusSettings::default()
	};
	let expected = base64_standard(&[0x5a, 8, 0x0a, 6, 0x0a, 4, b'i', b'd', b'l', b'e']);
	assert_eq!(encode_status_settings(&settings), expected);
}

fn base64_standard(bytes: &[u8]) -> String {
	const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
	let mut out = String::new();
	for chunk in bytes.chunks(3) {
		let block = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
		let value = u32::from(block[0]) << 16 | u32::from(block[1]) << 8 | u32::from(block[2]);
		for index in 0..4 {
			if index <= chunk.len() {
				out.push(ALPHABET[(value >> (18 - 6 * index) & 63) as usize] as char);
			} else {
				out.push('=');
			}
		}
	}
	out
}
