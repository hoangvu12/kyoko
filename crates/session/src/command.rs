use chrono::{DateTime, Utc};
use protocol::{
	ChannelType, FavoriteGif, Gif, InteractionOption, Nonce, NotificationLevel, PermissionOverwriteKind,
	Permissions, Snowflake, StatusKind,
};
use rest::UploadFile;
use rest::api::SearchQuery;
use serde_json::Value;
use state::{EmojiCandidate, RiskyReason, RiskyTarget, Viewport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendSource {
	AddFriend,
	Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteSource {
	AddServer,
	Message {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
}

#[derive(Debug)]
pub enum Command {
	SelectChannel {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	},
	LoadOlder {
		channel_id: Snowflake,
	},
	LoadNewer {
		channel_id: Snowflake,
	},
	LoadAround {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	RefreshMedia {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	SetViewport {
		channel_id: Snowflake,
		viewport: Viewport,
	},
	SendMessage {
		channel_id: Snowflake,
		content: String,
		reply_to: Option<Snowflake>,
		ping_reply: bool,
		files: Vec<UploadFile>,
	},
	RetryMessage {
		channel_id: Snowflake,
		nonce: Nonce,
	},
	DiscardMessage {
		channel_id: Snowflake,
		nonce: Nonce,
	},
	LoadApplicationCommands {
		channel_id: Snowflake,
	},
	RunApplicationCommand {
		channel_id: Snowflake,
		command_id: Snowflake,
		options: Vec<InteractionOption>,
	},
	EditMessage {
		channel_id: Snowflake,
		message_id: Snowflake,
		content: String,
	},
	DeleteMessage {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	AddReaction {
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
	},
	RemoveReaction {
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
	},
	RemoveUserReaction {
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
		user_id: Snowflake,
	},
	Typing {
		channel_id: Snowflake,
	},
	AckChannel {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	SetViewing(bool),
	SetDraft {
		channel_id: Snowflake,
		content: String,
	},
	SetPresence {
		status: StatusKind,
		custom_status: Option<String>,
	},
	SetAfk(bool),
	SubscribeGuild {
		guild_id: Snowflake,
	},
	SetMemberViewport {
		guild_id: Snowflake,
		channel_id: Snowflake,
		first: u32,
		last: u32,
	},
	SearchMembers {
		guild_id: Snowflake,
		query: String,
	},
	RequestMembers {
		guild_id: Snowflake,
		user_ids: Vec<Snowflake>,
	},
	OpenDm {
		user_id: Snowflake,
	},
	ConfirmRisky {
		target: RiskyTarget,
		reason: RiskyReason,
	},
	Search {
		guild_id: Option<Snowflake>,
		channel_id: Option<Snowflake>,
		query: SearchQuery,
	},
	SearchMore,
	CloseSearch,
	ToggleCategory {
		guild_id: Snowflake,
		category_id: Snowflake,
	},
	RefreshThemeLinks {
		urls: Vec<String>,
	},
	CreateThread {
		channel_id: Snowflake,
		message_id: Option<Snowflake>,
		name: String,
		auto_archive_minutes: u32,
		private: bool,
	},
	JoinThread {
		thread_id: Snowflake,
	},
	LeaveThread {
		thread_id: Snowflake,
	},
	ArchiveThread {
		thread_id: Snowflake,
		archived: bool,
	},
	LoadArchivedThreads {
		channel_id: Snowflake,
		more: bool,
	},
	LoadForumPosts {
		channel_id: Snowflake,
		more: bool,
	},
	CreateForumPost {
		channel_id: Snowflake,
		name: String,
		content: String,
		tags: Vec<Snowflake>,
		files: Vec<UploadFile>,
	},
	LoadPins {
		channel_id: Snowflake,
	},
	PinMessage {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	UnpinMessage {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	AddFriend {
		username: String,
		from: FriendSource,
	},
	AcceptFriend {
		user_id: Snowflake,
	},
	RemoveRelationship {
		user_id: Snowflake,
	},
	BlockUser {
		user_id: Snowflake,
	},
	LoadProfile {
		user_id: Snowflake,
		guild_id: Option<Snowflake>,
	},
	CreateInvite {
		channel_id: Snowflake,
		max_age: u32,
		max_uses: u32,
		temporary: bool,
	},
	ResolveInvite {
		code: String,
	},
	AcceptInvite {
		code: String,
		from: InviteSource,
	},
	LoadOnboarding {
		guild_id: Snowflake,
	},
	SubmitOnboarding {
		guild_id: Snowflake,
		chosen: Vec<Snowflake>,
		prompts_seen: Vec<(Snowflake, i64)>,
		options_seen: Vec<(Snowflake, i64)>,
	},
	LoadMemberVerification {
		guild_id: Snowflake,
		invite_code: Option<String>,
	},
	AcceptMemberVerification {
		guild_id: Snowflake,
	},
	LeaveGuild {
		guild_id: Snowflake,
	},
	CreateGroupDm {
		recipient_ids: Vec<Snowflake>,
	},
	AddGroupRecipient {
		channel_id: Snowflake,
		user_id: Snowflake,
	},
	RemoveGroupRecipient {
		channel_id: Snowflake,
		user_id: Snowflake,
	},
	RenameGroup {
		channel_id: Snowflake,
		name: String,
	},
	CloseDm {
		channel_id: Snowflake,
	},
	EditGuild {
		guild_id: Snowflake,
		name: Option<String>,
		icon_png: Option<Vec<u8>>,
		description: Option<String>,
	},
	CreateChannel {
		guild_id: Snowflake,
		name: String,
		kind: ChannelType,
		parent_id: Option<Snowflake>,
	},
	EditChannel {
		channel_id: Snowflake,
		name: Option<String>,
		topic: Option<String>,
		nsfw: Option<bool>,
		slowmode: Option<u32>,
		position: Option<i64>,
		parent_id: Option<Snowflake>,
	},
	DeleteChannel {
		channel_id: Snowflake,
	},
	CreateRole {
		guild_id: Snowflake,
		name: String,
		color: u32,
		permissions: Permissions,
		hoist: bool,
		mentionable: bool,
	},
	EditRole {
		guild_id: Snowflake,
		role_id: Snowflake,
		name: Option<String>,
		color: Option<u32>,
		permissions: Option<Permissions>,
		hoist: Option<bool>,
		mentionable: Option<bool>,
	},
	DeleteRole {
		guild_id: Snowflake,
		role_id: Snowflake,
	},
	ReorderRoles {
		guild_id: Snowflake,
		positions: Vec<(Snowflake, i64)>,
	},
	SetChannelOverwrite {
		channel_id: Snowflake,
		target_id: Snowflake,
		kind: PermissionOverwriteKind,
		allow: Permissions,
		deny: Permissions,
	},
	DeleteChannelOverwrite {
		channel_id: Snowflake,
		target_id: Snowflake,
	},
	CreateEmoji {
		guild_id: Snowflake,
		name: String,
		image: Vec<u8>,
	},
	DeleteEmoji {
		guild_id: Snowflake,
		emoji_id: Snowflake,
	},
	LoadAuditLog {
		guild_id: Snowflake,
		before: Option<Snowflake>,
		action_type: Option<i64>,
	},
	LoadBans {
		guild_id: Snowflake,
	},
	LoadMoreBans {
		guild_id: Snowflake,
	},
	LoadInvites {
		guild_id: Snowflake,
	},
	RevokeInvite {
		guild_id: Snowflake,
		code: String,
	},
	KickMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		reason: Option<String>,
	},
	BanMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		delete_message_seconds: u32,
		reason: Option<String>,
	},
	UnbanMember {
		guild_id: Snowflake,
		user_id: Snowflake,
	},
	TimeoutMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		until: Option<DateTime<Utc>>,
		reason: Option<String>,
	},
	SetNickname {
		guild_id: Snowflake,
		user_id: Snowflake,
		nick: Option<String>,
	},
	AddMemberRole {
		guild_id: Snowflake,
		user_id: Snowflake,
		role_id: Snowflake,
	},
	RemoveMemberRole {
		guild_id: Snowflake,
		user_id: Snowflake,
		role_id: Snowflake,
	},
	DeleteMessages {
		channel_id: Snowflake,
		ids: Vec<Snowflake>,
	},
	MoveMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		channel_id: Option<Snowflake>,
	},
	ServerMuteMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		muted: bool,
	},
	ServerDeafenMember {
		guild_id: Snowflake,
		user_id: Snowflake,
		deafened: bool,
	},
	MuteChannel {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
		until: Option<DateTime<Utc>>,
	},
	UnmuteChannel {
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
	},
	MuteGuild {
		guild_id: Snowflake,
		until: Option<DateTime<Utc>>,
	},
	UnmuteGuild {
		guild_id: Snowflake,
	},
	SetChannelNotifications {
		channel_id: Snowflake,
		level: NotificationLevel,
	},
	SetGuildNotifications {
		guild_id: Option<Snowflake>,
		level: NotificationLevel,
		suppress_everyone: bool,
		suppress_roles: bool,
		mobile_push: bool,
	},
	MarkGuildRead {
		guild_id: Snowflake,
	},
	MarkChannelRead {
		channel_id: Snowflake,
	},
	MarkAllRead,
	MarkUnread {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	UpdateUserSettings(Value),
	SetGuildLayout(Vec<state::RailEntry>),
	UpdateGuildSettingsRaw {
		guild_id: Option<Snowflake>,
		changes: Value,
	},
	SwitchAccount(Snowflake),
	CreateGuild {
		name: String,
		icon: Option<String>,
	},
	LoadSessions,
	LogoutSessions(Vec<String>),
	SetCustomStatus {
		text: Option<String>,
		emoji: Option<String>,
		expires: Option<DateTime<Utc>>,
	},
	SetStatus(StatusKind),
	SetActivitySharing(bool),
	EditProfile {
		global_name: Option<String>,
		bio: Option<String>,
		avatar_png: Option<Vec<u8>>,
		banner_png: Option<Vec<u8>>,
		pronouns: Option<String>,
	},
	SetGuildProfile {
		guild_id: Snowflake,
		nick: Option<String>,
		avatar_png: Option<Vec<u8>>,
		bio: Option<String>,
	},
	LoadReactionUsers {
		channel_id: Snowflake,
		message_id: Snowflake,
		emoji: String,
		burst: bool,
		after: Option<Snowflake>,
	},
	ForwardMessage {
		channel_id: Snowflake,
		message_id: Snowflake,
		to_channel_id: Snowflake,
	},
	VotePoll {
		channel_id: Snowflake,
		message_id: Snowflake,
		answer_ids: Vec<i64>,
	},
	EndPoll {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	SendSticker {
		channel_id: Snowflake,
		sticker_id: Snowflake,
	},
	LoadGifTrending,
	LoadTrendingGifs,
	SearchGifs {
		query: String,
	},
	SuggestGifs {
		query: String,
	},
	SelectGif {
		id: String,
		query: String,
	},
	SendGif {
		channel_id: Snowflake,
		gif: Gif,
	},
	LoadStickerPacks,
	LoadTopEmojis {
		guild_id: Snowflake,
	},
	LoadGuildStickers {
		guild_id: Snowflake,
	},
	CreateSticker {
		guild_id: Snowflake,
		name: String,
		description: String,
		tags: String,
		file: Vec<u8>,
	},
	EditSticker {
		guild_id: Snowflake,
		sticker_id: Snowflake,
		name: Option<String>,
		description: Option<String>,
		tags: Option<String>,
	},
	DeleteSticker {
		guild_id: Snowflake,
		sticker_id: Snowflake,
	},
	EditEmoji {
		guild_id: Snowflake,
		id: Snowflake,
		name: String,
		roles: Vec<Snowflake>,
	},
	RankEmoji {
		guild_id: Option<Snowflake>,
		query: String,
		candidates: Vec<EmojiCandidate>,
	},
	LoadFavorites,
	SetGifFavorite {
		gif: FavoriteGif,
		favorite: bool,
	},
	SetEmojiFavorite {
		entry: String,
		favorite: bool,
	},
	SuppressEmbeds {
		channel_id: Snowflake,
		message_id: Snowflake,
	},
	Reconnect,
	Logout,
	ShowDeletedMessages(bool),
	Shutdown,
}

impl Command {
	pub fn reads_channel(&self) -> Option<Snowflake> {
		match self {
			Command::SelectChannel { channel_id, .. }
			| Command::LoadOlder { channel_id }
			| Command::LoadNewer { channel_id }
			| Command::LoadAround { channel_id, .. }
			| Command::RefreshMedia { channel_id, .. }
			| Command::SetViewport { channel_id, .. }
			| Command::SendMessage { channel_id, .. }
			| Command::LoadApplicationCommands { channel_id }
			| Command::RunApplicationCommand { channel_id, .. }
			| Command::AddReaction { channel_id, .. }
			| Command::Typing { channel_id }
			| Command::AckChannel { channel_id, .. }
			| Command::SetMemberViewport { channel_id, .. }
			| Command::CreateThread { channel_id, .. }
			| Command::JoinThread {
				thread_id: channel_id,
			}
			| Command::LoadArchivedThreads { channel_id, .. }
			| Command::LoadForumPosts { channel_id, .. }
			| Command::CreateForumPost { channel_id, .. }
			| Command::LoadPins { channel_id }
			| Command::MarkChannelRead { channel_id }
			| Command::MarkUnread { channel_id, .. }
			| Command::LoadReactionUsers { channel_id, .. }
			| Command::SendSticker { channel_id, .. }
			| Command::SendGif { channel_id, .. }
			| Command::Search {
				channel_id: Some(channel_id),
				..
			} => Some(*channel_id),
			_ => None,
		}
	}
}
