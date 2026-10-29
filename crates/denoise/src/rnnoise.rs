use std::ffi::{c_int, c_void};
use std::ptr::NonNull;
use std::sync::OnceLock;

use crate::error::{DenoiseError, Result};

pub(crate) const FRAME: usize = 480;
const PCM_SCALE: f32 = 32768.0;

#[repr(C)]
struct DenoiseState {
	_opaque: [u8; 0],
}

#[repr(C)]
struct RnnModel {
	_opaque: [u8; 0],
}

unsafe extern "C" {
	fn rnnoise_model_from_buffer(ptr: *const c_void, len: c_int) -> *mut RnnModel;
	fn rnnoise_create(model: *mut RnnModel) -> *mut DenoiseState;
	fn rnnoise_destroy(state: *mut DenoiseState);
	fn rnnoise_process_frame(state: *mut DenoiseState, out: *mut f32, input: *const f32) -> f32;
}

struct Model(NonNull<RnnModel>);

unsafe impl Send for Model {}
unsafe impl Sync for Model {}

static MODEL: OnceLock<Option<Model>> = OnceLock::new();

fn model(weights: &'static [u8]) -> Option<&'static Model> {
	MODEL
		.get_or_init(|| {
			let length = c_int::try_from(weights.len()).ok()?;
			NonNull::new(unsafe { rnnoise_model_from_buffer(weights.as_ptr().cast(), length) }).map(Model)
		})
		.as_ref()
}

pub(crate) struct Rnnoise {
	state: NonNull<DenoiseState>,
	input: [f32; FRAME],
	output: [f32; FRAME],
}

unsafe impl Send for Rnnoise {}

impl Rnnoise {
	pub fn new(weights: &'static [u8]) -> Result<Self> {
		let model = model(weights)
			.ok_or_else(|| DenoiseError::Model("RNNoise could not read its weights".to_owned()))?;
		let state = NonNull::new(unsafe { rnnoise_create(model.0.as_ptr()) })
			.ok_or_else(|| DenoiseError::Model("RNNoise could not load its weights".to_owned()))?;
		Ok(Self {
			state,
			input: [0.0; FRAME],
			output: [0.0; FRAME],
		})
	}

	pub fn process(&mut self, hop: &mut [f32]) {
		for (scaled, sample) in self.input.iter_mut().zip(hop.iter()) {
			*scaled = sample * PCM_SCALE;
		}
		unsafe {
			rnnoise_process_frame(self.state.as_ptr(), self.output.as_mut_ptr(), self.input.as_ptr());
		}
		for (sample, scaled) in hop.iter_mut().zip(&self.output) {
			*sample = scaled / PCM_SCALE;
		}
	}
}

impl Drop for Rnnoise {
	fn drop(&mut self) {
		unsafe { rnnoise_destroy(self.state.as_ptr()) };
	}
}
