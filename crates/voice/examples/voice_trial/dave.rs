use hmac::{Hmac, Mac};
use openmls::prelude::{
	BasicCredential, Ciphersuite, CredentialWithKey, GroupId, KeyPackage, MlsGroup, MlsGroupCreateConfig,
	MlsGroupJoinConfig, MlsMessageBodyIn, MlsMessageIn, OpenMlsProvider, PURE_PLAINTEXT_WIRE_FORMAT_POLICY,
	StagedWelcome,
};
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use sha2::Sha256;
use tls_codec::{DeserializeBytes, Serialize};

use crate::text;

const SUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;
const MEDIA_KEY_LABEL: &str = "Discord Secure Frames v0";
const KEY_BYTES: usize = 16;
const CHANNEL: u64 = 1_234;

struct Member {
	user_id: u64,
	provider: OpenMlsRustCrypto,
	signer: SignatureKeyPair,
	credential: CredentialWithKey,
}

impl Member {
	fn new(user_id: u64) -> Result<Member, String> {
		let signer = SignatureKeyPair::new(SUITE.signature_algorithm()).map_err(text)?;
		let credential = CredentialWithKey {
			credential: BasicCredential::new(user_id.to_be_bytes().to_vec()).into(),
			signature_key: signer.public().into(),
		};
		Ok(Member {
			user_id,
			provider: OpenMlsRustCrypto::default(),
			signer,
			credential,
		})
	}

	fn key_package(&self) -> Result<KeyPackage, String> {
		let bundle = KeyPackage::builder()
			.build(SUITE, &self.provider, &self.signer, self.credential.clone())
			.map_err(text)?;
		Ok(bundle.key_package().clone())
	}

	fn media_key(&self, group: &MlsGroup, sender: u64) -> Result<Vec<u8>, String> {
		group
			.export_secret(
				self.provider.crypto(),
				MEDIA_KEY_LABEL,
				&sender.to_le_bytes(),
				KEY_BYTES,
			)
			.map_err(text)
	}
}

pub fn two_members() -> Result<String, String> {
	let alice = Member::new(1)?;
	let bob = Member::new(2)?;
	let config = MlsGroupCreateConfig::builder()
		.ciphersuite(SUITE)
		.use_ratchet_tree_extension(true)
		.wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
		.build();
	let mut group = MlsGroup::new_with_group_id(
		&alice.provider,
		&alice.signer,
		&config,
		GroupId::from_slice(&CHANNEL.to_be_bytes()),
		alice.credential.clone(),
	)
	.map_err(text)?;
	let (_, welcome, _) = group
		.add_members(&alice.provider, &alice.signer, &[bob.key_package()?])
		.map_err(text)?;
	group.merge_pending_commit(&alice.provider).map_err(text)?;
	let bytes = welcome.tls_serialize_detached().map_err(text)?;
	let MlsMessageBodyIn::Welcome(welcome) = MlsMessageIn::tls_deserialize_exact_bytes(&bytes)
		.map_err(text)?
		.extract()
	else {
		return Err("the add did not produce a welcome".to_owned());
	};
	let join = MlsGroupJoinConfig::builder()
		.use_ratchet_tree_extension(true)
		.wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
		.build();
	let joined = StagedWelcome::new_from_welcome(&bob.provider, &join, welcome, None)
		.map_err(text)?
		.into_group(&bob.provider)
		.map_err(text)?;
	let sent = alice.media_key(&group, alice.user_id)?;
	let received = bob.media_key(&joined, alice.user_id)?;
	if sent != received {
		return Err("the two members derived different media keys".to_owned());
	}
	let mut mac = Hmac::<Sha256>::new_from_slice(&sent).map_err(text)?;
	mac.update(b"key");
	let ratchet = mac.finalize().into_bytes();
	Ok(format!(
		"epoch {}, {} members, {}-byte media key agreed, {}-byte ratchet step, welcome {} bytes",
		joined.epoch().as_u64(),
		joined.members().count(),
		sent.len(),
		ratchet.len(),
		bytes.len()
	))
}
