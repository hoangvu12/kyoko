use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use protocol::{
	ApplicationCommandIndex, DispatchEvent, InteractionModal, Message, Nonce, Poll, Ready, ReadySupplemental,
	Snowflake, StickerItem, User,
};

use crate::deletions::{DeleteBatch, DeleteStatus};
use crate::kept::MessageVersion;
use crate::loaded::Loaded;
use crate::locks::{RiskyReason, RiskyTarget};
use crate::messages::{CommandStage, PendingFile, SendFailure, Viewport};
use crate::modal::ModalStatus;
use crate::state::{ConnectionStatus, SecretStorage};
use crate::views::{RenderedMessage, UserOverride};
use crate::voice::{StreamInfo, UserAudio, VideoKind, VoiceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadDirection {
	History,
	Around,
	Newer,
}

#[derive(Debug, Clone)]
pub enum LocalEvent {
	PendingMessage {
		channel_id: Snowflake,
		nonce: Nonce,
		content: String,
		stickers: Vec<StickerItem>,
		poll: Option<Box<Poll>>,
		author: Box<User>,
		by_plugin: Option<String>,
		files: Vec<PendingFile>,
	},
	UploadProgress {
		nonce: Nonce,
		sent: u64,
	},
	PendingFailed {
		nonce: Nonce,
		reason: SendFailure,
	},
	PendingAltered {
		nonce: Nonce,
		content: String,
		by: String,
	},
	EditAltered {
		channel_id: Snowflake,
		id: Snowflake,
		by: Option<String>,
	},
	MessageRendered {
		channel_id: Snowflake,
		id: Snowflake,
		source: Arc<Message>,
		rendered: Arc<RenderedMessage>,
	},
	UserOverrides {
		entries: Vec<(Option<Snowflake>, Snowflake, Option<UserOverride>)>,
	},
	UserOverridesCleared,
	PendingDiscarded {
		nonce: Nonce,
	},
	PendingCommand {
		channel_id: Snowflake,
		nonce: Nonce,
		name: String,
		application: Box<User>,
		user: Box<User>,
	},
	CommandAdvanced {
		nonce: Nonce,
		stage: CommandStage,
	},
	EphemeralDismissed {
		channel_id: Snowflake,
		id: Snowflake,
	},
	InteractionExpired {
		channel_id: Snowflake,
		id: Snowflake,
	},
	ComponentInteraction {
		channel_id: Snowflake,
		id: Snowflake,
		failed: bool,
	},
	ComponentWaiting {
		channel_id: Snowflake,
		id: Snowflake,
		custom_id: Option<String>,
	},
	ModalOpened(Box<InteractionModal>),
	ModalChanged {
		status: ModalStatus,
		missing: Vec<String>,
	},
	ModalClosed,
	DraftChanged {
		channel_id: Snowflake,
		content: String,
	},
	ChannelSelected {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
	},
	ViewportChanged {
		channel_id: Snowflake,
		viewport: Viewport,
	},
	CategoryCollapsed {
		channel_id: Snowflake,
		collapsed: bool,
	},
	Connection(ConnectionStatus),
	HistoryFailed {
		channel_id: Snowflake,
		reason: String,
	},
	HistoryRequested {
		channel_id: Snowflake,
	},
	ProfileRequested {
		user_id: Snowflake,
	},
	ProfileSettled {
		user_id: Snowflake,
	},
	LatencyMeasured(Duration),
	SecretStorage(SecretStorage),
	VoiceConnection {
		status: VoiceStatus,
		channel_id: Snowflake,
		guild_id: Option<Snowflake>,
	},
	VoiceSelf {
		self_mute: bool,
		self_deaf: bool,
	},
	Speaking {
		user_id: Snowflake,
		speaking: bool,
	},
	VoicePeer {
		user_id: Snowflake,
		present: bool,
	},
	UserAudio {
		user_id: Snowflake,
		audio: UserAudio,
	},
	VoiceStats {
		ping_ms: Option<u32>,
		loss: u64,
		jitter: f32,
		e2ee: bool,
		video_bitrate_bps: u32,
	},
	MicTestStarted,
	MicTestLevel {
		level_db: f32,
	},
	MicTestFinished,
	AccountSwitchRequested(Snowflake),
	VoiceLevels {
		input: f32,
		output: f32,
	},
	CameraChanged(bool),
	ScreenShareChanged(Option<StreamInfo>),
	Watching(Option<String>),
	RemoteVideo {
		user_id: Snowflake,
		kind: VideoKind,
		active: bool,
	},
	Loaded(Box<Loaded>),
	RecentMentionRemoved {
		message_id: Snowflake,
	},
	CommandRejected {
		reason: String,
	},
	BanRevoked {
		guild_id: Snowflake,
		user_id: Snowflake,
	},
	DeletesQueued(DeleteBatch),
	DeleteProgressed {
		batch: u64,
		message_id: Snowflake,
		status: DeleteStatus,
	},
	RiskyConfirmed {
		target: RiskyTarget,
		reason: RiskyReason,
	},
	ReferenceFetched {
		channel_id: Snowflake,
		message_id: Snowflake,
		message: Option<Box<Message>>,
	},
	UserFetched {
		channel_id: Snowflake,
		user: Box<User>,
	},
	KeptRestored {
		channel_id: Snowflake,
		deleted: Vec<(Message, DateTime<Utc>)>,
		versions: Vec<(Snowflake, Vec<MessageVersion>)>,
	},
	ShowDeletedMessages(bool),
	MediaRefreshed {
		channel_id: Snowflake,
		message: Box<Message>,
	},
	CommandIndexLoaded {
		scope: Snowflake,
		index: Arc<ApplicationCommandIndex>,
	},
	StatusSettings(protocol::ProtoStatusSettings),
	Autocomplete(crate::composer::Autocomplete),
}

#[derive(Debug)]
pub enum Event<'a> {
	Ready(&'a Ready),
	ReadySupplemental(&'a ReadySupplemental),
	Dispatch(&'a DispatchEvent),
	MessagesLoaded {
		channel_id: Snowflake,
		messages: Vec<Message>,
		direction: LoadDirection,
		reached_start: bool,
	},
	Local(LocalEvent),
}
