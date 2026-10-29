use sha2::{Digest, Sha256};

use crate::error::{DenoiseError, Result};

pub(crate) fn verified(name: &'static str, bytes: &'static [u8], sha256: &str) -> Result<&'static [u8]> {
	let digest = Sha256::digest(bytes);
	let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
	if hex == sha256 {
		Ok(bytes)
	} else {
		Err(DenoiseError::Integrity(name))
	}
}
