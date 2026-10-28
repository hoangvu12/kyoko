const HANGOVER_FRAMES: u32 = 15;
const SILENCE_FLOOR: f32 = 1e-6;
pub(crate) const LEVEL_FLOOR_DB: f32 = -100.0;
const DB_FALL_PER_FRAME: f32 = 1.5;

#[derive(Default)]
pub struct VoiceGate {
	remaining: u32,
}

impl VoiceGate {
	pub fn update(&mut self, level_db: f32, threshold_db: f32) -> bool {
		if level_db >= threshold_db {
			self.remaining = HANGOVER_FRAMES;
			true
		} else if self.remaining > 0 {
			self.remaining -= 1;
			true
		} else {
			false
		}
	}

	pub fn reset(&mut self) {
		self.remaining = 0;
	}
}

pub fn rms(frame: &[f32]) -> f32 {
	if frame.is_empty() {
		return 0.0;
	}
	(frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len() as f32).sqrt()
}

pub fn to_db(linear: f32) -> f32 {
	20.0 * linear.max(SILENCE_FLOOR).log10()
}

pub fn meter_level(linear: f32) -> f32 {
	((to_db(linear) + 60.0) / 60.0).clamp(0.0, 1.0)
}

pub fn decayed_db(level_db: f32, previous_db: f32) -> f32 {
	level_db.max(previous_db - DB_FALL_PER_FRAME).max(LEVEL_FLOOR_DB)
}
