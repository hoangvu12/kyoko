use std::env;
use std::fs;
use std::process::ExitCode;

const BLOCK: usize = 64;
const NAME_BYTES: usize = 44;
const DEBUG_FLOAT_GUARD: &str = "#ifndef DISABLE_DEBUG_FLOAT";
const DEBUG_FLOAT_END: &str = "#endif /*DISABLE_DEBUG_FLOAT*/";

#[derive(Clone, Copy)]
enum Kind {
	Float,
	Int,
	Int8,
}

impl Kind {
	fn code(self) -> i32 {
		match self {
			Self::Float => 0,
			Self::Int => 1,
			Self::Int8 => 3,
		}
	}

	fn of(declaration: &str) -> Option<Self> {
		if declaration.starts_with("static const float ") {
			Some(Self::Float)
		} else if declaration.starts_with("static const int ") {
			Some(Self::Int)
		} else if declaration.starts_with("static const opus_int8 ") {
			Some(Self::Int8)
		} else {
			None
		}
	}

	fn push(self, value: &str, data: &mut Vec<u8>) -> Result<(), String> {
		match self {
			Self::Float => data.extend(parse::<f32>(value)?.to_le_bytes()),
			Self::Int => data.extend(parse::<i32>(value)?.to_le_bytes()),
			Self::Int8 => data.extend(parse::<i8>(value)?.to_le_bytes()),
		}
		Ok(())
	}
}

fn parse<T: std::str::FromStr>(value: &str) -> Result<T, String> {
	value.parse().map_err(|_| format!("not a number: {value}"))
}

struct Array {
	name: String,
	kind: Kind,
	data: Vec<u8>,
}

fn arrays(source: &str) -> Result<Vec<Array>, String> {
	let mut found = Vec::new();
	let mut lines = source.lines();
	let mut debug_float = false;
	while let Some(line) = lines.next() {
		let line = line.trim();
		if line == DEBUG_FLOAT_GUARD {
			debug_float = true;
			continue;
		}
		if line == DEBUG_FLOAT_END {
			debug_float = false;
			continue;
		}
		let Some(kind) = Kind::of(line) else {
			continue;
		};
		let name = line
			.split_whitespace()
			.nth(3)
			.and_then(|declared| declared.split('[').next())
			.ok_or_else(|| format!("unreadable declaration: {line}"))?
			.to_owned();
		let mut data = Vec::new();
		for values in lines.by_ref() {
			let values = values.trim();
			if values.starts_with("};") {
				break;
			}
			for value in values.split(',').map(str::trim).filter(|value| !value.is_empty()) {
				kind.push(value, &mut data)?;
			}
		}
		if !debug_float {
			found.push(Array { name, kind, data });
		}
	}
	Ok(found)
}

fn blob(arrays: &[Array]) -> Vec<u8> {
	let mut out = Vec::new();
	for array in arrays {
		let padded = array.data.len().div_ceil(BLOCK) * BLOCK;
		out.extend(b"DNNw");
		out.extend(0i32.to_le_bytes());
		out.extend(array.kind.code().to_le_bytes());
		out.extend((array.data.len() as i32).to_le_bytes());
		out.extend((padded as i32).to_le_bytes());
		let mut name = [0u8; NAME_BYTES];
		let bytes = array.name.as_bytes();
		let length = bytes.len().min(NAME_BYTES - 1);
		name[..length].copy_from_slice(&bytes[..length]);
		out.extend(name);
		out.extend(&array.data);
		out.resize(out.len() + padded - array.data.len(), 0);
	}
	out
}

fn main() -> ExitCode {
	let arguments: Vec<String> = env::args().skip(1).collect();
	let [source, output] = arguments.as_slice() else {
		eprintln!("usage: rnnoise_weights <rnnoise_data.c> <output.bin>");
		return ExitCode::FAILURE;
	};
	let result = fs::read_to_string(source)
		.map_err(|error| error.to_string())
		.and_then(|text| arrays(&text))
		.and_then(|arrays| {
			for array in &arrays {
				println!("{} {} bytes", array.name, array.data.len());
			}
			fs::write(output, blob(&arrays)).map_err(|error| error.to_string())
		});
	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}
