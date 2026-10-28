use std::os::raw::c_int;

use btls_sys as ffi;
use openmls_traits::types::{CryptoError, HpkeAeadType, HpkeConfig, HpkeKdfType, HpkeKemType};
use zeroize::Zeroizing;

const MAX_ENC_BYTES: usize = ffi::EVP_HPKE_MAX_ENC_LENGTH as usize;
const MAX_PUBLIC_KEY_BYTES: usize = ffi::EVP_HPKE_MAX_PUBLIC_KEY_LENGTH as usize;
const MAX_PRIVATE_KEY_BYTES: usize = ffi::EVP_HPKE_MAX_PRIVATE_KEY_LENGTH as usize;

#[derive(Clone, Copy)]
pub(crate) struct Suite {
	kem: *const ffi::EVP_HPKE_KEM,
	kdf: *const ffi::EVP_HPKE_KDF,
	aead: *const ffi::EVP_HPKE_AEAD,
}

impl Suite {
	pub fn new(config: &HpkeConfig) -> Result<Self, CryptoError> {
		ffi::init();
		let kem = match config.0 {
			HpkeKemType::DhKemP256 => unsafe { ffi::EVP_hpke_p256_hkdf_sha256() },
			_ => return Err(CryptoError::UnsupportedCiphersuite),
		};
		let kdf = match config.1 {
			HpkeKdfType::HkdfSha256 => unsafe { ffi::EVP_hpke_hkdf_sha256() },
			_ => return Err(CryptoError::UnsupportedKdf),
		};
		let aead = match config.2 {
			HpkeAeadType::AesGcm128 => unsafe { ffi::EVP_hpke_aes_128_gcm() },
			_ => return Err(CryptoError::UnsupportedAeadAlgorithm),
		};
		Ok(Self { kem, kdf, aead })
	}
}

struct Context(*mut ffi::EVP_HPKE_CTX);

impl Context {
	fn new() -> Result<Self, CryptoError> {
		let context = unsafe { ffi::EVP_HPKE_CTX_new() };
		if context.is_null() {
			return Err(CryptoError::CryptoLibraryError);
		}
		Ok(Self(context))
	}

	fn sender(suite: Suite, public_key: &[u8], info: &[u8]) -> Result<(Self, Vec<u8>), CryptoError> {
		let context = Self::new()?;
		let mut enc = vec![0; MAX_ENC_BYTES];
		let mut enc_len = 0;
		let done = unsafe {
			ffi::EVP_HPKE_CTX_setup_sender(
				context.0,
				enc.as_mut_ptr(),
				&mut enc_len,
				enc.len(),
				suite.kem,
				suite.kdf,
				suite.aead,
				public_key.as_ptr(),
				public_key.len(),
				info.as_ptr(),
				info.len(),
			)
		};
		succeeded(done, CryptoError::SenderSetupError)?;
		enc.truncate(enc_len);
		Ok((context, enc))
	}

	#[cfg(test)]
	fn seeded_sender(
		suite: Suite,
		public_key: &[u8],
		info: &[u8],
		seed: &[u8],
	) -> Result<(Self, Vec<u8>), CryptoError> {
		let context = Self::new()?;
		let mut enc = vec![0; MAX_ENC_BYTES];
		let mut enc_len = 0;
		let done = unsafe {
			ffi::EVP_HPKE_CTX_setup_sender_with_seed_for_testing(
				context.0,
				enc.as_mut_ptr(),
				&mut enc_len,
				enc.len(),
				suite.kem,
				suite.kdf,
				suite.aead,
				public_key.as_ptr(),
				public_key.len(),
				info.as_ptr(),
				info.len(),
				seed.as_ptr(),
				seed.len(),
			)
		};
		succeeded(done, CryptoError::SenderSetupError)?;
		enc.truncate(enc_len);
		Ok((context, enc))
	}

	fn recipient(suite: Suite, enc: &[u8], private_key: &[u8], info: &[u8]) -> Result<Self, CryptoError> {
		let key = Key::from_private(suite, private_key)?;
		let context = Self::new()?;
		let done = unsafe {
			ffi::EVP_HPKE_CTX_setup_recipient(
				context.0,
				key.0,
				suite.kdf,
				suite.aead,
				enc.as_ptr(),
				enc.len(),
				info.as_ptr(),
				info.len(),
			)
		};
		succeeded(done, CryptoError::ReceiverSetupError)?;
		Ok(context)
	}

	fn seal(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
		let overhead = unsafe { ffi::EVP_HPKE_CTX_max_overhead(self.0) };
		let mut out = vec![0; plaintext.len() + overhead];
		let mut out_len = 0;
		let done = unsafe {
			ffi::EVP_HPKE_CTX_seal(
				self.0,
				out.as_mut_ptr(),
				&mut out_len,
				out.len(),
				plaintext.as_ptr(),
				plaintext.len(),
				aad.as_ptr(),
				aad.len(),
			)
		};
		succeeded(done, CryptoError::HpkeEncryptionError)?;
		out.truncate(out_len);
		Ok(out)
	}

	fn open(&mut self, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
		let mut out = vec![0; ciphertext.len()];
		let mut out_len = 0;
		let done = unsafe {
			ffi::EVP_HPKE_CTX_open(
				self.0,
				out.as_mut_ptr(),
				&mut out_len,
				out.len(),
				ciphertext.as_ptr(),
				ciphertext.len(),
				aad.as_ptr(),
				aad.len(),
			)
		};
		succeeded(done, CryptoError::HpkeDecryptionError)?;
		out.truncate(out_len);
		Ok(out)
	}

	fn export(&self, context: &[u8], length: usize) -> Result<Vec<u8>, CryptoError> {
		let mut secret = vec![0; length];
		let done = unsafe {
			ffi::EVP_HPKE_CTX_export(
				self.0,
				secret.as_mut_ptr(),
				secret.len(),
				context.as_ptr(),
				context.len(),
			)
		};
		succeeded(done, CryptoError::ExporterError)?;
		Ok(secret)
	}
}

impl Drop for Context {
	fn drop(&mut self) {
		unsafe { ffi::EVP_HPKE_CTX_free(self.0) }
	}
}

struct Key(*mut ffi::EVP_HPKE_KEY);

impl Key {
	fn new() -> Result<Self, CryptoError> {
		let key = unsafe { ffi::EVP_HPKE_KEY_new() };
		if key.is_null() {
			return Err(CryptoError::CryptoLibraryError);
		}
		Ok(Self(key))
	}

	fn from_private(suite: Suite, private_key: &[u8]) -> Result<Self, CryptoError> {
		let key = Self::new()?;
		let done =
			unsafe { ffi::EVP_HPKE_KEY_init(key.0, suite.kem, private_key.as_ptr(), private_key.len()) };
		succeeded(done, CryptoError::InvalidLength)?;
		Ok(key)
	}

	fn derive(suite: Suite, ikm: &[u8]) -> Result<Self, CryptoError> {
		let key = Self::new()?;
		let done = unsafe { ffi::EVP_HPKE_KEY_derive(key.0, suite.kem, ikm.as_ptr(), ikm.len()) };
		succeeded(done, CryptoError::CryptoLibraryError)?;
		Ok(key)
	}

	fn public(&self) -> Result<Vec<u8>, CryptoError> {
		let mut out = vec![0; MAX_PUBLIC_KEY_BYTES];
		let mut out_len = 0;
		let done = unsafe { ffi::EVP_HPKE_KEY_public_key(self.0, out.as_mut_ptr(), &mut out_len, out.len()) };
		succeeded(done, CryptoError::CryptoLibraryError)?;
		out.truncate(out_len);
		Ok(out)
	}

	fn private(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
		let mut out = Zeroizing::new(vec![0; MAX_PRIVATE_KEY_BYTES]);
		let mut out_len = 0;
		let done =
			unsafe { ffi::EVP_HPKE_KEY_private_key(self.0, out.as_mut_ptr(), &mut out_len, out.len()) };
		succeeded(done, CryptoError::CryptoLibraryError)?;
		out.truncate(out_len);
		Ok(out)
	}
}

impl Drop for Key {
	fn drop(&mut self) {
		unsafe { ffi::EVP_HPKE_KEY_free(self.0) }
	}
}

fn succeeded(done: c_int, error: CryptoError) -> Result<(), CryptoError> {
	if done == 1 { Ok(()) } else { Err(error) }
}

pub(crate) fn seal(
	suite: Suite,
	public_key: &[u8],
	info: &[u8],
	aad: &[u8],
	plaintext: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
	let (mut context, enc) = Context::sender(suite, public_key, info)?;
	let ciphertext = context.seal(aad, plaintext)?;
	Ok((enc, ciphertext))
}

pub(crate) fn open(
	suite: Suite,
	enc: &[u8],
	private_key: &[u8],
	info: &[u8],
	aad: &[u8],
	ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
	Context::recipient(suite, enc, private_key, info)?.open(aad, ciphertext)
}

pub(crate) fn sender_export(
	suite: Suite,
	public_key: &[u8],
	info: &[u8],
	context: &[u8],
	length: usize,
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
	let (sender, enc) = Context::sender(suite, public_key, info)?;
	Ok((enc, sender.export(context, length)?))
}

pub(crate) fn recipient_export(
	suite: Suite,
	enc: &[u8],
	private_key: &[u8],
	info: &[u8],
	context: &[u8],
	length: usize,
) -> Result<Vec<u8>, CryptoError> {
	Context::recipient(suite, enc, private_key, info)?.export(context, length)
}

pub(crate) fn derive_key_pair(
	suite: Suite,
	ikm: &[u8],
) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>), CryptoError> {
	let key = Key::derive(suite, ikm)?;
	Ok((key.private()?, key.public()?))
}

#[cfg(test)]
mod tests {
	use openmls_traits::types::{HpkeAeadType, HpkeConfig, HpkeKdfType, HpkeKemType};

	use super::{Context, Suite, derive_key_pair, open, recipient_export};

	const INFO: &str = "4f6465206f6e2061204772656369616e2055726e";
	const IKM_E: &str = "4270e54ffd08d79d5928020af4686d8f6b7d35dbe470265f1f5aa22816ce860e";
	const IKM_R: &str = "668b37171f1072f3cf12ea8a236a45df23fc13b82af3609ad1e354f6ef817550";
	const SK_R: &str = "f3ce7fdae57e1a310d87f1ebbde6f328be0a99cdbcadf4d6589cf29de4b8ffd2";
	const PK_R: &str = "04fe8c19ce0905191ebc298a9245792531f26f0cece2460639e8bc39cb7f706a826a779b4cf969b8a0e539c7f62fb3d30ad6aa8f80e30f1d128aafd68a2ce72ea0";
	const ENC: &str = "04a92719c6195d5085104f469a8b9814d5838ff72b60501e2c4466e5e67b325ac98536d7b61a1af4b78e5b7f951c0900be863c403ce65c9bfcb9382657222d18c4";
	const PT: &str = "4265617574792069732074727574682c20747275746820626561757479";
	const ENCRYPTIONS: [(&str, &str); 2] = [
		(
			"436f756e742d30",
			"5ad590bb8baa577f8619db35a36311226a896e7342a6d836d8b7bcd2f20b6c7f9076ac232e3ab2523f39513434",
		),
		(
			"436f756e742d31",
			"fa6f037b47fc21826b610172ca9637e82d6e5801eb31cbd3748271affd4ecb06646e0329cbdf3c3cd655b28e82",
		),
	];
	const EXPORTS: [(&str, &str); 3] = [
		(
			"",
			"5e9bc3d236e1911d95e65b576a8a86d478fb827e8bdfe77b741b289890490d4d",
		),
		(
			"00",
			"6cff87658931bda83dc857e6353efe4987a201b849658d9b047aab4cf216e796",
		),
		(
			"54657374436f6e74657874",
			"d8f1ea7942adbba7412c6d431c62d01371ea476b823eb697e1f6e6cae1dab85a",
		),
	];
	const EXPORT_BYTES: usize = 32;

	fn bytes(text: &str) -> Vec<u8> {
		hex::decode(text).expect("test vectors are hex")
	}

	fn suite() -> Suite {
		Suite::new(&HpkeConfig(
			HpkeKemType::DhKemP256,
			HpkeKdfType::HkdfSha256,
			HpkeAeadType::AesGcm128,
		))
		.expect("the DAVE suite is supported")
	}

	#[test]
	fn rfc_9180_base_p256_sha256_aes128gcm() {
		let suite = suite();
		let (private, public) = derive_key_pair(suite, &bytes(IKM_R)).expect("DeriveKeyPair");
		assert_eq!(private.as_slice(), bytes(SK_R));
		assert_eq!(public, bytes(PK_R));
		let (mut sender, enc) =
			Context::seeded_sender(suite, &public, &bytes(INFO), &bytes(IKM_E)).expect("SetupBaseS");
		assert_eq!(enc, bytes(ENC));
		for (aad, ciphertext) in ENCRYPTIONS {
			assert_eq!(
				sender.seal(&bytes(aad), &bytes(PT)).expect("seal"),
				bytes(ciphertext)
			);
		}
		let first = ENCRYPTIONS[0];
		let opened = open(
			suite,
			&enc,
			&private,
			&bytes(INFO),
			&bytes(first.0),
			&bytes(first.1),
		)
		.expect("open");
		assert_eq!(opened, bytes(PT));
		for (context, value) in EXPORTS {
			assert_eq!(
				sender.export(&bytes(context), EXPORT_BYTES).expect("export"),
				bytes(value)
			);
			let received =
				recipient_export(suite, &enc, &private, &bytes(INFO), &bytes(context), EXPORT_BYTES)
					.expect("receiver export");
			assert_eq!(received, bytes(value));
		}
	}
}
