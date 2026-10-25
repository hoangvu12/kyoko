use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use serde_json::Value;

const MAX_ID_LENGTH: usize = 64;
const BASES: [&str; 3] = ["dark", "light", "amoled"];
const FIELDS: [&str; 4] = ["name", "version", "author", "description"];

#[derive(Args)]
pub struct ThemeArgs {
	#[command(subcommand)]
	command: ThemeCommand,
}

#[derive(Subcommand)]
enum ThemeCommand {
	#[command(about = "Check every first party theme folder")]
	Check(CheckArgs),
}

#[derive(Args)]
struct CheckArgs {
	#[arg(long, default_value = "themes")]
	dir: PathBuf,
}

pub fn run(args: ThemeArgs) -> Result<()> {
	match args.command {
		ThemeCommand::Check(args) => check(&args),
	}
}

fn check(args: &CheckArgs) -> Result<()> {
	let mut count = 0;
	let mut entries: Vec<PathBuf> = fs::read_dir(&args.dir)
		.with_context(|| format!("could not read {}", args.dir.display()))?
		.flatten()
		.map(|entry| entry.path())
		.filter(|path| path.is_dir())
		.collect();
	entries.sort();
	for folder in entries {
		check_folder(&folder)?;
		count += 1;
	}
	println!("{count} themes in {} are valid", args.dir.display());
	Ok(())
}

fn check_folder(folder: &Path) -> Result<()> {
	let meta_path = folder.join("theme.toml");
	let meta: toml::Table = fs::read_to_string(&meta_path)
		.with_context(|| format!("could not read {}", meta_path.display()))?
		.parse()
		.with_context(|| format!("invalid {}", meta_path.display()))?;
	let field = |name: &str| -> Result<&str> {
		meta.get(name)
			.and_then(|value| value.as_str())
			.filter(|value| !value.trim().is_empty())
			.with_context(|| format!("{} needs a `{name}` string", meta_path.display()))
	};
	let id = field("id")?;
	ensure!(
		valid_id(id),
		"{}: id `{id}` must match [a-z0-9-]{{1,{MAX_ID_LENGTH}}}",
		meta_path.display()
	);
	let folder_name = folder
		.file_name()
		.and_then(|name| name.to_str())
		.unwrap_or_default();
	ensure!(
		id == folder_name,
		"{}: id `{id}` must match the folder name",
		meta_path.display()
	);
	for name in FIELDS {
		field(name)?;
	}
	let theme_path = folder.join("theme.json");
	let bytes = fs::read(&theme_path).with_context(|| format!("could not read {}", theme_path.display()))?;
	ensure!(
		bytes.len() <= session::MAX_THEME_BYTES,
		"{} is larger than {} KB",
		theme_path.display(),
		session::MAX_THEME_BYTES / 1024
	);
	let Ok(Value::Object(tokens)) = serde_json::from_slice::<Value>(&bytes) else {
		bail!("{} must be a JSON object", theme_path.display());
	};
	if let Some(base) = tokens.get("base") {
		ensure!(
			base.as_str().is_some_and(|base| BASES.contains(&base)),
			"{}: base must be one of {}",
			theme_path.display(),
			BASES.join(", ")
		);
	}
	Ok(())
}

fn valid_id(id: &str) -> bool {
	(1..=MAX_ID_LENGTH).contains(&id.len())
		&& id
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
