use std::collections::BTreeMap;

use super::client_hello::{ClientHello, HelloError, Reader};

const HANDSHAKE_RECORD: u8 = 22;
const CLIENT_HELLO: u8 = 1;
const RANDOM: usize = 32;

pub fn is_dtls(datagram: &[u8]) -> bool {
	datagram.first().is_some_and(|first| (20..=63).contains(first))
}

#[derive(Default)]
pub struct DtlsHelloReader {
	record_version: Option<u16>,
	length: Option<usize>,
	fragments: BTreeMap<usize, Vec<u8>>,
	pub datagrams: usize,
	pub bytes: usize,
}

impl DtlsHelloReader {
	pub fn feed(&mut self, datagram: &[u8]) -> Result<Option<ClientHello>, HelloError> {
		if !is_dtls(datagram) {
			return Ok(None);
		}
		let mut records = Reader::new(datagram);
		let mut used = false;
		while !records.is_empty() {
			let kind = records.u8()?;
			let version = records.u16()?;
			records.take(8)?;
			let fragment = records.vec16()?;
			if kind != HANDSHAKE_RECORD {
				continue;
			}
			let mut handshake = Reader::new(fragment);
			if handshake.u8()? != CLIENT_HELLO {
				continue;
			}
			let length = u24(handshake.take(3)?);
			handshake.take(2)?;
			let offset = u24(handshake.take(3)?);
			let piece = handshake.vec24()?;
			self.record_version.get_or_insert(version);
			self.length = Some(length);
			self.fragments.entry(offset).or_insert_with(|| piece.to_vec());
			used = true;
		}
		if used {
			self.datagrams += 1;
			self.bytes += datagram.len();
		}
		self.assembled()
	}

	fn assembled(&self) -> Result<Option<ClientHello>, HelloError> {
		let (Some(record_version), Some(length)) = (self.record_version, self.length) else {
			return Ok(None);
		};
		let mut message = Vec::with_capacity(length);
		for (offset, piece) in &self.fragments {
			if *offset > message.len() {
				return Ok(None);
			}
			let skip = message.len() - offset;
			message.extend_from_slice(piece.get(skip..).unwrap_or_default());
		}
		if message.len() < length {
			return Ok(None);
		}
		let tls_message = without_cookie(&message[..length])?;
		ClientHello::parse(record_version, &tls_message).map(Some)
	}
}

pub fn read_dtls_client_hello(datagram: &[u8]) -> Result<Option<ClientHello>, HelloError> {
	DtlsHelloReader::default().feed(datagram)
}

fn u24(bytes: &[u8]) -> usize {
	u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]) as usize
}

fn without_cookie(body: &[u8]) -> Result<Vec<u8>, HelloError> {
	let mut reader = Reader::new(body);
	let head = reader.take(2 + RANDOM)?;
	let session = reader.vec8()?;
	let cookie = reader.vec8()?;
	let rest = &body[head.len() + 1 + session.len() + 1 + cookie.len()..];
	let mut message = vec![CLIENT_HELLO];
	let length = head.len() + 1 + session.len() + rest.len();
	message.extend_from_slice(&(length as u32).to_be_bytes()[1..]);
	message.extend_from_slice(head);
	message.push(session.len() as u8);
	message.extend_from_slice(session);
	message.extend_from_slice(rest);
	Ok(message)
}
