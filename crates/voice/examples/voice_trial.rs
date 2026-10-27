use std::process::ExitCode;

fn main() -> ExitCode {
	let trial = voice::trial();
	print!("{trial}");
	if trial.passed() {
		ExitCode::SUCCESS
	} else {
		ExitCode::FAILURE
	}
}
