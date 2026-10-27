use sha2::{Digest, Sha256};

use super::client_hello::{ClientHello, Reader};
use super::names::{ALPN, SERVER_NAME, SIGNATURE_ALGORITHMS, SUPPORTED_VERSIONS, is_grease};

pub struct Ja4 {
	pub fingerprint: String,
	pub raw: String,
}

pub fn ja4(hello: &ClientHello) -> Ja4 {
	let ciphers = without_grease(&hello.ciphers);
	let kinds = without_grease(
		&hello
			.extensions
			.iter()
			.map(|extension| extension.kind)
			.collect::<Vec<_>>(),
	);
	let mut sorted_ciphers = ciphers.clone();
	sorted_ciphers.sort_unstable();
	let mut hashed_extensions = kinds
		.iter()
		.copied()
		.filter(|kind| *kind != SERVER_NAME && *kind != ALPN)
		.collect::<Vec<_>>();
	hashed_extensions.sort_unstable();
	let signatures = hello
		.extension(SIGNATURE_ALGORITHMS)
		.and_then(|data| {
			Reader::new(data)
				.vec16()
				.ok()
				.and_then(|list| Reader::new(list).u16s().ok())
		})
		.map(|list| without_grease(&list))
		.unwrap_or_default();
	let prefix = format!(
		"{}{}{}{:02}{:02}{}",
		if is_dtls(hello) { 'd' } else { 't' },
		version(hello),
		if hello.extension(SERVER_NAME).is_some() {
			'd'
		} else {
			'i'
		},
		ciphers.len().min(99),
		kinds.len().min(99),
		alpn(hello),
	);
	let cipher_text = joined(&sorted_ciphers);
	let mut extension_text = joined(&hashed_extensions);
	if !signatures.is_empty() {
		extension_text.push('_');
		extension_text.push_str(&joined(&signatures));
	}
	Ja4 {
		fingerprint: format!(
			"{prefix}_{}_{}",
			short_hash(&cipher_text),
			short_hash(&extension_text)
		),
		raw: format!("{prefix}_{cipher_text}_{extension_text}"),
	}
}

fn without_grease(values: &[u16]) -> Vec<u16> {
	values
		.iter()
		.copied()
		.filter(|value| !is_grease(*value))
		.collect()
}

fn joined(values: &[u16]) -> String {
	values
		.iter()
		.map(|value| format!("{value:04x}"))
		.collect::<Vec<_>>()
		.join(",")
}

fn short_hash(text: &str) -> String {
	if text.is_empty() {
		return "000000000000".to_owned();
	}
	hex::encode(Sha256::digest(text.as_bytes()))[..12].to_owned()
}

fn version(hello: &ClientHello) -> &'static str {
	let highest = hello
		.extension(SUPPORTED_VERSIONS)
		.and_then(|data| {
			Reader::new(data)
				.vec8()
				.ok()
				.and_then(|list| Reader::new(list).u16s().ok())
		})
		.and_then(|list| {
			let versions = without_grease(&list).into_iter();
			if is_dtls(hello) {
				versions.min()
			} else {
				versions.max()
			}
		})
		.unwrap_or(hello.legacy_version);
	match highest {
		0x0304 => "13",
		0x0303 => "12",
		0x0302 => "11",
		0x0301 => "10",
		0xfefc => "d3",
		0xfefd => "d2",
		0xfeff => "d1",
		_ => "00",
	}
}

fn is_dtls(hello: &ClientHello) -> bool {
	hello.legacy_version >= 0xfe00
}

fn alpn(hello: &ClientHello) -> String {
	let first = hello
		.extension(ALPN)
		.and_then(|data| {
			Reader::new(data)
				.vec16()
				.ok()
				.and_then(|list| Reader::new(list).strings8().ok())
		})
		.and_then(|list| list.into_iter().next());
	let Some(first) = first else {
		return "00".to_owned();
	};
	let characters = first.chars().collect::<Vec<_>>();
	match (characters.first(), characters.last()) {
		(Some(head), Some(tail)) if head.is_ascii_alphanumeric() && tail.is_ascii_alphanumeric() => {
			format!("{head}{tail}")
		}
		_ => "99".to_owned(),
	}
}
