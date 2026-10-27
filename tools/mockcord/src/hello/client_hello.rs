use std::fmt;

const HANDSHAKE_RECORD: u8 = 22;
const CLIENT_HELLO: u8 = 1;
const RECORD_HEADER: usize = 5;
const HANDSHAKE_HEADER: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientHello {
	pub record_version: u16,
	pub legacy_version: u16,
	pub session_id_length: usize,
	pub ciphers: Vec<u16>,
	pub compression_methods: Vec<u8>,
	pub extensions: Vec<Extension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
	pub kind: u16,
	pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelloError {
	NotHandshake,
	NotClientHello,
	Truncated,
}

impl fmt::Display for HelloError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		let text = match self {
			HelloError::NotHandshake => "the first record is not a TLS handshake",
			HelloError::NotClientHello => "the first handshake message is not a ClientHello",
			HelloError::Truncated => "the ClientHello is cut short",
		};
		formatter.write_str(text)
	}
}

impl std::error::Error for HelloError {}

pub struct Records {
	pub consumed: usize,
	pub record_version: u16,
	pub message: Vec<u8>,
}

pub fn collect_records(bytes: &[u8]) -> Result<Option<Records>, HelloError> {
	let mut at = 0;
	let mut message = Vec::new();
	let mut record_version = None;
	loop {
		if let Some(wanted) = handshake_length(&message)
			&& message.len() >= wanted
		{
			message.truncate(wanted);
			return Ok(Some(Records {
				consumed: at,
				record_version: record_version.unwrap_or_default(),
				message,
			}));
		}
		let Some(header) = bytes.get(at..at + RECORD_HEADER) else {
			return Ok(None);
		};
		if header[0] != HANDSHAKE_RECORD {
			return Err(HelloError::NotHandshake);
		}
		record_version.get_or_insert(u16::from_be_bytes([header[1], header[2]]));
		let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
		let Some(body) = bytes.get(at + RECORD_HEADER..at + RECORD_HEADER + length) else {
			return Ok(None);
		};
		message.extend_from_slice(body);
		at += RECORD_HEADER + length;
	}
}

fn handshake_length(message: &[u8]) -> Option<usize> {
	let header = message.get(..HANDSHAKE_HEADER)?;
	let length = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
	Some(HANDSHAKE_HEADER + length)
}

impl ClientHello {
	pub fn parse(record_version: u16, message: &[u8]) -> Result<ClientHello, HelloError> {
		let mut reader = Reader::new(message);
		if reader.u8()? != CLIENT_HELLO {
			return Err(HelloError::NotClientHello);
		}
		let body = reader.vec24()?;
		let mut reader = Reader::new(body);
		let legacy_version = reader.u16()?;
		reader.take(32)?;
		let session_id_length = reader.vec8()?.len();
		let ciphers = Reader::new(reader.vec16()?).u16s()?;
		let compression_methods = reader.vec8()?.to_vec();
		let mut extensions = Vec::new();
		if !reader.is_empty() {
			let mut list = Reader::new(reader.vec16()?);
			while !list.is_empty() {
				let kind = list.u16()?;
				let data = list.vec16()?.to_vec();
				extensions.push(Extension { kind, data });
			}
		}
		Ok(ClientHello {
			record_version,
			legacy_version,
			session_id_length,
			ciphers,
			compression_methods,
			extensions,
		})
	}

	pub fn read(bytes: &[u8]) -> Result<Option<(usize, ClientHello)>, HelloError> {
		let Some(records) = collect_records(bytes)? else {
			return Ok(None);
		};
		let hello = ClientHello::parse(records.record_version, &records.message)?;
		Ok(Some((records.consumed, hello)))
	}

	pub fn extension(&self, kind: u16) -> Option<&[u8]> {
		self.extensions
			.iter()
			.find(|extension| extension.kind == kind)
			.map(|extension| extension.data.as_slice())
	}
}

pub struct Reader<'a> {
	bytes: &'a [u8],
}

impl<'a> Reader<'a> {
	pub fn new(bytes: &'a [u8]) -> Reader<'a> {
		Reader { bytes }
	}

	pub fn is_empty(&self) -> bool {
		self.bytes.is_empty()
	}

	pub fn take(&mut self, count: usize) -> Result<&'a [u8], HelloError> {
		if self.bytes.len() < count {
			return Err(HelloError::Truncated);
		}
		let (taken, rest) = self.bytes.split_at(count);
		self.bytes = rest;
		Ok(taken)
	}

	pub fn u8(&mut self) -> Result<u8, HelloError> {
		Ok(self.take(1)?[0])
	}

	pub fn u16(&mut self) -> Result<u16, HelloError> {
		let bytes = self.take(2)?;
		Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
	}

	pub fn vec8(&mut self) -> Result<&'a [u8], HelloError> {
		let length = usize::from(self.u8()?);
		self.take(length)
	}

	pub fn vec16(&mut self) -> Result<&'a [u8], HelloError> {
		let length = usize::from(self.u16()?);
		self.take(length)
	}

	pub fn vec24(&mut self) -> Result<&'a [u8], HelloError> {
		let bytes = self.take(3)?;
		let length = u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]) as usize;
		self.take(length)
	}

	pub fn u16s(mut self) -> Result<Vec<u16>, HelloError> {
		let mut values = Vec::new();
		while !self.is_empty() {
			values.push(self.u16()?);
		}
		Ok(values)
	}

	pub fn strings8(mut self) -> Result<Vec<String>, HelloError> {
		let mut values = Vec::new();
		while !self.is_empty() {
			values.push(String::from_utf8_lossy(self.vec8()?).into_owned());
		}
		Ok(values)
	}
}
