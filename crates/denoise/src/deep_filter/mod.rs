mod dsp;
#[cfg(feature = "onnx")]
pub(crate) mod export;
mod model;

use std::collections::VecDeque;
use std::sync::Arc;

use realfft::num_complex::Complex32;
use tract_nnef::prelude::*;
use tract_nnef::tract_core::internal::{SimpleState, bail};

use crate::error::{DenoiseError, Result};
use dsp::{Features, Stft, erb_bands, norm_alpha};
pub(crate) use model::{Plans, load};

const SILENCE_ENERGY: f32 = 1e-7;
const SILENT_HOPS_BEFORE_SKIP: usize = 5;
const NOISE_ONLY_DB: f32 = -10.0;
const CLEAN_SPEECH_DB: f32 = 30.0;
const LIGHT_NOISE_DB: f32 = 20.0;

pub(crate) struct DeepFilter {
	plans: Arc<Plans>,
	encoder: TypedSimpleState,
	erb_decoder: TypedSimpleState,
	deep_decoder: TypedSimpleState,
	stft: Stft,
	features: Features,
	spectrum: Vec<Complex32>,
	enhanced: Vec<Complex32>,
	noisy: VecDeque<Vec<Complex32>>,
	shaped: VecDeque<Vec<Complex32>>,
	erb: Vec<f32>,
	complex: Vec<Complex32>,
	stacked: Vec<f32>,
	silent_hops: usize,
}

struct Stages {
	gains: bool,
	silence: bool,
	deep: bool,
}

impl Stages {
	fn for_snr(snr_db: f32) -> Self {
		if snr_db < NOISE_ONLY_DB {
			Self {
				gains: false,
				silence: true,
				deep: false,
			}
		} else if snr_db > CLEAN_SPEECH_DB {
			Self {
				gains: false,
				silence: false,
				deep: false,
			}
		} else if snr_db > LIGHT_NOISE_DB {
			Self {
				gains: true,
				silence: false,
				deep: false,
			}
		} else {
			Self {
				gains: true,
				silence: false,
				deep: true,
			}
		}
	}
}

impl DeepFilter {
	pub fn new(plans: Arc<Plans>) -> Result<Self> {
		let config = &plans.config;
		let stft = Stft::new(config.fft_size, config.hop);
		let bins = stft.bins();
		let bands = erb_bands(
			config.sample_rate,
			config.fft_size,
			config.erb_bands,
			config.min_erb_width,
		);
		let alpha = norm_alpha(config.sample_rate, config.hop, config.norm_tau);
		let frame = || vec![Complex32::default(); bins];
		let state = |plan: &Arc<TypedRunnableModel>| {
			SimpleState::new(plan).map_err(|error| DenoiseError::Model(error.to_string()))
		};
		Ok(Self {
			encoder: state(&plans.encoder)?,
			erb_decoder: state(&plans.erb_decoder)?,
			deep_decoder: state(&plans.deep_decoder)?,
			features: Features::new(bands, config.deep_bins, alpha),
			spectrum: frame(),
			enhanced: frame(),
			noisy: (0..config.deep_order.max(config.lookahead))
				.map(|_| frame())
				.collect(),
			shaped: (0..config.deep_order + config.conv_lookahead)
				.map(|_| frame())
				.collect(),
			erb: vec![0.0; config.erb_bands],
			complex: vec![Complex32::default(); config.deep_bins],
			stacked: vec![0.0; config.deep_bins * 2],
			silent_hops: 0,
			stft,
			plans,
		})
	}

	pub fn hop(&self) -> usize {
		self.plans.config.hop
	}

	pub fn latency_samples(&self) -> usize {
		let config = &self.plans.config;
		config.fft_size - config.hop + config.lookahead * config.hop
	}

	pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
		let energy = input.iter().map(|sample| sample * sample).sum::<f32>() / input.len().max(1) as f32;
		if energy < SILENCE_ENERGY {
			self.silent_hops += 1;
		} else {
			self.silent_hops = 0;
		}
		if self.silent_hops > SILENT_HOPS_BEFORE_SKIP {
			output.fill(0.0);
			return Ok(());
		}
		self.stft.analysis(input, &mut self.spectrum);
		rotate(&mut self.noisy, &self.spectrum);
		rotate(&mut self.shaped, &self.spectrum);
		self.infer()
			.map_err(|error| DenoiseError::Model(error.to_string()))?;
		self.stft.synthesis(&mut self.enhanced, output);
		Ok(())
	}

	fn infer(&mut self) -> TractResult<()> {
		let config = &self.plans.config;
		let deep_bins = config.deep_bins;
		let deep_order = config.deep_order;
		self.features.erb(&self.spectrum, &mut self.erb);
		self.features
			.complex(&self.spectrum[..deep_bins], &mut self.complex);
		for (index, bin) in self.complex.iter().enumerate() {
			self.stacked[index] = bin.re;
			self.stacked[deep_bins + index] = bin.im;
		}
		let erb = Tensor::from_shape(&[1, 1, 1, config.erb_bands], &self.erb)?;
		let complex = Tensor::from_shape(&[1, 2, 1, deep_bins], &self.stacked)?;
		let mut encoded = self.encoder.run(tvec![erb.into(), complex.into()])?;
		let snr_db = encoded
			.pop()
			.map(|snr| {
				snr.try_as_plain_ram()
					.and_then(|view| view.as_slice::<f32>().map(|values| values[0]))
			})
			.transpose()?;
		let (Some(snr_db), Some(c0), Some(embedding)) = (snr_db, encoded.pop(), encoded.pop()) else {
			bail!("the encoder returned too few outputs");
		};
		let stages = Stages::for_snr(snr_db);
		let target = &mut self.shaped[deep_order - 1];
		if stages.gains {
			let (Some(e3), Some(e2), Some(e1), Some(e0)) =
				(encoded.pop(), encoded.pop(), encoded.pop(), encoded.pop())
			else {
				bail!("the encoder returned too few outputs");
			};
			let gains = self.erb_decoder.run(tvec![embedding.clone(), e3, e2, e1, e0])?;
			let Some(gains) = gains.first() else {
				bail!("the erb decoder returned nothing");
			};
			self.features
				.apply_gains(target, gains.try_as_plain_ram()?.as_slice::<f32>()?);
		} else if stages.silence {
			target.fill(Complex32::default());
		}
		self.enhanced.copy_from_slice(target);
		if stages.deep {
			let coefs = self.deep_decoder.run(tvec![embedding, c0])?;
			let Some(coefs) = coefs.first() else {
				bail!("the deep filter decoder returned nothing");
			};
			let coefs = coefs.try_as_plain_ram()?;
			let coefs = coefs.as_slice::<f32>()?;
			for (bin, enhanced) in self.enhanced[..deep_bins].iter_mut().enumerate() {
				*enhanced = Complex32::default();
				for (order, frame) in self.noisy.iter().take(deep_order).enumerate() {
					let at = (bin * deep_order + order) * 2;
					*enhanced += frame[bin] * Complex32::new(coefs[at], coefs[at + 1]);
				}
			}
		}
		Ok(())
	}
}

fn rotate(frames: &mut VecDeque<Vec<Complex32>>, spectrum: &[Complex32]) {
	if let Some(mut oldest) = frames.pop_front() {
		oldest.copy_from_slice(spectrum);
		frames.push_back(oldest);
	}
}
