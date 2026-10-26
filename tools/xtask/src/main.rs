mod checks;
mod emoji_pack;
mod headless;
mod parity;
mod release;
mod theme;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Kyoko development tasks")]
struct Cli {
	#[command(subcommand)]
	task: Task,
}

#[derive(Subcommand)]
enum Task {
	HeadlessRun(headless::Args),
	Parity(parity::Args),
	Release(release::ReleaseArgs),
	Theme(theme::ThemeArgs),
	Twemoji(emoji_pack::TwemojiArgs),
	FmtCheck,
	Lint,
	Test,
	CheckAll,
}

fn main() -> ExitCode {
	let cli = Cli::parse();
	let outcome = match cli.task {
		Task::HeadlessRun(args) => headless::run(args),
		Task::Parity(args) => parity::run(args),
		Task::Release(args) => release::run(args),
		Task::Theme(args) => theme::run(args),
		Task::Twemoji(args) => emoji_pack::run(args),
		Task::FmtCheck => checks::fmt_check(),
		Task::Lint => checks::lint(),
		Task::Test => checks::test(),
		Task::CheckAll => checks::check_all(),
	};
	match outcome {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("xtask: {error:#}");
			ExitCode::FAILURE
		}
	}
}
