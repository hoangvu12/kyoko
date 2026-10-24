pub const LAZY_USER_NOTES: u64 = 1 << 0;
pub const NO_AFFINE_USER_IDS: u64 = 1 << 1;
pub const VERSIONED_READ_STATES: u64 = 1 << 2;
pub const VERSIONED_USER_GUILD_SETTINGS: u64 = 1 << 3;
pub const DEDUPE_USER_OBJECTS: u64 = 1 << 4;
pub const PRIORITIZED_READY_PAYLOAD: u64 = 1 << 5;
pub const MULTIPLE_GUILD_EXPERIMENT_POPULATIONS: u64 = 1 << 6;
pub const NON_CHANNEL_READ_STATES: u64 = 1 << 7;
pub const AUTH_TOKEN_REFRESH: u64 = 1 << 8;
pub const USER_SETTINGS_PROTO: u64 = 1 << 9;
pub const CLIENT_STATE_V2: u64 = 1 << 10;
pub const PASSIVE_GUILD_UPDATE: u64 = 1 << 11;
pub const AUTO_CALL_CONNECT: u64 = 1 << 12;
pub const DEBOUNCE_MESSAGE_REACTIONS: u64 = 1 << 13;
pub const PASSIVE_GUILD_UPDATE_V2: u64 = 1 << 14;
pub const CHANNEL_OBFUSCATION: u64 = 1 << 15;
pub const AUTO_LOBBY_CONNECT: u64 = 1 << 16;

pub fn has(capabilities: u64, capability: u64) -> bool {
	capabilities & capability == capability
}
