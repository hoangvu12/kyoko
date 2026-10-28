use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{AeadCore, AeadInOut, KeyInit, Nonce};
use chacha20poly1305::XChaCha20Poly1305;
use tokio::net::UdpSocket;

use super::recording::{VoicePacket, VoiceRtcp};
use super::{AES_GCM_MODE, Voice, XCHACHA_MODE};

pub(super) const OPUS_FRAME_SAMPLES: u32 = 960;

const ECHO_SSRC_OFFSET: u32 = 1;
const DISCOVERY_SSRC: std::ops::Range<usize> = 4..8;
const TAG_LEN: usize = 16;
const COUNTER_LEN: usize = 4;
const RTP_HEADER_LEN: usize = 12;
const CSRC_LEN: usize = 4;
const EXTENSION_PREAMBLE_LEN: usize = 4;
const EXTENSION_FLAG: u8 = 0x10;
const CSRC_COUNT_MASK: u8 = 0x0f;
const RTCP_PAYLOAD_TYPES: std::ops::RangeInclusive<u8> = 192..=223;
const SSRC_RANGE: std::ops::Range<usize> = 8..12;
const RTCP_HEADER_LEN: usize = 8;
const RTCP_SENDER_RANGE: std::ops::Range<usize> = 4..8;
const RTCP_FORMAT_MASK: u8 = 0x1f;
const DISCOVERY_PACKET_LEN: usize = 74;
const DISCOVERY_LENGTH_FIELD: u16 = 70;
const DISCOVERY_REQUEST: u16 = 1;
const DISCOVERY_RESPONSE: u16 = 2;
const DISCOVERY_ADDRESS_START: usize = 8;
const DISCOVERY_PORT_START: usize = 72;
const UDP_BUFFER_BYTES: usize = 2048;

enum Cipher {
	Aes(Box<Aes256Gcm>),
	XChaCha(Box<XChaCha20Poly1305>),
}

impl Cipher {
	fn new(mode: &str, key: &[u8]) -> Option<Self> {
		match mode {
			AES_GCM_MODE => Aes256Gcm::new_from_slice(key)
				.ok()
				.map(|cipher| Self::Aes(Box::new(cipher))),
			XCHACHA_MODE => XChaCha20Poly1305::new_from_slice(key)
				.ok()
				.map(|cipher| Self::XChaCha(Box::new(cipher))),
			_ => None,
		}
	}

	fn open(&self, counter: &[u8], header: &[u8], body: &mut Vec<u8>) -> Option<()> {
		match self {
			Self::Aes(cipher) => cipher.decrypt_in_place(&padded::<Aes256Gcm>(counter), header, body),
			Self::XChaCha(cipher) => {
				cipher.decrypt_in_place(&padded::<XChaCha20Poly1305>(counter), header, body)
			}
		}
		.ok()
	}

	fn seal(&self, counter: &[u8], header: &[u8], body: &mut Vec<u8>) -> Option<()> {
		match self {
			Self::Aes(cipher) => cipher.encrypt_in_place(&padded::<Aes256Gcm>(counter), header, body),
			Self::XChaCha(cipher) => {
				cipher.encrypt_in_place(&padded::<XChaCha20Poly1305>(counter), header, body)
			}
		}
		.ok()
	}
}

pub(super) fn seal_packet(
	mode: &str,
	key: &[u8],
	counter: &[u8],
	header: &[u8],
	payload: &[u8],
) -> Option<Vec<u8>> {
	let cipher = Cipher::new(mode, key)?;
	let mut body = payload.to_vec();
	cipher.seal(counter, header, &mut body)?;
	let mut packet = header.to_vec();
	packet.extend_from_slice(&body);
	packet.extend_from_slice(counter);
	Some(packet)
}

fn padded<A: AeadCore>(counter: &[u8]) -> Nonce<A> {
	let mut nonce = Nonce::<A>::default();
	nonce[..COUNTER_LEN].copy_from_slice(counter);
	nonce
}

pub(super) async fn run(socket: Arc<UdpSocket>, voice: Arc<Voice>) {
	let mut shutdown = voice.shutdown.clone();
	let mut buffer = [0u8; UDP_BUFFER_BYTES];
	loop {
		tokio::select! {
			received = socket.recv_from(&mut buffer) => {
				let Ok((length, from)) = received else {
					continue;
				};
				let packet = &buffer[..length];
				if is_discovery(packet) {
					let ssrc = u32::from_be_bytes(packet[DISCOVERY_SSRC].try_into().unwrap_or_default());
					voice.clients.lock().insert(ssrc, from);
					let _ = socket.send_to(&discovery_reply(packet, from), from).await;
				} else if let Some(reply) = echoed(&voice, packet) {
					if voice.echo_held.load(Ordering::Relaxed) {
						voice.held_echo.lock().push((reply, from));
					} else {
						let _ = socket.send_to(&reply, from).await;
					}
				}
			}
			() = voice.echo_released.notified() => {
				let held = std::mem::take(&mut *voice.held_echo.lock());
				for (reply, to) in held {
					let _ = socket.send_to(&reply, to).await;
				}
			}
			_ = shutdown.changed() => break,
		}
	}
}

fn is_discovery(packet: &[u8]) -> bool {
	packet.len() == DISCOVERY_PACKET_LEN && packet[..2] == DISCOVERY_REQUEST.to_be_bytes()
}

fn discovery_reply(request: &[u8], from: SocketAddr) -> Vec<u8> {
	let mut reply = vec![0u8; DISCOVERY_PACKET_LEN];
	reply[..2].copy_from_slice(&DISCOVERY_RESPONSE.to_be_bytes());
	reply[2..4].copy_from_slice(&DISCOVERY_LENGTH_FIELD.to_be_bytes());
	reply[4..8].copy_from_slice(&request[4..8]);
	let address = from.ip().to_string();
	reply[DISCOVERY_ADDRESS_START..DISCOVERY_ADDRESS_START + address.len()]
		.copy_from_slice(address.as_bytes());
	reply[DISCOVERY_PORT_START..].copy_from_slice(&from.port().to_be_bytes());
	reply
}

fn header_len(packet: &[u8]) -> Option<usize> {
	let mut length = RTP_HEADER_LEN + CSRC_LEN * usize::from(packet.first()? & CSRC_COUNT_MASK);
	if packet[0] & EXTENSION_FLAG != 0 {
		length += EXTENSION_PREAMBLE_LEN;
	}
	Some(length)
}

fn record_rtcp(voice: &Voice, packet: &[u8]) -> Option<()> {
	if packet.len() < RTCP_HEADER_LEN + TAG_LEN + COUNTER_LEN {
		return None;
	}
	let sender_ssrc = u32::from_be_bytes(packet[RTCP_SENDER_RANGE].try_into().ok()?);
	let mode = voice.modes.lock().get(&sender_ssrc).cloned()?;
	let cipher = Cipher::new(&mode, &voice.key)?;
	let (sealed, counter) = packet.split_at(packet.len() - COUNTER_LEN);
	let (header, body) = sealed.split_at(RTCP_HEADER_LEN);
	let mut body = body.to_vec();
	cipher.open(counter, header, &mut body)?;
	voice.record(|log| {
		log.rtcp.push(VoiceRtcp {
			sender_ssrc,
			packet_type: header[1],
			format: header[0] & RTCP_FORMAT_MASK,
			body,
			at: Instant::now(),
		});
	});
	Some(())
}

fn echoed(voice: &Voice, packet: &[u8]) -> Option<Vec<u8>> {
	if packet.len() < RTP_HEADER_LEN {
		return None;
	}
	if RTCP_PAYLOAD_TYPES.contains(&packet[1]) {
		record_rtcp(voice, packet);
		return None;
	}
	let header_len = header_len(packet)?;
	if packet.len() < header_len + TAG_LEN + COUNTER_LEN {
		return None;
	}
	let ssrc = u32::from_be_bytes(packet[SSRC_RANGE].try_into().ok()?);
	let mode = voice.modes.lock().get(&ssrc).cloned()?;
	let cipher = Cipher::new(&mode, &voice.key)?;
	let (sealed, counter) = packet.split_at(packet.len() - COUNTER_LEN);
	let (header, body) = sealed.split_at(header_len);
	let mut payload = body.to_vec();
	cipher.open(counter, header, &mut payload)?;
	let opened = voice.open_dave(ssrc, &payload);
	voice.record(|log| {
		log.packets.push(VoicePacket {
			ssrc,
			mode: mode.clone(),
			payload: payload.clone(),
			opened,
			at: Instant::now(),
		});
	});
	let mut echo_header = header.to_vec();
	echo_header[SSRC_RANGE].copy_from_slice(&ssrc.wrapping_add(ECHO_SSRC_OFFSET).to_be_bytes());
	cipher.seal(counter, &echo_header, &mut payload)?;
	echo_header.extend_from_slice(&payload);
	echo_header.extend_from_slice(counter);
	Some(echo_header)
}
