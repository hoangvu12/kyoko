mod audio;
mod dave;
mod seal;
#[cfg(feature = "openh264")]
mod software;
#[cfg(windows)]
mod windows_media;

use std::fmt;
use std::process::ExitCode;

pub struct Check {
	pub name: &'static str,
	pub outcome: Result<String, String>,
}

pub struct Trial {
	pub checks: Vec<Check>,
}

impl Trial {
	pub fn passed(&self) -> bool {
		self.checks.iter().all(|check| check.outcome.is_ok())
	}
}

impl fmt::Display for Trial {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		for check in &self.checks {
			match &check.outcome {
				Ok(detail) => writeln!(formatter, "ok      {}: {detail}", check.name)?,
				Err(error) => writeln!(formatter, "FAILED  {}: {error}", check.name)?,
			}
		}
		Ok(())
	}
}

pub fn trial() -> Trial {
	let mut checks = vec![
		check("opus", audio::opus_round_trip),
		check("rubato", audio::resample),
		check("noise suppression", audio::suppress_noise),
		check("cpal", audio::devices),
		check("transport seal", seal::round_trip),
		check("dave mls", dave::two_members),
	];
	#[cfg(windows)]
	checks.extend([
		check("media foundation", windows_media::h264_transforms),
		check("graphics capture", windows_media::capture_supported),
	]);
	#[cfg(feature = "openh264")]
	checks.push(check("openh264", software::round_trip));
	Trial { checks }
}

fn check(name: &'static str, run: fn() -> Result<String, String>) -> Check {
	Check { name, outcome: run() }
}

fn text(error: impl fmt::Display) -> String {
	error.to_string()
}

fn main() -> ExitCode {
	let trial = trial();
	print!("{trial}");
	if trial.passed() {
		ExitCode::SUCCESS
	} else {
		ExitCode::FAILURE
	}
}
