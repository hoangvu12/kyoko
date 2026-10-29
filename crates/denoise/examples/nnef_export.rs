use std::env;
use std::fs;
use std::process::ExitCode;

use denoise::export::{largest_difference, nnef_package};
use denoise::testing::Clip;
use sha2::{Digest, Sha256};

const ONNX_SHA256: &str = "c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616";
const SNRS_DB: [f32; 3] = [0.0, 5.0, 10.0];
const LEAD_IN: usize = 48_000;
const SEED: u64 = 11;

fn sha256(bytes: &[u8]) -> String {
	Sha256::digest(bytes)
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect()
}

fn run(source: &str, output: &str) -> Result<(), String> {
	let onnx = fs::read(source).map_err(|error| error.to_string())?;
	if sha256(&onnx) != ONNX_SHA256 {
		return Err(format!("{source} is not DeepFilterNet3_onnx.tar.gz at d375b2d"));
	}
	let package = nnef_package(&onnx).map_err(|error| error.to_string())?;
	fs::write(output, &package).map_err(|error| error.to_string())?;
	println!("{output}: {} bytes, sha256 {}", package.len(), sha256(&package));
	for snr_db in SNRS_DB {
		let clip = Clip::new(snr_db, LEAD_IN, SEED);
		let difference =
			largest_difference(&onnx, &package, &clip.noisy).map_err(|error| error.to_string())?;
		println!("  {snr_db:>4} dB clip: largest sample difference between ONNX and NNEF {difference:e}");
	}
	Ok(())
}

fn main() -> ExitCode {
	let arguments: Vec<String> = env::args().skip(1).collect();
	let [source, output] = arguments.as_slice() else {
		eprintln!("usage: nnef_export <DeepFilterNet3_onnx.tar.gz> <DeepFilterNet3.nnef.tar.gz>");
		return ExitCode::FAILURE;
	};
	match run(source, output) {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}
