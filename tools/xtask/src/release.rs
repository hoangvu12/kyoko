use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Subcommand;

#[derive(clap::Args)]
pub struct ReleaseArgs {
	#[command(subcommand)]
	command: ReleaseCommand,
}

#[derive(Subcommand)]
enum ReleaseCommand {
	Notes(NotesArgs),
}

#[derive(clap::Args)]
struct NotesArgs {
	#[arg(long)]
	since: Option<String>,
	#[arg(long)]
	out: Option<PathBuf>,
}

pub fn run(args: ReleaseArgs) -> Result<()> {
	match args.command {
		ReleaseCommand::Notes(args) => notes(args),
	}
}

fn notes(args: NotesArgs) -> Result<()> {
	let mut command = Command::new("git");
	command.args(["log", "--no-merges", "--pretty=format:%s"]);
	if let Some(tag) = &args.since {
		command.arg(format!("{tag}..HEAD"));
	}
	let output = command.output().context("run git log")?;
	if !output.status.success() {
		bail!(
			"git log failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}
	let mut text = String::from_utf8(output.stdout)?
		.lines()
		.filter(|line| !line.trim().is_empty())
		.map(|line| format!("- {}\n", line.trim()))
		.collect::<String>();
	if text.is_empty() {
		text.push_str("- no changes\n");
	}
	match &args.out {
		Some(path) => fs::write(path, text).with_context(|| format!("write {}", path.display())),
		None => {
			print!("{text}");
			Ok(())
		}
	}
}
