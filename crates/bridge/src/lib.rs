mod inbox;
mod sidebar;

pub use inbox::{Inbox, changes_notifier};
pub use sidebar::{ChannelEntry, ChannelKind, GuildEntry, Refresh, Sidebar};
