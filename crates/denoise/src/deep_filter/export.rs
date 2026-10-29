use std::io::{Cursor, Write};

use flate2::Compression;
use flate2::write::GzEncoder;
use tract_onnx::prelude::*;
use tract_onnx::tract_hir::shapefactoid;
use tract_pulse::WithPulse;
use tract_pulse::internal::ToDim;
use tract_pulse::model::{PulsedModel, PulsedModelExt};

use super::model::{CONFIG, Config, NNEF_NAMES, Plans, model_error, plans, unpack};
use crate::error::{DenoiseError, Result};

const STREAM: &str = "S";
const ONNX_NAMES: [&str; 3] = ["enc.onnx", "erb_dec.onnx", "df_dec.onnx"];

pub(crate) fn onnx_plans(archive: &[u8]) -> Result<Plans> {
	let (config, models) = pulsed_models(archive)?;
	plans(Config::parse(&config)?, models)
}

pub(crate) fn nnef_package(archive: &[u8]) -> Result<Vec<u8>> {
	let (config, models) = pulsed_models(archive)?;
	let nnef = tract_nnef::nnef().with_pulse();
	let mut package = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
	add(&mut package, CONFIG, config.as_bytes())?;
	for (name, model) in NNEF_NAMES.into_iter().zip(&models) {
		let tar = nnef.write_to_tar(model, Vec::new()).map_err(model_error)?;
		add(&mut package, name, &tar)?;
	}
	package
		.into_inner()
		.and_then(|encoder| encoder.finish())
		.map_err(|error| DenoiseError::Model(error.to_string()))
}

fn add(package: &mut tar::Builder<impl Write>, name: &str, bytes: &[u8]) -> Result<()> {
	let mut header = tar::Header::new_gnu();
	header.set_size(bytes.len() as u64);
	header.set_mode(0o644);
	header.set_mtime(0);
	header.set_cksum();
	package
		.append_data(&mut header, name, bytes)
		.map_err(|error| DenoiseError::Model(error.to_string()))
}

fn pulsed_models(archive: &[u8]) -> Result<(String, [TypedModel; 3])> {
	let archive = unpack(archive, ONNX_NAMES)?;
	let config = Config::parse(&archive.config)?;
	let models = [
		encoder(&archive.encoder, &config).map_err(model_error)?,
		erb_decoder(&archive.erb_decoder, &config).map_err(model_error)?,
		deep_decoder(&archive.deep_decoder, &config).map_err(model_error)?,
	];
	Ok((archive.config, models))
}

fn onnx_model(bytes: &[u8]) -> TractResult<InferenceModel> {
	tract_onnx::onnx()
		.with_ignore_output_shapes(true)
		.model_for_read(&mut Cursor::new(bytes))
}

fn with_outputs(model: InferenceModel, names: &[&str]) -> TractResult<InferenceModel> {
	let outlets = names
		.iter()
		.map(|name| {
			model
				.outlet_labels
				.iter()
				.find(|(_, label)| label.as_str() == *name)
				.map(|(outlet, _)| *outlet)
				.map_or_else(
					|| model.node_by_name(name).map(|node| OutletId::new(node.id, 0)),
					Ok,
				)
		})
		.collect::<TractResult<Vec<_>>>()?;
	model.with_output_outlets(&outlets)
}

fn pulsed(model: InferenceModel, stream: Symbol) -> TractResult<TypedModel> {
	let mut typed = model.into_typed()?;
	typed.declutter()?;
	let pulsed = PulsedModel::new(&typed, stream, &1.to_dim())?;
	pulsed.into_typed()?.into_decluttered()
}

fn encoder(bytes: &[u8], config: &Config) -> TractResult<TypedModel> {
	let mut model = onnx_model(bytes)?;
	let s = model.symbols.sym(STREAM);
	model = model
		.with_input_fact(
			0,
			InferenceFact::dt_shape(f32::datum_type(), shapefactoid!(1, 1, s, (config.erb_bands))),
		)?
		.with_input_fact(
			1,
			InferenceFact::dt_shape(f32::datum_type(), shapefactoid!(1, 2, s, (config.deep_bins))),
		)?
		.with_input_names(["feat_erb", "feat_spec"])?;
	model = with_outputs(model, &["e0", "e1", "e2", "e3", "emb", "c0", "lsnr"])?;
	model.analyse(true)?;
	pulsed(model, s)
}

fn erb_decoder(bytes: &[u8], config: &Config) -> TractResult<TypedModel> {
	let mut model = onnx_model(bytes)?;
	let s = model.symbols.sym(STREAM);
	let width = config.conv_channels;
	let hidden = width * config.erb_bands / 4;
	let quarter = config.erb_bands / 4;
	let half = config.erb_bands / 2;
	let fact = |shape| InferenceFact::dt_shape(f32::datum_type(), shape);
	model = model
		.with_input_fact(0, fact(shapefactoid!(1, s, hidden)))?
		.with_input_fact(1, fact(shapefactoid!(1, width, s, quarter)))?
		.with_input_fact(2, fact(shapefactoid!(1, width, s, quarter)))?
		.with_input_fact(3, fact(shapefactoid!(1, width, s, half)))?
		.with_input_fact(4, fact(shapefactoid!(1, width, s, (config.erb_bands))))?
		.with_input_names(["emb", "e3", "e2", "e1", "e0"])?;
	model.analyse(true)?;
	pulsed(model, s)
}

fn deep_decoder(bytes: &[u8], config: &Config) -> TractResult<TypedModel> {
	let mut model = onnx_model(bytes)?;
	let s = model.symbols.sym(STREAM);
	let width = config.conv_channels;
	let hidden = width * config.erb_bands / 4;
	let fact = |shape| InferenceFact::dt_shape(f32::datum_type(), shape);
	model = model
		.with_input_fact(0, fact(shapefactoid!(1, s, hidden)))?
		.with_input_fact(1, fact(shapefactoid!(1, width, s, (config.deep_bins))))?
		.with_input_names(["emb", "c0"])?;
	model = with_outputs(model, &["coefs"])?;
	model.analyse(true)?;
	pulsed(model, s)
}
