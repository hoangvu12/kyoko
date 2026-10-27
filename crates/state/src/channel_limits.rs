use protocol::{ChannelType, PremiumTier};

pub const BITRATE_MIN: u32 = 8_000;
pub const SLOWMODE_MAX: u32 = 21_600;
pub const CHANNEL_NAME_MAX: usize = 100;
pub const CHANNEL_TOPIC_MAX: usize = 1_024;
pub const VOICE_USER_LIMIT_MAX: u32 = 99;
pub const STAGE_USER_LIMIT_MAX: u32 = 10_000;
pub const ARCHIVE_DURATIONS: [u32; 4] = [60, 1_440, 4_320, 10_080];
pub const VIDEO_QUALITY_AUTO: i64 = 1;
pub const VIDEO_QUALITY_FULL: i64 = 2;

const BITRATE_LIMITS: [u32; 4] = [96_000, 128_000, 256_000, 384_000];
const STAGE_BITRATE_LIMIT: u32 = 64_000;
const VIP_REGIONS: &str = "VIP_REGIONS";

pub fn bitrate_limit(tier: Option<PremiumTier>, features: &[String], kind: ChannelType) -> u32 {
	if kind == ChannelType::GuildStageVoice {
		return STAGE_BITRATE_LIMIT;
	}
	if features.iter().any(|feature| feature == VIP_REGIONS) {
		return BITRATE_LIMITS[3];
	}
	let level = match tier {
		Some(PremiumTier::Tier1) => 1,
		Some(PremiumTier::Tier2) => 2,
		Some(PremiumTier::Tier3) => 3,
		_ => 0,
	};
	BITRATE_LIMITS[level]
}

pub fn user_limit_range(kind: ChannelType) -> (u32, u32) {
	if kind == ChannelType::GuildStageVoice {
		(1, STAGE_USER_LIMIT_MAX)
	} else {
		(0, VOICE_USER_LIMIT_MAX)
	}
}
