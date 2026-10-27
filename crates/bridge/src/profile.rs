use std::cmp::Reverse;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use media::cdn;
use media::{ImageRequest, Shape, SizeHint, Variant};
use protocol::{
	Activity, ActivityKind, ChannelType, PrimaryGuild, RelationshipKind, Snowflake, StatusKind, User,
};
use session::Badge;
use state::{ConnectionStatus, CustomStatus, RiskyLock, State};

use crate::display::Display;
use crate::dm_list::DmStatus;
use crate::images::{avatar_request, device_pixels, emoji_request, guild_icon_request};

pub const BANNER_PIXELS: (u32, u32) = (600, 300);
pub const BADGE_PIXELS: u32 = 64;
pub const TAG_BADGE_PIXELS: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
	None,
	Friend,
	Outgoing,
	Incoming,
	Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBadge {
	pub icon_url: String,
	pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildTag {
	pub text: String,
	pub guild_id: Option<Snowflake>,
	pub badge: Option<String>,
}

impl GuildTag {
	fn of(user: &User) -> Option<Self> {
		[user.primary_guild.as_ref(), user.clan.as_ref()]
			.into_iter()
			.flatten()
			.find_map(Self::from_primary)
	}

	fn from_primary(primary: &PrimaryGuild) -> Option<Self> {
		if primary.identity_enabled == Some(false) {
			return None;
		}
		let text = primary.tag.clone().filter(|tag| !tag.trim().is_empty())?;
		Some(Self {
			text,
			guild_id: primary.identity_guild_id,
			badge: primary.badge.clone().filter(|badge| !badge.is_empty()),
		})
	}

	pub fn badge_request(&self) -> Option<ImageRequest> {
		let (guild_id, badge) = (self.guild_id?, self.badge.as_deref()?);
		Some(ImageRequest::new(
			cdn::guild_tag_badge_url(guild_id.0, badge, TAG_BADGE_PIXELS),
			Variant::new(SizeHint::square(TAG_BADGE_PIXELS), Shape::Square),
		))
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileConnection {
	pub kind: String,
	pub name: String,
	pub verified: bool,
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

	pub fn glyph(&self) -> Option<&str> {
		self.emoji_id
			.is_none()
			.then_some(self.emoji_name.as_deref())
			.flatten()
	}

	pub fn emoji_request(&self, cdn: &str, points: f64, scale: f64) -> Option<ImageRequest> {
		let id = self.emoji_id?;
		Some(emoji_request(cdn, id, self.animated, points, scale))
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
		let emoji = activity.emoji.as_ref();
		Self {
			text: activity.state.clone().filter(|text| !text.is_empty()),
			emoji_name: emoji
				.and_then(|emoji| emoji.name.clone())
				.filter(|name| !name.is_empty()),
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
	pub name_extra: Option<String>,
	pub username: String,
	pub handle: String,
	pub member_avatar: Option<String>,
	pub banner: Option<String>,
	pub accent: Option<u32>,
	pub pronouns: Option<String>,
	pub tag: Option<GuildTag>,
	pub bio: Option<String>,
	pub badges: Vec<ProfileBadge>,
	pub decorations: Vec<Badge>,
	pub roles: Vec<RoleChip>,
	pub created: NaiveDate,
	pub joined: Option<NaiveDate>,
	pub guild_name: Option<String>,
	pub mutual_guilds: Vec<MutualGuild>,
	pub mutual_friends: Vec<MutualFriend>,
	pub mutual_friend_count: usize,
	pub connections: Vec<ProfileConnection>,
	pub status: DmStatus,
	pub custom_status: StatusLine,
	pub activities: Vec<ActivityLine>,
	pub relation: Relation,
	pub mine: bool,
	pub bot: bool,
	pub dm_channel: Option<Snowflake>,
	pub dm_lock: Option<RiskyLock>,
	pub loaded: bool,
	pub loading: bool,
}

impl ProfileCard {
	pub fn of(
		state: &State,
		user_id: Snowflake,
		guild_id: Option<Snowflake>,
		now: DateTime<Utc>,
	) -> Option<Self> {
		let user = state.user(user_id)?.as_ref().clone();
		let profile = state.profile(user_id);
		let member = guild_id
			.and_then(|id| state.guild(id))
			.and_then(|guild| guild.members.get(&user_id))
			.or_else(|| {
				guild_id
					.and(profile)
					.and_then(|profile| profile.guild_member.as_ref())
			});
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
			decorations: Vec::new(),
			id: user_id,
			guild_id,
			name: state.display_name(guild_id, user_id),
			name_extra: None,
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
			pronouns: profile
				.and_then(|profile| profile.pronouns.clone())
				.filter(|pronouns| !pronouns.is_empty()),
			tag: profile
				.and_then(|profile| GuildTag::of(&profile.user))
				.or_else(|| GuildTag::of(&user)),
			bio: profile
				.and_then(|profile| profile.bio.clone())
				.filter(|bio| !bio.trim().is_empty()),
			badges: profile
				.map(|profile| {
					profile
						.badges
						.iter()
						.map(|badge| ProfileBadge {
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
			guild_name: member
				.and(guild_id)
				.and_then(|id| state.guild(id))
				.map(|guild| guild.name.clone()),
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
						.map(|account| ProfileConnection {
							kind: account.kind.clone(),
							name: account.name.clone(),
							verified: account.verified,
						})
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
			loading: profile.is_none() && state.profile_loading(user_id),
			user,
		})
	}

	pub fn avatar_request(&self, cdn_base: &str, points: f64, scale: f64) -> Option<ImageRequest> {
		match (self.guild_id, self.member_avatar.as_deref()) {
			(Some(guild_id), Some(hash)) => {
				let pixels = device_pixels(points, scale);
				let url = cdn::guild_member_avatar_url(cdn_base, guild_id.0, self.id.0, hash, pixels, false);
				Some(ImageRequest::new(
					url,
					Variant::new(SizeHint::square(pixels), Shape::Circle),
				))
			}
			_ => avatar_request(cdn_base, &self.user, points, scale, false),
		}
	}

	pub fn banner_request(&self) -> Option<ImageRequest> {
		banner_request(self.id, self.banner.as_deref()?)
	}

	pub fn badge_requests(&self) -> Vec<ImageRequest> {
		self.badges
			.iter()
			.map(|badge| {
				ImageRequest::new(
					badge.icon_url.clone(),
					Variant::new(SizeHint::square(BADGE_PIXELS), Shape::Square),
				)
			})
			.collect()
	}

	pub fn guild_icon_request(
		guild: &MutualGuild,
		cdn_base: &str,
		points: f64,
		scale: f64,
	) -> Option<ImageRequest> {
		let hash = guild.icon.as_deref()?;
		Some(guild_icon_request(cdn_base, guild.id, hash, points, scale, false))
	}

	pub fn shown_with(mut self, state: &State, display: &Display) -> Self {
		if let Some(shown) = display.person_name(state, self.guild_id, &self.user) {
			self.name = shown.name;
			self.name_extra = shown.extra;
		}
		if let Some(decorations) = display.decorations(state, self.guild_id, &self.user) {
			self.decorations = decorations.profiles;
		}
		self
	}

	pub fn handle_line(&self) -> String {
		match &self.pronouns {
			Some(pronouns) => format!("{} \u{b7} {pronouns}", self.handle),
			None => self.handle.clone(),
		}
	}

	pub fn mutual_server_count(&self) -> usize {
		self.mutual_guilds.len()
	}
}

fn banner_request(user_id: Snowflake, hash: &str) -> Option<ImageRequest> {
	let (width, height) = BANNER_PIXELS;
	Some(ImageRequest::new(
		cdn::banner_url(user_id.0, hash, width, false),
		Variant::new(SizeHint::new(width, height), Shape::Square),
	))
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
		let custom = state
			.own_custom_status(now)
			.map(|custom| StatusLine::from_custom(&custom))
			.unwrap_or_default();
		return (DmStatus::from_kind(&state.own_status()), custom, activities);
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
	let text = |value: &Option<String>| value.clone().filter(|value| !value.trim().is_empty());
	let line = |heading: String| ActivityLine {
		heading,
		name: activity.name.clone(),
		details: text(&activity.details),
		state: text(&activity.state),
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
		},
		ActivityKind::Playing => line("Playing".to_owned()),
		ActivityKind::Streaming => line("Streaming".to_owned()),
		ActivityKind::Listening => line(format!("Listening to {}", activity.name)),
		ActivityKind::Watching => line(format!("Watching {}", activity.name)),
		ActivityKind::Competing => line("Competing in".to_owned()),
		_ => line("Activity".to_owned()),
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

pub fn dm_channel(state: &State, user_id: Snowflake) -> Option<Snowflake> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusChoice {
	Online,
	Idle,
	Dnd,
	Invisible,
}

impl StatusChoice {
	pub const ALL: [StatusChoice; 4] = [Self::Online, Self::Idle, Self::Dnd, Self::Invisible];

	pub fn of(kind: &StatusKind) -> Self {
		match kind {
			StatusKind::Idle => Self::Idle,
			StatusKind::Dnd => Self::Dnd,
			StatusKind::Invisible | StatusKind::Offline => Self::Invisible,
			_ => Self::Online,
		}
	}

	pub fn kind(self) -> StatusKind {
		match self {
			Self::Online => StatusKind::Online,
			Self::Idle => StatusKind::Idle,
			Self::Dnd => StatusKind::Dnd,
			Self::Invisible => StatusKind::Invisible,
		}
	}

	pub fn label(self) -> &'static str {
		match self {
			Self::Online => "Online",
			Self::Idle => "Idle",
			Self::Dnd => "Do Not Disturb",
			Self::Invisible => "Invisible",
		}
	}

	pub fn key(self) -> &'static str {
		match self {
			Self::Online => "online",
			Self::Idle => "idle",
			Self::Dnd => "dnd",
			Self::Invisible => "invisible",
		}
	}

	pub fn description(self) -> Option<&'static str> {
		match self {
			Self::Online | Self::Idle => None,
			Self::Dnd => Some("You will not receive desktop notifications"),
			Self::Invisible => Some("You will appear offline"),
		}
	}

	pub fn dot(self) -> DmStatus {
		match self {
			Self::Online => DmStatus::Online,
			Self::Idle => DmStatus::Idle,
			Self::Dnd => DmStatus::Dnd,
			Self::Invisible => DmStatus::Offline,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelUser {
	pub user: User,
	pub name: String,
	pub username: String,
	pub status: StatusChoice,
	pub custom_status: StatusLine,
	pub latency: Option<Duration>,
}

impl PanelUser {
	pub fn of(state: &State, now: DateTime<Utc>) -> Option<Self> {
		let user = state.current_user()?;
		let session = state.session();
		let latency = match session.connection {
			ConnectionStatus::Connected => session.latency,
			_ => None,
		};
		Some(Self {
			name: state.display_name(None, user.id),
			username: user.username.clone(),
			status: StatusChoice::of(&state.own_status()),
			custom_status: state
				.own_custom_status(now)
				.map(|custom| StatusLine::from_custom(&custom))
				.unwrap_or_default(),
			latency,
			user: user.clone(),
		})
	}

	pub fn banner_request(&self) -> Option<ImageRequest> {
		banner_request(self.user.id, self.user.banner.as_deref()?)
	}

	pub fn accent(&self) -> Option<u32> {
		self.user.accent_color.and_then(|color| u32::try_from(color).ok())
	}

	pub fn subtitle(&self) -> &str {
		self.custom_status.text.as_deref().unwrap_or(&self.username)
	}

	pub fn latency_text(&self) -> String {
		match self.latency {
			Some(latency) => format!("Latency: {} ms", latency.as_millis()),
			None => "Connecting".to_owned(),
		}
	}
}
