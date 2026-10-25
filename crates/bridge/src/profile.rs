use std::cmp::Reverse;

use chrono::{DateTime, NaiveDate, Utc};
use media::cdn;
use protocol::{
	Activity, ActivityKind, ChannelType, PartialEmoji, RelationshipKind, Snowflake, StatusKind, User,
};
use state::{CustomStatus, RiskyLock, State};

use crate::dm_list::DmStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
	None,
	Friend,
	Outgoing,
	Incoming,
	Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBadgeIcon {
	pub icon_url: String,
	pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleChip {
	pub name: String,
	pub color: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutualGuild {
	pub id: Snowflake,
	pub name: String,
	pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MutualFriend {
	pub id: Snowflake,
	pub name: String,
	pub user: Option<User>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLine {
	pub heading: String,
	pub name: String,
	pub details: Option<String>,
	pub state: Option<String>,
	pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusLine {
	pub text: Option<String>,
	pub emoji_name: Option<String>,
	pub emoji_id: Option<Snowflake>,
	pub animated: bool,
}

impl StatusLine {
	pub fn is_empty(&self) -> bool {
		self.text.is_none() && self.emoji_name.is_none() && self.emoji_id.is_none()
	}

	fn from_custom(custom: &CustomStatus) -> Self {
		let emoji = custom.emoji.as_ref();
		Self {
			text: custom.text.clone(),
			emoji_name: emoji.and_then(|emoji| emoji.name.clone()),
			emoji_id: emoji.and_then(|emoji| emoji.id),
			animated: emoji.is_some_and(|emoji| emoji.animated),
		}
	}

	fn from_activity(activity: &Activity) -> Self {
		let emoji: Option<&PartialEmoji> = activity.emoji.as_ref();
		Self {
			text: activity.state.clone().filter(|text| !text.is_empty()),
			emoji_name: emoji
				.filter(|emoji| emoji.id.is_none())
				.and_then(|emoji| emoji.name.clone()),
			emoji_id: emoji.and_then(|emoji| emoji.id),
			animated: emoji.is_some_and(|emoji| emoji.animated),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileCard {
	pub id: Snowflake,
	pub guild_id: Option<Snowflake>,
	pub user: User,
	pub name: String,
	pub username: String,
	pub handle: String,
	pub member_avatar: Option<String>,
	pub banner: Option<String>,
	pub accent: Option<u32>,
	pub pronouns: Option<String>,
	pub bio: Option<String>,
	pub badges: Vec<ProfileBadgeIcon>,
	pub roles: Vec<RoleChip>,
	pub created: NaiveDate,
	pub joined: Option<NaiveDate>,
	pub mutual_guilds: Vec<MutualGuild>,
	pub mutual_friends: Vec<MutualFriend>,
	pub mutual_friend_count: usize,
	pub connections: Vec<(String, String)>,
	pub status: DmStatus,
	pub custom_status: StatusLine,
	pub activities: Vec<ActivityLine>,
	pub relation: Relation,
	pub mine: bool,
	pub bot: bool,
	pub dm_channel: Option<Snowflake>,
	pub dm_lock: Option<RiskyLock>,
	pub loaded: bool,
}

impl ProfileCard {
	pub fn of(state: &State, user_id: Snowflake, guild_id: Option<Snowflake>, now: DateTime<Utc>) -> Option<Self> {
		let user = state.user(user_id)?.as_ref().clone();
		let profile = state.profile(user_id);
		let guild = guild_id.and_then(|id| state.guild(id));
		let member = guild.and_then(|guild| guild.members.get(&user_id));
		let mine = state.current_user_id() == Some(user_id);
		let handle = match user.discriminator.as_deref() {
			Some(tag) if tag != "0" && !tag.is_empty() => format!("{}#{tag}", user.username),
			_ => user.username.clone(),
		};
		let (status, custom_status, activities) = presence_of(state, user_id, mine, now);
		let mutual_guilds = profile
			.map(|profile| {
				profile
					.mutual_guilds
					.iter()
					.filter_map(|id| state.guild(*id))
					.map(|guild| MutualGuild {
						id: guild.id,
						name: guild.name.clone(),
						icon: guild.icon.clone(),
					})
					.collect()
			})
			.unwrap_or_default();
		let mutual_friends: Vec<MutualFriend> = profile
			.map(|profile| {
				profile
					.mutual_friends
					.iter()
					.map(|id| MutualFriend {
						id: *id,
						name: state.display_name(None, *id),
						user: state.user(*id).map(|user| user.as_ref().clone()),
					})
					.collect()
			})
			.unwrap_or_default();
		Some(Self {
			id: user_id,
			guild_id,
			name: state.display_name(guild_id, user_id),
			username: user.username.clone(),
			handle,
			member_avatar: member.and_then(|member| member.avatar.clone()),
			banner: profile
				.and_then(|profile| profile.banner.clone())
				.or_else(|| user.banner.clone()),
			accent: profile
				.and_then(|profile| profile.accent_color)
				.or(user.accent_color)
				.and_then(|color| u32::try_from(color).ok()),
			pronouns: profile.and_then(|profile| profile.pronouns.clone()),
			bio: profile.and_then(|profile| profile.bio.clone()),
			badges: profile
				.map(|profile| {
					profile
						.badges
						.iter()
						.map(|badge| ProfileBadgeIcon {
							icon_url: cdn::badge_icon_url(&badge.icon),
							description: badge.description.clone(),
						})
						.collect()
				})
				.unwrap_or_default(),
			roles: roles(state, guild_id, user_id),
			created: user_id.timestamp().date_naive(),
			joined: member
				.and_then(|member| member.joined_at)
				.map(|joined| joined.date_naive()),
			mutual_friend_count: profile
				.map_or(0, |profile| profile.mutual_friends_count as usize)
				.max(mutual_friends.len()),
			mutual_guilds,
			mutual_friends,
			connections: profile
				.map(|profile| {
					profile
						.connected_accounts
						.iter()
						.map(|account| (account.kind.clone(), account.name.clone()))
						.collect()
				})
				.unwrap_or_default(),
			status,
			custom_status,
			activities,
			relation: relation(state, user_id),
			mine,
			bot: user.bot == Some(true),
			dm_channel: dm_channel(state, user_id),
			dm_lock: (!mine).then(|| state.dm_lock(user_id)).flatten(),
			loaded: profile.is_some(),
			user,
		})
	}

	pub fn avatar_url(&self, cdn_base: &str, pixels: u32) -> String {
		match (self.guild_id, self.member_avatar.as_deref()) {
			(Some(guild_id), Some(hash)) => {
				cdn::guild_member_avatar_url(cdn_base, guild_id.0, self.id.0, hash, pixels, false)
			}
			_ => match self.user.avatar.as_deref() {
				Some(hash) => cdn::still_avatar_url(cdn_base, self.id.0, hash, pixels),
				None => cdn::default_avatar_url(cdn_base, self.id.0, self.user.discriminator.as_deref()),
			},
		}
	}

	pub fn banner_url(&self, pixels: u32) -> Option<String> {
		self.banner
			.as_deref()
			.map(|hash| cdn::banner_url(self.id.0, hash, pixels, false))
	}

	pub fn handle_line(&self) -> String {
		match &self.pronouns {
			Some(pronouns) => format!("{} \u{b7} {pronouns}", self.handle),
			None => self.handle.clone(),
		}
	}
}

fn presence_of(
	state: &State,
	user_id: Snowflake,
	mine: bool,
	now: DateTime<Utc>,
) -> (DmStatus, StatusLine, Vec<ActivityLine>) {
	let presence = state.presence(user_id);
	let activities: Vec<ActivityLine> = presence
		.map(|presence| presence.activities.iter().filter_map(activity_line).collect())
		.unwrap_or_default();
	if mine {
		let status = match state.own_status() {
			StatusKind::Invisible => DmStatus::Offline,
			other => DmStatus::from_kind(&other),
		};
		let custom = state
			.own_custom_status(now)
			.map(|custom| StatusLine::from_custom(&custom))
			.unwrap_or_default();
		return (status, custom, activities);
	}
	let custom = presence
		.and_then(|presence| {
			presence
				.activities
				.iter()
				.find(|activity| activity.kind == ActivityKind::Custom)
		})
		.map(StatusLine::from_activity)
		.unwrap_or_default();
	(DmStatus::of(state, user_id), custom, activities)
}

pub fn activity_line(activity: &Activity) -> Option<ActivityLine> {
	let text = |value: &Option<String>| value.clone().filter(|value| !value.is_empty());
	let line = |heading: &str| ActivityLine {
		heading: heading.to_owned(),
		name: activity.name.clone(),
		details: text(&activity.details),
		state: text(&activity.state),
		extra: None,
	};
	Some(match activity.kind {
		ActivityKind::Custom => return None,
		ActivityKind::Listening if activity.name == "Spotify" => ActivityLine {
			heading: "Listening to Spotify".to_owned(),
			name: text(&activity.details).unwrap_or_else(|| activity.name.clone()),
			details: text(&activity.state).map(|artist| format!("by {artist}")),
			state: activity
				.assets
				.as_ref()
				.and_then(|assets| text(&assets.large_text))
				.map(|album| format!("on {album}")),
			extra: None,
		},
		ActivityKind::Playing => line("Playing a game"),
		ActivityKind::Streaming => ActivityLine {
			extra: text(&activity.url),
			..line("Live on Streaming")
		},
		ActivityKind::Listening => line(&format!("Listening to {}", activity.name)),
		ActivityKind::Watching => line(&format!("Watching {}", activity.name)),
		ActivityKind::Competing => line("Competing"),
		_ => line("Activity"),
	})
}

fn roles(state: &State, guild_id: Option<Snowflake>, user_id: Snowflake) -> Vec<RoleChip> {
	let Some(guild) = guild_id.and_then(|id| state.guild(id)) else {
		return Vec::new();
	};
	let Some(member) = guild.members.get(&user_id) else {
		return Vec::new();
	};
	let mut found: Vec<_> = member
		.roles
		.iter()
		.filter(|id| **id != guild.id)
		.filter_map(|id| guild.roles.get(id))
		.collect();
	found.sort_by_key(|role| Reverse(role.position));
	found
		.into_iter()
		.map(|role| RoleChip {
			name: role.name.clone(),
			color: u32::try_from(role.color).ok().filter(|color| *color != 0),
		})
		.collect()
}

fn relation(state: &State, user_id: Snowflake) -> Relation {
	state
		.relationships()
		.find(|relationship| relationship.id == user_id)
		.map_or(Relation::None, |relationship| match relationship.kind {
			RelationshipKind::Friend => Relation::Friend,
			RelationshipKind::OutgoingRequest => Relation::Outgoing,
			RelationshipKind::IncomingRequest => Relation::Incoming,
			RelationshipKind::Blocked => Relation::Blocked,
			_ => Relation::None,
		})
}

fn dm_channel(state: &State, user_id: Snowflake) -> Option<Snowflake> {
	state
		.dm_rows()
		.into_iter()
		.find(|row| {
			row.recipients == [user_id]
				&& state
					.channel(row.channel_id)
					.is_some_and(|channel| channel.kind == ChannelType::Dm)
		})
		.map(|row| row.channel_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelUser {
	pub id: Snowflake,
	pub name: String,
	pub username: String,
	pub status: &'static str,
	pub custom_status: StatusLine,
}

impl PanelUser {
	pub fn of(state: &State, now: DateTime<Utc>) -> Option<Self> {
		let user = state.current_user()?;
		let status = match state.own_status() {
			StatusKind::Idle => "idle",
			StatusKind::Dnd => "dnd",
			StatusKind::Invisible => "invisible",
			_ => "online",
		};
		Some(Self {
			id: user.id,
			name: state.display_name(None, user.id),
			username: user.username.clone(),
			status,
			custom_status: state
				.own_custom_status(now)
				.map(|custom| StatusLine::from_custom(&custom))
				.unwrap_or_default(),
		})
	}

	pub fn dot(&self) -> &'static str {
		match self.status {
			"invisible" => "offline",
			other => other,
		}
	}
}
