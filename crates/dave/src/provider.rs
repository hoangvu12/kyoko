use btls::aead::{AeadCtx, Algorithm};
use btls::hash::{MessageDigest, hash};
use btls::hmac::Hmac;
use btls_sys as ffi;
use openmls_memory_storage::MemoryStorage;
use openmls_traits::OpenMlsProvider;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::types::{
	AeadType, Ciphersuite, CryptoError, ExporterSecret, HashType, HpkeCiphertext, HpkeConfig, HpkeKeyPair,
	KemOutput, SignatureScheme,
};
use tls_codec::SecretVLBytes;

use crate::hpke::{self, Suite};
use crate::signer;

const SUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;
const DIGEST_BYTES: usize = 32;
const GCM_TAG_BYTES: usize = 16;

#[derive(Debug, Default)]
pub struct BoringCrypto;

#[derive(Debug, Default)]
pub struct Provider {
	crypto: BoringCrypto,
	storage: MemoryStorage,
}

impl OpenMlsProvider for Provider {
	type CryptoProvider = BoringCrypto;
	type RandProvider = BoringCrypto;
	type StorageProvider = MemoryStorage;

	fn storage(&self) -> &Self::StorageProvider {
		&self.storage
	}

	fn crypto(&self) -> &Self::CryptoProvider {
		&self.crypto
	}

	fn rand(&self) -> &Self::RandProvider {
		&self.crypto
	}
}

#[derive(Debug, thiserror::Error)]
#[error("BoringSSL could not produce random bytes")]
pub struct RandError;

impl OpenMlsRand for BoringCrypto {
	type Error = RandError;

	fn random_array<const N: usize>(&self) -> Result<[u8; N], Self::Error> {
		let mut out = [0; N];
		btls::rand::rand_bytes(&mut out).map_err(|_| RandError)?;
		Ok(out)
	}

	fn random_vec(&self, len: usize) -> Result<Vec<u8>, Self::Error> {
		let mut out = vec![0; len];
		btls::rand::rand_bytes(&mut out).map_err(|_| RandError)?;
		Ok(out)
	}
}

fn sha256(hash_type: HashType) -> Result<(), CryptoError> {
	match hash_type {
		HashType::Sha2_256 => Ok(()),
		_ => Err(CryptoError::UnsupportedHashAlgorithm),
	}
}

fn p256(scheme: SignatureScheme) -> Result<(), CryptoError> {
	match scheme {
		SignatureScheme::ECDSA_SECP256R1_SHA256 => Ok(()),
		_ => Err(CryptoError::UnsupportedSignatureScheme),
	}
}

fn aes_128_gcm(alg: AeadType, key: &[u8]) -> Result<AeadCtx, CryptoError> {
	match alg {
		AeadType::Aes128Gcm => AeadCtx::new(&Algorithm::aes_128_gcm(), key, GCM_TAG_BYTES)
			.map_err(|_| CryptoError::CryptoLibraryError),
		_ => Err(CryptoError::UnsupportedAeadAlgorithm),
	}
}

impl OpenMlsCrypto for BoringCrypto {
	fn supports(&self, ciphersuite: Ciphersuite) -> Result<(), CryptoError> {
		if ciphersuite == SUITE {
			Ok(())
		} else {
			Err(CryptoError::UnsupportedCiphersuite)
		}
	}

	fn supported_ciphersuites(&self) -> Vec<Ciphersuite> {
		vec![SUITE]
	}

	fn hkdf_extract(
		&self,
		hash_type: HashType,
		salt: &[u8],
		ikm: &[u8],
	) -> Result<SecretVLBytes, CryptoError> {
		sha256(hash_type)?;
		ffi::init();
		let mut out = vec![0; DIGEST_BYTES];
		let mut out_len = 0;
		let done = unsafe {
			ffi::HKDF_extract(
				out.as_mut_ptr(),
				&mut out_len,
				ffi::EVP_sha256(),
				ikm.as_ptr(),
				ikm.len(),
				salt.as_ptr(),
				salt.len(),
			)
		};
		if done != 1 {
			return Err(CryptoError::CryptoLibraryError);
		}
		out.truncate(out_len);
		Ok(out.into())
	}

	fn hmac(&self, hash_type: HashType, key: &[u8], message: &[u8]) -> Result<SecretVLBytes, CryptoError> {
		sha256(hash_type)?;
		let mut mac =
			Hmac::init(key, &MessageDigest::sha256()).map_err(|_| CryptoError::CryptoLibraryError)?;
		mac.update(message).map_err(|_| CryptoError::CryptoLibraryError)?;
		mac.finalize()
			.map(SecretVLBytes::from)
			.map_err(|_| CryptoError::CryptoLibraryError)
	}

	fn hkdf_expand(
		&self,
		hash_type: HashType,
		prk: &[u8],
		info: &[u8],
		okm_len: usize,
	) -> Result<SecretVLBytes, CryptoError> {
		sha256(hash_type)?;
		ffi::init();
		let mut out = vec![0; okm_len];
		let done = unsafe {
			ffi::HKDF_expand(
				out.as_mut_ptr(),
				out.len(),
				ffi::EVP_sha256(),
				prk.as_ptr(),
				prk.len(),
				info.as_ptr(),
				info.len(),
			)
		};
		if done != 1 {
			return Err(CryptoError::HkdfOutputLengthInvalid);
		}
		Ok(out.into())
	}

	fn hash(&self, hash_type: HashType, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
		sha256(hash_type)?;
		hash(MessageDigest::sha256(), data)
			.map(|digest| digest.to_vec())
			.map_err(|_| CryptoError::CryptoLibraryError)
	}

	fn aead_encrypt(
		&self,
		alg: AeadType,
		key: &[u8],
		data: &[u8],
		nonce: &[u8],
		aad: &[u8],
	) -> Result<Vec<u8>, CryptoError> {
		let mut context = aes_128_gcm(alg, key)?;
		let mut sealed = data.to_vec();
		let mut tag = [0; GCM_TAG_BYTES];
		let written = context
			.seal_in_place_mut(nonce, &mut sealed, &mut tag, aad)
			.map_err(|_| CryptoError::CryptoLibraryError)?
			.len();
		sealed.extend_from_slice(&tag[..written]);
		Ok(sealed)
	}

	fn aead_decrypt(
		&self,
		alg: AeadType,
		key: &[u8],
		ct_tag: &[u8],
		nonce: &[u8],
		aad: &[u8],
	) -> Result<Vec<u8>, CryptoError> {
		let mut context = aes_128_gcm(alg, key)?;
		let body = ct_tag
			.len()
			.checked_sub(GCM_TAG_BYTES)
			.ok_or(CryptoError::AeadDecryptionError)?;
		let (ciphertext, tag) = ct_tag.split_at(body);
		let mut opened = ciphertext.to_vec();
		context
			.open_in_place_mut(nonce, &mut opened, tag, aad)
			.map_err(|_| CryptoError::AeadDecryptionError)?;
		Ok(opened)
	}

	fn signature_key_gen(&self, alg: SignatureScheme) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
		p256(alg)?;
		let (private, public) = signer::generate()?;
		Ok((private.to_vec(), public))
	}

	fn verify_signature(
		&self,
		alg: SignatureScheme,
		data: &[u8],
		pk: &[u8],
		signature: &[u8],
	) -> Result<(), CryptoError> {
		p256(alg)?;
		signer::verify(pk, data, signature)
	}

	fn sign(&self, alg: SignatureScheme, data: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
		p256(alg)?;
		signer::sign(key, data)
	}

	fn hpke_seal(
		&self,
		config: HpkeConfig,
		pk_r: &[u8],
		info: &[u8],
		aad: &[u8],
		ptxt: &[u8],
	) -> Result<HpkeCiphertext, CryptoError> {
		let (kem_output, ciphertext) = hpke::seal(Suite::new(&config)?, pk_r, info, aad, ptxt)?;
		Ok(HpkeCiphertext {
			kem_output: kem_output.into(),
			ciphertext: ciphertext.into(),
		})
	}

	fn hpke_open(
		&self,
		config: HpkeConfig,
		input: &HpkeCiphertext,
		sk_r: &[u8],
		info: &[u8],
		aad: &[u8],
	) -> Result<Vec<u8>, CryptoError> {
		hpke::open(
			Suite::new(&config)?,
			input.kem_output.as_slice(),
			sk_r,
			info,
			aad,
			input.ciphertext.as_slice(),
		)
	}

	fn hpke_setup_sender_and_export(
		&self,
		config: HpkeConfig,
		pk_r: &[u8],
		info: &[u8],
		exporter_context: &[u8],
		exporter_length: usize,
	) -> Result<(KemOutput, ExporterSecret), CryptoError> {
		let (enc, secret) = hpke::sender_export(
			Suite::new(&config)?,
			pk_r,
			info,
			exporter_context,
			exporter_length,
		)?;
		Ok((enc, secret.into()))
	}

	fn hpke_setup_receiver_and_export(
		&self,
		config: HpkeConfig,
		enc: &[u8],
		sk_r: &[u8],
		info: &[u8],
		exporter_context: &[u8],
		exporter_length: usize,
	) -> Result<ExporterSecret, CryptoError> {
		hpke::recipient_export(
			Suite::new(&config)?,
			enc,
			sk_r,
			info,
			exporter_context,
			exporter_length,
		)
		.map(ExporterSecret::from)
	}

	fn derive_hpke_keypair(&self, config: HpkeConfig, ikm: &[u8]) -> Result<HpkeKeyPair, CryptoError> {
		let (private, public) = hpke::derive_key_pair(Suite::new(&config)?, ikm)?;
		Ok(HpkeKeyPair {
			private: private.as_slice().into(),
			public,
		})
	}
}
