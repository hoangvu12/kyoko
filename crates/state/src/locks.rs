use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use protocol::{
	Channel, ChannelType, MEMBER_BYPASSES_VERIFICATION, MEMBER_COMPLETED_ONBOARDING,
	MEMBER_STARTED_ONBOARDING, RelationshipKind, Snowflake, VerificationLevel,
};

use crate::guild::GuildState;
use crate::state::State;

const ONBOARDING_FEATURE: &str = "GUILD_ONBOARDING";
const ACCOUNT_AGE_GATE: Duration = Duration::minutes(5);
const MEMBERSHIP_AGE_GATE: Duration = Duration::minutes(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskyTarget {
	Channel(Snowflake),
	User(Snowflake),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskyReason {
	StrangerDm,
	MessageRequest,
	SpamDm,
	EmptyChannel,
	Screening,
	Onboarding,
	Verification,
}

impl RiskyReason {
	pub fn code(self) -> &'static str {
		match self {
			Self::StrangerDm => "stranger_dm",
			Self::MessageRequest => "message_request",
			Self::SpamDm => "spam_dm",
			Self::EmptyChannel => "empty_channel",
			Self::Screening => "screening",
			Self::Onboarding => "onboarding",
			Self::Verification => "verification",
		}
	}

	pub fn explanation(self) -> &'static str {
		match self {
			Self::StrangerDm => {
				"You haven't talked with this person before. Messaging strangers from a new conversation can get your account temporarily blocked."
			}
			Self::MessageRequest => {
				"This conversation is a message request. Replying from kyoko can trip Discord's spam checks; accept it in the official client first."
			}
			Self::SpamDm => {
				"Discord marked this conversation as spam. Replying can trip Discord's spam checks."
			}
			Self::EmptyChannel => {
				"This channel has no messages yet. Being the first to post can trip Discord's spam checks."
			}
			Self::Screening => {
				"This server asks new members to accept its rules first. Open the server to read and accept them, or do it in the official client."
			}
			Self::Onboarding => {
				"This server's onboarding isn't finished. Open the server to finish it, or do it in the official client."
			}
			Self::Verification => {
				"This server's verification level doesn't allow you to talk here yet. Meet it in the official client or wait until it lifts."
			}
		}
	}

	pub fn is_gate(self) -> bool {
		matches!(self, Self::Screening | Self::Onboarding | Self::Verification)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskyLock {
	pub target: RiskyTarget,
	pub reason: RiskyReason,
	pub until: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub(crate) struct RiskyLocks {
	pub(crate) enabled: bool,
	pub(crate) confirmed: HashSet<(RiskyTarget, RiskyReason)>,
}

impl Default for RiskyLocks {
	fn default() -> Self {
		Self {
			enabled: true,
			confirmed: HashSet::new(),
		}
	}
}

struct Gate {
	reason: RiskyReason,
	until: Option<DateTime<Utc>>,
}

fn gate(reason: RiskyReason) -> Gate {
	Gate { reason, until: None }
}

fn timed(reason: RiskyReason, lifts: DateTime<Utc>, now: DateTime<Utc>) -> Option<Gate> {
	(lifts > now).then_some(Gate {
		reason,
		until: Some(lifts),
	})
}

fn has_flag(flags: Option<u64>, flag: u64) -> bool {
	flags.is_some_and(|flags| flags & flag != 0)
}

fn level_rank(level: VerificationLevel) -> u8 {
	match level {
		VerificationLevel::None => 0,
		VerificationLevel::Low => 1,
		VerificationLevel::Medium => 2,
		VerificationLevel::High => 3,
		VerificationLevel::VeryHigh => 4,
		VerificationLevel::Unknown(_) => 0,
	}
}

impl State {
	pub fn safety_locks(&self) -> bool {
		self.risky.enabled
	}

	pub fn set_safety_locks(&mut self, enabled: bool) {
		self.risky.enabled = enabled;
	}

	pub fn is_membership_pending(&self, guild_id: Snowflake) -> bool {
		let (Some(guild), Some(me)) = (self.guilds.get(&guild_id), self.current_user_id()) else {
			return false;
		};
		let Some(member) = guild.members.get(&me) else {
			return false;
		};
		member.pending == Some(true)
			|| (has_feature(guild, ONBOARDING_FEATURE)
				&& has_flag(member.flags, MEMBER_STARTED_ONBOARDING)
				&& !has_flag(member.flags, MEMBER_COMPLETED_ONBOARDING))
	}

	pub fn is_risky_confirmed(&self, target: RiskyTarget, reason: RiskyReason) -> bool {
		self.risky.confirmed.contains(&(target, reason))
	}

	pub fn channel_lock(&self, channel_id: Snowflake, now: DateTime<Utc>) -> Option<RiskyLock> {
		if !self.risky.enabled {
			return None;
		}
		let target = RiskyTarget::Channel(channel_id);
		let channel = self.channel(channel_id)?;
		let (reason, until) = match self.channel_guild(channel_id) {
			Some(guild_id) => self.guild_channel_risk(guild_id, channel, now)?,
			None => (self.private_channel_risk(channel)?, None),
		};
		Some(RiskyLock {
			target,
			reason,
			until,
		})
	}

	pub fn dm_lock(&self, user_id: Snowflake) -> Option<RiskyLock> {
		if !self.risky.enabled || self.current_user_id() == Some(user_id) || self.is_friend(user_id) {
			return None;
		}
		if self.is_risky_confirmed(RiskyTarget::User(user_id), RiskyReason::StrangerDm) {
			return None;
		}
		let existing = self
			.private_channels
			.values()
			.find(|channel| channel.kind == ChannelType::Dm && channel.recipient_ids.contains(&user_id));
		if existing.is_some_and(|channel| !self.has_no_history(channel)) {
			return None;
		}
		Some(RiskyLock {
			target: RiskyTarget::User(user_id),
			reason: RiskyReason::StrangerDm,
			until: None,
		})
	}

	pub(crate) fn confirm_risky(&mut self, target: RiskyTarget, reason: RiskyReason) -> bool {
		self.risky.confirmed.insert((target, reason))
	}

	fn private_channel_risk(&self, channel: &Channel) -> Option<RiskyReason> {
		let unconfirmed = |reason| !self.is_risky_confirmed(RiskyTarget::Channel(channel.id), reason);
		let direct = matches!(channel.kind, ChannelType::Dm | ChannelType::GroupDm);
		if direct && channel.is_spam == Some(true) && unconfirmed(RiskyReason::SpamDm) {
			return Some(RiskyReason::SpamDm);
		}
		if direct && channel.is_message_request == Some(true) && unconfirmed(RiskyReason::MessageRequest) {
			return Some(RiskyReason::MessageRequest);
		}
		let recipient = channel.recipient_ids.first().copied();
		let stranger = channel.kind == ChannelType::Dm
			&& recipient.is_some_and(|user_id| {
				!self.is_friend(user_id)
					&& !self.is_risky_confirmed(RiskyTarget::User(user_id), RiskyReason::StrangerDm)
			}) && self.has_no_history(channel);
		(stranger && unconfirmed(RiskyReason::StrangerDm)).then_some(RiskyReason::StrangerDm)
	}

	fn guild_channel_risk(
		&self,
		guild_id: Snowflake,
		channel: &Channel,
		now: DateTime<Utc>,
	) -> Option<(RiskyReason, Option<DateTime<Utc>>)> {
		let unconfirmed = |reason| !self.is_risky_confirmed(RiskyTarget::Channel(channel.id), reason);
		if let Some(gate) = self.guild_gate(guild_id, now)
			&& unconfirmed(gate.reason)
		{
			return Some((gate.reason, gate.until));
		}
		let postable = matches!(
			channel.kind,
			ChannelType::GuildText | ChannelType::GuildAnnouncement
		);
		(postable && self.has_no_history(channel) && unconfirmed(RiskyReason::EmptyChannel))
			.then_some((RiskyReason::EmptyChannel, None))
	}

	fn guild_gate(&self, guild_id: Snowflake, now: DateTime<Utc>) -> Option<Gate> {
		let guild = self.guilds.get(&guild_id)?;
		let me = self.current_user.as_ref()?;
		if guild.owner_id == Some(me.id) {
			return None;
		}
		let member = guild.members.get(&me.id)?;
		if member.pending == Some(true) {
			return Some(gate(RiskyReason::Screening));
		}
		if has_feature(guild, ONBOARDING_FEATURE)
			&& has_flag(member.flags, MEMBER_STARTED_ONBOARDING)
			&& !has_flag(member.flags, MEMBER_COMPLETED_ONBOARDING)
		{
			return Some(gate(RiskyReason::Onboarding));
		}
		if !member.roles.is_empty() || has_flag(member.flags, MEMBER_BYPASSES_VERIFICATION) {
			return None;
		}
		let level = guild.verification_level.map_or(0, level_rank);
		if level >= 1 && me.verified == Some(false) {
			return Some(gate(RiskyReason::Verification));
		}
		if level >= 4 && me.phone == Some(None) {
			return Some(gate(RiskyReason::Verification));
		}
		if level >= 2
			&& let Some(gate) = timed(
				RiskyReason::Verification,
				me.id.timestamp() + ACCOUNT_AGE_GATE,
				now,
			) {
			return Some(gate);
		}
		if level >= 3
			&& let Some(joined) = member.joined_at
		{
			return timed(RiskyReason::Verification, joined + MEMBERSHIP_AGE_GATE, now);
		}
		None
	}

	fn has_no_history(&self, channel: &Channel) -> bool {
		match self.messages.get(&channel.id) {
			Some(cache) if !cache.ordered.is_empty() => false,
			Some(cache) if cache.reached_start => true,
			_ => channel.last_message_id.is_none(),
		}
	}

	fn is_friend(&self, user_id: Snowflake) -> bool {
		self.relationships
			.get(&user_id)
			.is_some_and(|relationship| relationship.kind == RelationshipKind::Friend)
	}
}

fn has_feature(guild: &GuildState, feature: &str) -> bool {
	guild.features.iter().any(|known| known == feature)
}
