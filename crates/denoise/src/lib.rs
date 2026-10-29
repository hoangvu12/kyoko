mod deep_filter;
mod error;
mod integrity;
mod rnnoise;
#[cfg(feature = "testing")]
pub mod testing;

#[cfg(feature = "onnx")]
pub mod export {
	use crate::deep_filter::{DeepFilter, export};
	use crate::error::Result;

	pub fn nnef_package(onnx_archive: &[u8]) -> Result<Vec<u8>> {
		export::nnef_package(onnx_archive)
	}

	pub fn largest_difference(onnx_archive: &[u8], nnef_package: &[u8], audio: &[f32]) -> Result<f32> {
		let mut from_onnx = DeepFilter::new(std::sync::Arc::new(export::onnx_plans(onnx_archive)?))?;
		let mut from_nnef = DeepFilter::new(std::sync::Arc::new(crate::deep_filter::load(nnef_package)?))?;
		let hop = from_onnx.hop();
		let (mut left, mut right) = (vec![0.0; hop], vec![0.0; hop]);
		let mut largest = 0.0f32;
		for chunk in audio.chunks_exact(hop) {
			from_onnx.process(chunk, &mut left)?;
			from_nnef.process(chunk, &mut right)?;
			for (a, b) in left.iter().zip(&right) {
				largest = largest.max((a - b).abs());
			}
		}
		Ok(largest)
	}
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once, OnceLock};
use std::thread;
use std::time::Duration;

use deep_filter::{DeepFilter, Plans};
pub use error::{DenoiseError, Result};
use rnnoise::Rnnoise;

pub const HOP: usize = 480;
pub const SAMPLE_RATE: usize = 48_000;

const DEEP_FILTER_MODEL: &[u8] = include_bytes!("../models/DeepFilterNet3.nnef.tar.gz");
const DEEP_FILTER_SHA256: &str = "592a715f37640c92af5aa086abdc7247572e0c858aac9c5319385927338a4317";
const RNNOISE_SHA256: &str = "a81e3e6f3e0ce3761493a37e2136a155d4801265d1bf438f80fccc9a6807d1e8";
const DEEP_FILTER_LATENCY: Duration = Duration::from_millis(30);
const RNNOISE_LATENCY: Duration = Duration::from_millis(10);

#[repr(C, align(64))]
struct Aligned<B: ?Sized>(B);

static RNNOISE_WEIGHTS: &Aligned<[u8]> = &Aligned(*include_bytes!("../models/rnnoise.bin"));
static DEEP_FILTER: OnceLock<Result<Arc<Plans>>> = OnceLock::new();
static RNNOISE_CHECKED: OnceLock<Result<&'static [u8]>> = OnceLock::new();
static LOADING: Once = Once::new();
static FALLBACK_LOGGED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Suppression {
	#[default]
	HighQuality,
	LowCpu,
	Off,
}

impl Suppression {
	pub fn code(self) -> u8 {
		match self {
			Self::HighQuality => 0,
			Self::LowCpu => 1,
			Self::Off => 2,
		}
	}

	pub fn from_code(code: u8) -> Self {
		match code {
			1 => Self::LowCpu,
			2 => Self::Off,
			_ => Self::HighQuality,
		}
	}

	pub fn latency(self) -> Duration {
		match self {
			Self::HighQuality => DEEP_FILTER_LATENCY,
			Self::LowCpu => RNNOISE_LATENCY,
			Self::Off => Duration::ZERO,
		}
	}
}

pub fn prepare(suppression: Suppression) {
	if suppression != Suppression::HighQuality {
		return;
	}
	LOADING.call_once(|| {
		let spawned = thread::Builder::new()
			.name("denoise-load".to_owned())
			.spawn(|| deep_filter_plans().map(drop));
		if let Err(error) = spawned {
			tracing::warn!(%error, "could not start loading the noise model");
		}
	});
}

pub fn load(suppression: Suppression) -> Result<()> {
	match suppression {
		Suppression::HighQuality => deep_filter_plans().map(drop),
		Suppression::LowCpu => rnnoise_weights().map(drop),
		Suppression::Off => Ok(()),
	}
}

fn deep_filter_plans() -> Result<Arc<Plans>> {
	DEEP_FILTER
		.get_or_init(|| {
			let archive = integrity::verified("DeepFilterNet3", DEEP_FILTER_MODEL, DEEP_FILTER_SHA256)?;
			let plans = deep_filter::load(archive).map(Arc::new);
			if let Err(error) = &plans {
				tracing::warn!(%error, "the DeepFilterNet3 model did not load");
			}
			plans
		})
		.clone()
}

fn rnnoise_weights() -> Result<&'static [u8]> {
	RNNOISE_CHECKED
		.get_or_init(|| integrity::verified("RNNoise", &RNNOISE_WEIGHTS.0, RNNOISE_SHA256))
		.clone()
}

#[derive(Default)]
pub struct NoiseSuppressor {
	active: Option<Suppression>,
	deep_filter: Option<DeepFilter>,
	rnnoise: Option<Rnnoise>,
	hop: Vec<f32>,
}

impl NoiseSuppressor {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn process(&mut self, wanted: Suppression, frame: &mut [f32]) -> Option<Suppression> {
		let used = match wanted {
			Suppression::Off => None,
			Suppression::HighQuality if self.deep_filter_ready() => Some(Suppression::HighQuality),
			Suppression::HighQuality | Suppression::LowCpu if self.rnnoise_ready() => {
				Some(Suppression::LowCpu)
			}
			Suppression::HighQuality | Suppression::LowCpu => None,
		};
		if used != self.active {
			self.active = used;
			self.reset_except(used);
		}
		match used? {
			Suppression::HighQuality => self.run_deep_filter(frame),
			_ => self.run_rnnoise(frame),
		}
	}

	fn reset_except(&mut self, used: Option<Suppression>) {
		if used != Some(Suppression::HighQuality) {
			self.deep_filter = None;
		}
		if used != Some(Suppression::LowCpu) {
			self.rnnoise = None;
		}
	}

	fn deep_filter_ready(&mut self) -> bool {
		if self.deep_filter.is_some() {
			return true;
		}
		prepare(Suppression::HighQuality);
		let Some(plans) = DEEP_FILTER.get() else {
			return false;
		};
		let created = plans.clone().and_then(DeepFilter::new);
		match created {
			Ok(filter) => {
				self.deep_filter = Some(filter);
				true
			}
			Err(error) => {
				if !FALLBACK_LOGGED.swap(true, Ordering::Relaxed) {
					tracing::warn!(%error, "falling back to RNNoise");
				}
				false
			}
		}
	}

	fn rnnoise_ready(&mut self) -> bool {
		if self.rnnoise.is_some() {
			return true;
		}
		match rnnoise_weights().and_then(Rnnoise::new) {
			Ok(rnnoise) => {
				self.rnnoise = Some(rnnoise);
				true
			}
			Err(error) => {
				tracing::warn!(%error, "RNNoise is unavailable");
				false
			}
		}
	}

	fn run_deep_filter(&mut self, frame: &mut [f32]) -> Option<Suppression> {
		let filter = self.deep_filter.as_mut()?;
		let hop = filter.hop();
		self.hop.resize(hop, 0.0);
		for chunk in frame.chunks_exact_mut(hop) {
			if let Err(error) = filter.process(chunk, &mut self.hop) {
				tracing::warn!(%error, "DeepFilterNet3 failed, restarting it");
				self.deep_filter = None;
				self.active = None;
				return None;
			}
			chunk.copy_from_slice(&self.hop);
		}
		Some(Suppression::HighQuality)
	}

	fn run_rnnoise(&mut self, frame: &mut [f32]) -> Option<Suppression> {
		let rnnoise = self.rnnoise.as_mut()?;
		for chunk in frame.as_chunks_mut::<{ rnnoise::FRAME }>().0 {
			rnnoise.process(chunk);
		}
		Some(Suppression::LowCpu)
	}

	pub fn latency_samples(&self) -> usize {
		match (&self.deep_filter, self.active) {
			(Some(filter), Some(Suppression::HighQuality)) => filter.latency_samples(),
			(_, Some(Suppression::LowCpu)) => rnnoise::FRAME,
			_ => 0,
		}
	}
}
