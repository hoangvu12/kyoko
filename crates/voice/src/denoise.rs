use nnnoiseless::DenoiseState;

const PCM_SCALE: f32 = 32768.0;

pub(crate) struct Denoiser {
	state: Box<DenoiseState<'static>>,
	input: [f32; DenoiseState::FRAME_SIZE],
	output: [f32; DenoiseState::FRAME_SIZE],
}

impl Denoiser {
	pub fn new() -> Self {
		Self {
			state: DenoiseState::new(),
			input: [0.0; DenoiseState::FRAME_SIZE],
			output: [0.0; DenoiseState::FRAME_SIZE],
		}
	}

	pub fn process(&mut self, frame: &mut [f32]) {
		for chunk in frame.as_chunks_mut::<{ DenoiseState::FRAME_SIZE }>().0 {
			for (scaled, sample) in self.input.iter_mut().zip(chunk.iter()) {
				*scaled = sample * PCM_SCALE;
			}
			self.state.process_frame(&mut self.output, &self.input);
			for (sample, scaled) in chunk.iter_mut().zip(&self.output) {
				*sample = scaled / PCM_SCALE;
			}
		}
	}
}
