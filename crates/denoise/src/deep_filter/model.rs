use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use flate2::read::GzDecoder;
use tract_nnef::framework::Nnef;
use tract_nnef::prelude::*;
use tract_pulse_opl::WithPulse;

use crate::error::{DenoiseError, Result};

pub(crate) const CONFIG: &str = "config.ini";
pub(crate) const NNEF_NAMES: [&str; 3] = ["enc.nnef.tar", "erb_dec.nnef.tar", "df_dec.nnef.tar"];

pub(crate) struct Config {
	pub sample_rate: usize,
	pub fft_size: usize,
	pub hop: usize,
	pub erb_bands: usize,
	pub min_erb_width: usize,
	pub deep_bins: usize,
	pub deep_order: usize,
	pub lookahead: usize,
	pub conv_lookahead: usize,
	pub norm_tau: f32,
	#[cfg(feature = "onnx")]
	pub conv_channels: usize,
}

pub(crate) struct Plans {
	pub config: Config,
	pub encoder: Arc<TypedRunnableModel>,
	pub erb_decoder: Arc<TypedRunnableModel>,
	pub deep_decoder: Arc<TypedRunnableModel>,
}

pub(crate) struct Archive {
	pub config: String,
	pub encoder: Vec<u8>,
	pub erb_decoder: Vec<u8>,
	pub deep_decoder: Vec<u8>,
}

pub(crate) fn nnef() -> Nnef {
	tract_nnef::nnef().with_pulse()
}

pub(crate) fn load(package: &[u8]) -> Result<Plans> {
	let archive = unpack(package, NNEF_NAMES)?;
	let config = Config::parse(&archive.config)?;
	let nnef = nnef();
	let read = |bytes: &[u8]| nnef.model_for_read(&mut Cursor::new(bytes)).map_err(model_error);
	plans(
		config,
		[
			read(&archive.encoder)?,
			read(&archive.erb_decoder)?,
			read(&archive.deep_decoder)?,
		],
	)
}

pub(crate) fn plans(config: Config, models: [TypedModel; 3]) -> Result<Plans> {
	let [encoder, erb_decoder, deep_decoder] =
		models.map(|model| model.into_optimized().and_then(|model| model.into_runnable()));
	Ok(Plans {
		encoder: encoder.map_err(model_error)?,
		erb_decoder: erb_decoder.map_err(model_error)?,
		deep_decoder: deep_decoder.map_err(model_error)?,
		config,
	})
}

pub(crate) fn model_error(error: TractError) -> DenoiseError {
	DenoiseError::Model(format!("{error:#}"))
}

pub(crate) fn unpack(bytes: &[u8], names: [&str; 3]) -> Result<Archive> {
	let mut tar = tar::Archive::new(GzDecoder::new(bytes));
	let mut found = Archive {
		config: String::new(),
		encoder: Vec::new(),
		erb_decoder: Vec::new(),
		deep_decoder: Vec::new(),
	};
	let entries = tar
		.entries()
		.map_err(|error| DenoiseError::Model(error.to_string()))?;
	for entry in entries {
		let mut entry = entry.map_err(|error| DenoiseError::Model(error.to_string()))?;
		let name = entry
			.path()
			.ok()
			.and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
			.unwrap_or_default();
		let read = if name == names[0] {
			entry.read_to_end(&mut found.encoder).map(drop)
		} else if name == names[1] {
			entry.read_to_end(&mut found.erb_decoder).map(drop)
		} else if name == names[2] {
			entry.read_to_end(&mut found.deep_decoder).map(drop)
		} else if name == CONFIG {
			entry.read_to_string(&mut found.config).map(drop)
		} else {
			Ok(())
		};
		read.map_err(|error| DenoiseError::Model(error.to_string()))?;
	}
	if found.config.is_empty()
		|| found.encoder.is_empty()
		|| found.erb_decoder.is_empty()
		|| found.deep_decoder.is_empty()
	{
		return Err(DenoiseError::Model("the model archive is incomplete".to_owned()));
	}
	Ok(found)
}

impl Config {
	pub(crate) fn parse(text: &str) -> Result<Self> {
		let mut values: HashMap<(String, String), String> = HashMap::new();
		let mut section = String::new();
		for line in text.lines().map(str::trim) {
			if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
				section = name.trim().to_owned();
			} else if let Some((key, value)) = line.split_once('=') {
				values.insert((section.clone(), key.trim().to_owned()), value.trim().to_owned());
			}
		}
		let get = |section: &str, key: &str| -> Result<String> {
			values
				.get(&(section.to_owned(), key.to_owned()))
				.cloned()
				.ok_or_else(|| DenoiseError::Model(format!("the model config has no {section}.{key}")))
		};
		let number = |section: &str, key: &str| -> Result<usize> {
			get(section, key)?
				.parse()
				.map_err(|_| DenoiseError::Model(format!("the model config has a bad {section}.{key}")))
		};
		if get("train", "model")? != "deepfilternet3" {
			return Err(DenoiseError::Model("the model is not DeepFilterNet3".to_owned()));
		}
		let conv_lookahead = number("deepfilternet", "conv_lookahead")?;
		let deep_lookahead = number("df", "df_lookahead")?;
		Ok(Self {
			sample_rate: number("df", "sr")?,
			fft_size: number("df", "fft_size")?,
			hop: number("df", "hop_size")?,
			erb_bands: number("df", "nb_erb")?,
			min_erb_width: number("df", "min_nb_erb_freqs")?,
			deep_bins: number("df", "nb_df")?,
			deep_order: number("df", "df_order")?,
			lookahead: conv_lookahead.max(deep_lookahead),
			conv_lookahead,
			norm_tau: get("df", "norm_tau")?
				.parse()
				.map_err(|_| DenoiseError::Model("the model config has a bad df.norm_tau".to_owned()))?,
			#[cfg(feature = "onnx")]
			conv_channels: number("deepfilternet", "conv_ch")?,
		})
	}
}
