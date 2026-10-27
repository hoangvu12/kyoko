use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::client_hello::{ClientHello, Reader};
use super::ja4::ja4;
use super::names::{
	self, ALPN, ALPS, ALPS_OLD, COMPRESS_CERTIFICATE, EC_POINT_FORMATS, ENCRYPTED_CLIENT_HELLO, KEY_SHARE,
	PADDING, PRE_SHARED_KEY, PSK_KEY_EXCHANGE_MODES, SERVER_NAME, SIGNATURE_ALGORITHMS, STATUS_REQUEST,
	SUPPORTED_GROUPS, SUPPORTED_VERSIONS, TRUST_ANCHORS, is_grease,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsSummary {
	pub ja4: String,
	pub ja4_r: String,
	pub record_version: String,
	pub legacy_version: String,
	pub session_id_length: usize,
	pub ciphers: Vec<String>,
	pub compression_methods: Vec<u8>,
	pub extensions: Vec<String>,
	pub extension_set: Vec<String>,
	pub server_name: Option<String>,
	pub supported_versions: Vec<String>,
	pub supported_groups: Vec<String>,
	pub key_shares: Vec<KeyShare>,
	pub signature_algorithms: Vec<String>,
	pub alpn: Vec<String>,
	pub alps: Option<Alps>,
	pub certificate_compression: Vec<String>,
	pub ec_point_formats: Vec<u8>,
	pub psk_key_exchange_modes: Vec<u8>,
	pub status_request: Option<String>,
	pub ech: Option<Ech>,
	pub trust_anchors: Option<Vec<String>>,
	pub pre_shared_key: bool,
	pub other_extensions: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyShare {
	pub group: String,
	pub length: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alps {
	pub extension: String,
	pub protocols: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ech {
	pub kdf: String,
	pub aead: String,
	pub enc_length: usize,
}

const DECODED: [u16; 15] = [
	SERVER_NAME,
	STATUS_REQUEST,
	SUPPORTED_GROUPS,
	EC_POINT_FORMATS,
	SIGNATURE_ALGORITHMS,
	ALPN,
	PADDING,
	COMPRESS_CERTIFICATE,
	PRE_SHARED_KEY,
	SUPPORTED_VERSIONS,
	PSK_KEY_EXCHANGE_MODES,
	KEY_SHARE,
	ALPS_OLD,
	ALPS,
	ENCRYPTED_CLIENT_HELLO,
];

impl TlsSummary {
	pub fn of(hello: &ClientHello) -> TlsSummary {
		let fingerprint = ja4(hello);
		let kinds = hello
			.extensions
			.iter()
			.map(|extension| extension.kind)
			.collect::<Vec<_>>();
		let mut extension_set = kinds
			.iter()
			.copied()
			.filter(|kind| !is_grease(*kind))
			.collect::<Vec<_>>();
		extension_set.sort_unstable();
		TlsSummary {
			ja4: fingerprint.fingerprint,
			ja4_r: fingerprint.raw,
			record_version: names::hex(hello.record_version),
			legacy_version: names::hex(hello.legacy_version),
			session_id_length: hello.session_id_length,
			ciphers: hello.ciphers.iter().map(|value| names::cipher(*value)).collect(),
			compression_methods: hello.compression_methods.clone(),
			extensions: kinds.iter().map(|kind| names::extension(*kind)).collect(),
			extension_set: extension_set.into_iter().map(names::extension).collect(),
			server_name: server_name(hello),
			supported_versions: list8_u16(hello, SUPPORTED_VERSIONS, names::version),
			supported_groups: list16_u16(hello, SUPPORTED_GROUPS, names::group),
			key_shares: key_shares(hello),
			signature_algorithms: list16_u16(hello, SIGNATURE_ALGORITHMS, names::signature),
			alpn: protocols(hello, ALPN).unwrap_or_default(),
			alps: alps(hello),
			certificate_compression: list8_u16(hello, COMPRESS_CERTIFICATE, names::certificate_compression),
			ec_point_formats: bytes8(hello, EC_POINT_FORMATS),
			psk_key_exchange_modes: bytes8(hello, PSK_KEY_EXCHANGE_MODES),
			status_request: hello.extension(STATUS_REQUEST).map(hex::encode),
			ech: ech(hello),
			trust_anchors: trust_anchors(hello),
			pre_shared_key: hello.extension(PRE_SHARED_KEY).is_some(),
			other_extensions: hello
				.extensions
				.iter()
				.filter(|extension| {
					!is_grease(extension.kind)
						&& !DECODED.contains(&extension.kind)
						&& extension.kind != TRUST_ANCHORS
				})
				.map(|extension| (names::extension(extension.kind), hex::encode(&extension.data)))
				.collect(),
		}
	}
}

fn server_name(hello: &ClientHello) -> Option<String> {
	let mut list = Reader::new(Reader::new(hello.extension(SERVER_NAME)?).vec16().ok()?);
	while !list.is_empty() {
		let kind = list.u8().ok()?;
		let name = list.vec16().ok()?;
		if kind == 0 {
			return Some(String::from_utf8_lossy(name).into_owned());
		}
	}
	None
}

fn list16_u16(hello: &ClientHello, kind: u16, name: fn(u16) -> String) -> Vec<String> {
	hello
		.extension(kind)
		.and_then(|data| Reader::new(data).vec16().ok())
		.and_then(|list| Reader::new(list).u16s().ok())
		.unwrap_or_default()
		.into_iter()
		.map(name)
		.collect()
}

fn list8_u16(hello: &ClientHello, kind: u16, name: fn(u16) -> String) -> Vec<String> {
	hello
		.extension(kind)
		.and_then(|data| Reader::new(data).vec8().ok())
		.and_then(|list| Reader::new(list).u16s().ok())
		.unwrap_or_default()
		.into_iter()
		.map(name)
		.collect()
}

fn bytes8(hello: &ClientHello, kind: u16) -> Vec<u8> {
	hello
		.extension(kind)
		.and_then(|data| Reader::new(data).vec8().ok())
		.map(<[u8]>::to_vec)
		.unwrap_or_default()
}

fn protocols(hello: &ClientHello, kind: u16) -> Option<Vec<String>> {
	let list = Reader::new(hello.extension(kind)?).vec16().ok()?;
	Reader::new(list).strings8().ok()
}

fn alps(hello: &ClientHello) -> Option<Alps> {
	[ALPS, ALPS_OLD].into_iter().find_map(|kind| {
		protocols(hello, kind).map(|protocols| Alps {
			extension: names::extension(kind),
			protocols,
		})
	})
}

fn key_shares(hello: &ClientHello) -> Vec<KeyShare> {
	let Some(list) = hello
		.extension(KEY_SHARE)
		.and_then(|data| Reader::new(data).vec16().ok())
	else {
		return Vec::new();
	};
	let mut reader = Reader::new(list);
	let mut shares = Vec::new();
	while !reader.is_empty() {
		let (Ok(group), Ok(key)) = (reader.u16(), reader.vec16()) else {
			break;
		};
		shares.push(KeyShare {
			group: names::group(group),
			length: key.len(),
		});
	}
	shares
}

fn ech(hello: &ClientHello) -> Option<Ech> {
	let mut reader = Reader::new(hello.extension(ENCRYPTED_CLIENT_HELLO)?);
	if reader.u8().ok()? != 0 {
		return None;
	}
	let kdf = reader.u16().ok()?;
	let aead = reader.u16().ok()?;
	reader.u8().ok()?;
	let enc = reader.vec16().ok()?;
	Some(Ech {
		kdf: names::hex(kdf),
		aead: names::hex(aead),
		enc_length: enc.len(),
	})
}

fn trust_anchors(hello: &ClientHello) -> Option<Vec<String>> {
	let mut reader = Reader::new(Reader::new(hello.extension(TRUST_ANCHORS)?).vec16().ok()?);
	let mut ids = Vec::new();
	while !reader.is_empty() {
		ids.push(hex::encode(reader.vec8().ok()?));
	}
	Some(ids)
}
