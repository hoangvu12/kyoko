use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::Args;
use ed25519_dalek::{Signer, SigningKey, Verifier};

#[derive(Args)]
pub struct KeygenArgs {
	out: PathBuf,
}

#[derive(Args)]
pub struct SignArgs {
	catalog: PathBuf,
	key: PathBuf,
}

pub fn keygen(args: &KeygenArgs) -> Result<()> {
	let key = SigningKey::from_bytes(&rand::random());
	let mut file = fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&args.out)
		.with_context(|| format!("could not create {}", args.out.display()))?;
	writeln!(file, "{}", STANDARD.encode(key.to_bytes()))?;
	println!("{}", STANDARD.encode(key.verifying_key().to_bytes()));
	Ok(())
}

fn read_signing_key(path: &Path) -> Result<SigningKey> {
	let text = fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
	let bytes = STANDARD
		.decode(text.trim())
		.with_context(|| format!("{} is not base64", path.display()))?;
	let seed = <[u8; 32]>::try_from(bytes.as_slice())
		.map_err(|_| anyhow!("{} must hold a 32 byte key", path.display()))?;
	Ok(SigningKey::from_bytes(&seed))
}

pub fn sign(args: &SignArgs) -> Result<()> {
	let bytes =
		fs::read(&args.catalog).with_context(|| format!("could not read {}", args.catalog.display()))?;
	let key = read_signing_key(&args.key)?;
	let signature = key.sign(&bytes);
	key.verifying_key().verify(&bytes, &signature)?;
	let signature = STANDARD.encode(signature.to_bytes());
	let mut signature_path = OsString::from(&args.catalog);
	signature_path.push(".sig");
	let signature_path = PathBuf::from(signature_path);
	fs::write(&signature_path, signature + "\n")
		.with_context(|| format!("could not write {}", signature_path.display()))?;
	println!("{}", signature_path.display());
	Ok(())
}
