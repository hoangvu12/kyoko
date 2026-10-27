use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{AeadCore, AeadInOut, KeyInit, Nonce};
use chacha20poly1305::XChaCha20Poly1305;

use crate::text;

const KEY: [u8; 32] = [7; 32];
const HEADER: [u8; 12] = [0x80, 0x78, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1];
const PAYLOAD: [u8; 120] = [1; 120];
const COUNTER: [u8; 4] = [0, 0, 0, 1];

pub fn round_trip() -> Result<String, String> {
	let aes = Aes256Gcm::new_from_slice(&KEY).map_err(text)?;
	let xchacha = XChaCha20Poly1305::new_from_slice(&KEY).map_err(text)?;
	let aes_bytes = sealed(&aes)?;
	let xchacha_bytes = sealed(&xchacha)?;
	Ok(format!(
		"aead_aes256_gcm_rtpsize {aes_bytes} bytes, aead_xchacha20_poly1305_rtpsize {xchacha_bytes} bytes"
	))
}

fn sealed<A: AeadInOut>(cipher: &A) -> Result<usize, String> {
	let nonce = padded::<A>();
	let mut body = PAYLOAD.to_vec();
	cipher
		.encrypt_in_place(&nonce, &HEADER, &mut body)
		.map_err(text)?;
	let length = HEADER.len() + body.len() + COUNTER.len();
	cipher
		.decrypt_in_place(&nonce, &HEADER, &mut body)
		.map_err(text)?;
	if body != PAYLOAD {
		return Err("the payload did not survive a round trip".to_owned());
	}
	Ok(length)
}

fn padded<A: AeadCore>() -> Nonce<A> {
	let mut nonce = Nonce::<A>::default();
	nonce[..COUNTER.len()].copy_from_slice(&COUNTER);
	nonce
}
