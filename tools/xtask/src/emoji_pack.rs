use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use sha2::{Digest, Sha256};

#[derive(Args)]
pub struct TwemojiArgs {
	#[arg(
		long,
		help = "A downloaded copy of the pinned release tarball, instead of fetching it"
	)]
	archive: Option<PathBuf>,
}

pub fn run(args: TwemojiArgs) -> Result<()> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
	let tarball = match args.archive {
		Some(path) => fs::read(&path).with_context(|| format!("could not read {}", path.display()))?,
		None => download()?,
	};
	let digest = hex::encode(Sha256::digest(&tarball));
	if digest != twemoji::RELEASE_SHA256 {
		bail!(
			"the tarball's sha256 is {digest}, but Twemoji {} is pinned at {}",
			twemoji::RELEASE,
			twemoji::RELEASE_SHA256
		);
	}
	let files = svgs(&tarball)?;
	if files.is_empty() {
		bail!("the tarball holds no assets/svg files");
	}
	let packed = twemoji::pack(&files).context("could not pack the svgs")?;
	let out = root.join(twemoji::PACK_PATH);
	fs::write(&out, &packed).with_context(|| format!("could not write {}", out.display()))?;
	let raw: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
	println!(
		"Twemoji {}: {} svgs, {} bytes, packed into {} bytes at {}",
		twemoji::RELEASE,
		files.len(),
		raw,
		packed.len(),
		twemoji::PACK_PATH
	);
	Ok(())
}

fn download() -> Result<Vec<u8>> {
	let output = Command::new("curl")
		.args([
			"--silent",
			"--show-error",
			"--fail",
			"--location",
			twemoji::RELEASE_URL,
		])
		.output()
		.context("could not run curl")?;
	if !output.status.success() {
		bail!(
			"curl could not fetch {}: {}",
			twemoji::RELEASE_URL,
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}
	Ok(output.stdout)
}

fn svgs(tarball: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
	let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
	let mut files = Vec::new();
	for entry in archive.entries().context("the tarball is not a gzipped tar")? {
		let mut entry = entry?;
		let path = entry.path()?.into_owned();
		let Some(stem) = svg_stem(&path) else {
			continue;
		};
		let mut bytes = Vec::new();
		entry.read_to_end(&mut bytes)?;
		files.push((stem, bytes));
	}
	files.sort();
	Ok(files)
}

fn svg_stem(path: &Path) -> Option<String> {
	let mut parts = path.components().skip(1);
	let assets = parts.next()?.as_os_str().to_str()?;
	let folder = parts.next()?.as_os_str().to_str()?;
	let file = parts.next()?.as_os_str().to_str()?;
	if assets != "assets" || folder != "svg" || parts.next().is_some() {
		return None;
	}
	let stem = file.strip_suffix(".svg")?;
	let valid = !stem.is_empty()
		&& stem
			.bytes()
			.all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || byte == b'-');
	valid.then(|| stem.to_owned())
}
