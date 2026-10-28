use btls::bn::{BigNum, BigNumContext};
use btls::ec::{EcGroup, EcKey, EcPoint, PointConversionForm};
use btls::ecdsa::EcdsaSig;
use btls::hash::{MessageDigest, hash};
use btls::nid::Nid;
use openmls_traits::signatures::{Signer, SignerError};
use openmls_traits::types::{CryptoError, SignatureScheme};
use zeroize::Zeroizing;

const SCALAR_BYTES: usize = 32;

pub struct SigningKey {
	private: Zeroizing<Vec<u8>>,
	public: Vec<u8>,
}

impl SigningKey {
	pub fn generate() -> Result<Self, CryptoError> {
		let (private, public) = generate()?;
		Ok(Self { private, public })
	}

	pub fn public(&self) -> &[u8] {
		&self.public
	}
}

impl Signer for SigningKey {
	fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
		sign(&self.private, payload).map_err(SignerError::CryptoError)
	}

	fn signature_scheme(&self) -> SignatureScheme {
		SignatureScheme::ECDSA_SECP256R1_SHA256
	}
}

fn library<E>(_: E) -> CryptoError {
	CryptoError::CryptoLibraryError
}

fn p256() -> Result<EcGroup, CryptoError> {
	EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(library)
}

pub(crate) fn generate() -> Result<(Zeroizing<Vec<u8>>, Vec<u8>), CryptoError> {
	let group = p256()?;
	let key = EcKey::generate(&group).map_err(library)?;
	let private = Zeroizing::new(key.private_key().to_vec_padded(SCALAR_BYTES).map_err(library)?);
	let mut context = BigNumContext::new().map_err(library)?;
	let public = key
		.public_key()
		.to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut context)
		.map_err(library)?;
	Ok((private, public))
}

pub(crate) fn sign(private: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
	if private.len() != SCALAR_BYTES {
		return Err(CryptoError::InvalidLength);
	}
	let group = p256()?;
	let mut context = BigNumContext::new().map_err(library)?;
	let scalar = BigNum::from_slice(private).map_err(library)?;
	let mut point = EcPoint::new(&group).map_err(library)?;
	point
		.mul_generator(&group, &scalar, &mut context)
		.map_err(library)?;
	let key = EcKey::from_private_components(&group, &scalar, &point).map_err(library)?;
	let digest = hash(MessageDigest::sha256(), data).map_err(library)?;
	EcdsaSig::sign(&digest, &key)
		.and_then(|signature| signature.to_der())
		.map_err(|_| CryptoError::SigningError)
}

pub(crate) fn verify(public: &[u8], data: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
	let group = p256()?;
	let mut context = BigNumContext::new().map_err(library)?;
	let point =
		EcPoint::from_bytes(&group, public, &mut context).map_err(|_| CryptoError::InvalidPublicKey)?;
	let key = EcKey::from_public_key(&group, &point).map_err(|_| CryptoError::InvalidPublicKey)?;
	let signature = EcdsaSig::from_der(signature).map_err(|_| CryptoError::InvalidSignature)?;
	let digest = hash(MessageDigest::sha256(), data).map_err(library)?;
	match signature.verify(&digest, &key) {
		Ok(true) => Ok(()),
		_ => Err(CryptoError::InvalidSignature),
	}
}
