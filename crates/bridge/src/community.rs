use chrono::{DateTime, TimeDelta, Utc};
use protocol::{ChannelType, NotificationLevel, Snowflake};
use state::{RiskyReason, State};

use crate::sidebar::channel_icon;
use crate::switcher::fuzzy_score;

pub const FORWARD_RESULTS: usize = 12;
pub const FORWARD_TARGETS: usize = 5;
const CODE_MIN: usize = 2;
const CODE_MAX: usize = 32;

pub const MUTE_WINDOWS: [(&str, Option<i64>); 5] = [
	("For 15 Minutes", Some(15)),
	("For 1 Hour", Some(60)),
	("For 8 Hours", Some(480)),
	("For 24 Hours", Some(1440)),
	("Until I turn it back on", None),
];

pub const INVITE_AGES: [(&str, u32); 7] = [
	("30 minutes", 1800),
	("1 hour", 3600),
	("6 hours", 21_600),
	("12 hours", 43_200),
	("1 day", 86_400),
	("7 days", 604_800),
	("Never", 0),
];

pub const INVITE_USES: [(&str, u32); 7] = [
	("No limit", 0),
	("1 use", 1),
	("5 uses", 5),
	("10 uses", 10),
	("25 uses", 25),
	("50 uses", 50),
	("100 uses", 100),
];

pub const DEFAULT_INVITE_AGE: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardTarget {
	pub channel_id: Snowflake,
	pub name: String,
	pub context: Option<String>,
	pub icon: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildNotifications {
	pub level: NotificationLevel,
	pub suppress_everyone: bool,
	pub suppress_roles: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberGate {
	Screening,
	Onboarding,
}

pub fn invite_code(input: &str) -> Option<String> {
	let cleaned = input.trim().split(['?', '#']).next()?.trim_end_matches('/');
	let tail = cleaned.rsplit('/').next()?;
	let valid = (CODE_MIN..=CODE_MAX).contains(&tail.len())
		&& tail
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_');
	valid.then(|| tail.to_owned())
}

pub fn invite_link(code: &str) -> String {
	format!("https://discord.gg/{code}")
}

pub fn forward_targets(state: &State, now: DateTime<Utc>) -> Vec<ForwardTarget> {
	let mut found = Vec::new();
	for summary in state.guild_summaries() {
		for node in state.guild_channel_tree(summary.id) {
			for channel in node.children.iter().filter(|channel| {
				!channel.locked
					&& matches!(
						channel.kind,
						ChannelType::GuildText | ChannelType::GuildAnnouncement
					) && !gated(state, channel.id, now)
			}) {
				found.push(ForwardTarget {
					channel_id: channel.id,
					name: channel.name.clone(),
					context: Some(summary.name.clone()),
					icon: channel_icon(channel.kind),
				});
			}
		}
	}
	found.extend(state.dm_rows().into_iter().map(|row| ForwardTarget {
		channel_id: row.channel_id,
		icon: if row.recipients.len() > 1 {
			"group"
		} else {
			"at_sign"
		},
		name: row.name,
		context: None,
	}));
	found
}

fn gated(state: &State, channel_id: Snowflake, now: DateTime<Utc>) -> bool {
	state
		.channel_lock(channel_id, now)
		.is_some_and(|lock| lock.reason.is_gate())
}

pub fn rank_forward(targets: &[ForwardTarget], query: &str) -> Vec<usize> {
	let query = query.trim();
	if query.is_empty() {
		return (0..targets.len().min(FORWARD_RESULTS)).collect();
	}
	let needle: Vec<char> = query.trim_start_matches('#').chars().collect();
	let mut scored: Vec<(i32, usize)> = targets
		.iter()
		.enumerate()
		.filter_map(|(index, target)| Some((fuzzy_score(&needle, &target.name)?, index)))
		.collect();
	scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
	scored
		.into_iter()
		.take(FORWARD_RESULTS)
		.map(|(_, index)| index)
		.collect()
}

pub fn guild_notifications(state: &State, guild_id: Snowflake) -> GuildNotifications {
	state.guild_settings(Some(guild_id)).map_or(
		GuildNotifications {
			level: NotificationLevel::AllMessages,
			suppress_everyone: false,
			suppress_roles: false,
		},
		|settings| GuildNotifications {
			level: settings
				.message_notifications
				.unwrap_or(NotificationLevel::AllMessages),
			suppress_everyone: settings.suppress_everyone,
			suppress_roles: settings.suppress_roles,
		},
	)
}

pub fn channel_notifications(state: &State, channel_id: Snowflake) -> NotificationLevel {
	let guild_id = state.channel_guild(channel_id);
	state
		.guild_settings(guild_id)
		.and_then(|settings| {
			settings
				.channel_overrides
				.iter()
				.find(|entry| entry.channel_id == channel_id)
				.and_then(|entry| entry.message_notifications)
		})
		.unwrap_or(NotificationLevel::Inherit)
}

pub fn invite_channel(state: &State, guild_id: Snowflake) -> Option<Snowflake> {
	state
		.guild_channel_tree(guild_id)
		.iter()
		.flat_map(|node| node.children.iter())
		.find(|row| !row.locked && row.kind == ChannelType::GuildText)
		.map(|row| row.id)
}

pub fn mute_until(minutes: Option<i64>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
	minutes.map(|minutes| now + TimeDelta::minutes(minutes))
}

pub fn guild_gate(state: &State, guild_id: Snowflake, now: DateTime<Utc>) -> Option<MemberGate> {
	let guild = state.guild(guild_id)?;
	guild
		.channels
		.values()
		.find_map(|channel| state.channel_lock(channel.id, now))
		.and_then(|lock| match lock.reason {
			RiskyReason::Screening => Some(MemberGate::Screening),
			RiskyReason::Onboarding => Some(MemberGate::Onboarding),
			_ => None,
		})
}

pub fn onboarding_ready(state: &State, guild_id: Snowflake) -> bool {
	state
		.onboarding(guild_id)
		.is_some_and(|onboarding| !onboarding.prompts.is_empty())
}
