use std::io::{self, ErrorKind};
use std::net::IpAddr;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::crypto::{EncryptionMode, PacketCipher};
use crate::error::{Result, VoiceError};
use crate::rtp::{self, Header};

const DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const DISCOVERY_ATTEMPTS: usize = 3;
const DISCOVERY_PACKET_BYTES: usize = 74;
const DISCOVERY_LENGTH_FIELD: u16 = 70;
const DISCOVERY_REQUEST: u16 = 1;
const DISCOVERY_RESPONSE: u16 = 2;
const ADDRESS_RANGE: std::ops::Range<usize> = 8..72;
const PORT_START: usize = 72;
const RTCP_HEADER_LEN: usize = 8;
const PSFB_PAYLOAD_TYPE: u8 = 206;
const PLI_FORMAT: u8 = 1;
const FIR_FORMAT: u8 = 4;
const RTCP_FORMAT_MASK: u8 = 0x1f;
const SSRC_LEN: usize = 4;

pub(crate) enum Inbound {
	Media { header: Header, payload: Vec<u8> },
	KeyframeRequest { media_ssrc: u32 },
}

pub(crate) trait Transport: Send + Sync {
	fn ssrc(&self) -> u32;
	fn send(&self, header: &[u8], payload: &[u8]) -> bool;
	fn receive<'a>(&'a self, buffer: &'a mut [u8]) -> BoxFuture<'a, io::Result<Option<Inbound>>>;
}

pub(crate) struct Discovered {
	socket: UdpSocket,
	ssrc: u32,
	pub address: String,
	pub port: u16,
}

impl Discovered {
	pub async fn open(ip: &str, port: u16, ssrc: u32) -> Result<Self> {
		let media: IpAddr = ip
			.parse()
			.map_err(|_| VoiceError::Handshake("invalid media server address"))?;
		let socket = UdpSocket::bind(if media.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).await?;
		socket.connect((media, port)).await?;
		let (address, port) = discover(&socket, ssrc).await?;
		Ok(Self {
			socket,
			ssrc,
			address,
			port,
		})
	}

	pub fn secure(self, mode: EncryptionMode, key: &[u8]) -> Result<UdpTransport> {
		Ok(UdpTransport {
			socket: self.socket,
			ssrc: self.ssrc,
			cipher: PacketCipher::new(mode, key)?,
		})
	}
}

async fn discover(socket: &UdpSocket, ssrc: u32) -> Result<(String, u16)> {
	let mut request = [0u8; DISCOVERY_PACKET_BYTES];
	request[..2].copy_from_slice(&DISCOVERY_REQUEST.to_be_bytes());
	request[2..4].copy_from_slice(&DISCOVERY_LENGTH_FIELD.to_be_bytes());
	request[4..8].copy_from_slice(&ssrc.to_be_bytes());
	let mut response = [0u8; DISCOVERY_PACKET_BYTES];
	for _ in 0..DISCOVERY_ATTEMPTS {
		socket.send(&request).await?;
		let Ok(Ok(length)) = timeout(DISCOVERY_TIMEOUT, socket.recv(&mut response)).await else {
			continue;
		};
		if length < DISCOVERY_PACKET_BYTES || response[..2] != DISCOVERY_RESPONSE.to_be_bytes() {
			continue;
		}
		let text = &response[ADDRESS_RANGE];
		let end = text.iter().position(|byte| *byte == 0).unwrap_or(text.len());
		let address = String::from_utf8_lossy(&text[..end]).into_owned();
		let port = u16::from_be_bytes([response[PORT_START], response[PORT_START + 1]]);
		return Ok((address, port));
	}
	Err(VoiceError::IpDiscovery)
}

pub(crate) struct UdpTransport {
	socket: UdpSocket,
	ssrc: u32,
	cipher: PacketCipher,
}

impl UdpTransport {
	fn open(&self, packet: &[u8]) -> Option<Inbound> {
		if rtp::is_rtcp(packet) {
			return self.open_feedback(packet);
		}
		let header = rtp::parse(packet)?;
		let payload = self
			.cipher
			.open(packet, header.header_len)
			.and_then(|plain| header.into_payload(plain))?;
		Some(Inbound::Media { header, payload })
	}

	fn open_feedback(&self, packet: &[u8]) -> Option<Inbound> {
		let format = packet[0] & RTCP_FORMAT_MASK;
		if packet[1] != PSFB_PAYLOAD_TYPE || !matches!(format, PLI_FORMAT | FIR_FORMAT) {
			return None;
		}
		let body = self.cipher.open(packet, RTCP_HEADER_LEN)?;
		let ssrc_at = if format == FIR_FORMAT { SSRC_LEN } else { 0 };
		let media_ssrc = u32::from_be_bytes(body.get(ssrc_at..ssrc_at + SSRC_LEN)?.try_into().ok()?);
		Some(Inbound::KeyframeRequest { media_ssrc })
	}

	async fn next(&self, buffer: &mut [u8]) -> io::Result<Option<Inbound>> {
		match self.socket.recv(buffer).await {
			Ok(length) => Ok(self.open(&buffer[..length])),
			Err(error)
				if matches!(
					error.kind(),
					ErrorKind::ConnectionReset | ErrorKind::ConnectionRefused
				) =>
			{
				Ok(None)
			}
			Err(error) => Err(error),
		}
	}
}

impl Transport for UdpTransport {
	fn ssrc(&self) -> u32 {
		self.ssrc
	}

	fn send(&self, header: &[u8], payload: &[u8]) -> bool {
		self.cipher
			.seal(header, payload)
			.is_some_and(|packet| self.socket.try_send(&packet).is_ok())
	}

	fn receive<'a>(&'a self, buffer: &'a mut [u8]) -> BoxFuture<'a, io::Result<Option<Inbound>>> {
		self.next(buffer).boxed()
	}
}
