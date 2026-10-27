use chrono::{DateTime, NaiveDate, Utc};
use media::ImageRequest;
use protocol::{ChannelType, Snowflake, User};
use state::{Changes, State};

use crate::dm_list::DmStatus;
use crate::images::avatar_request;
use crate::profile::{MutualFriend, MutualGuild, ProfileCard};

#[derive(Debug, Clone, PartialEq)]
pub enum DmPanel {
	Empty,
	Profile(Box<DmProfile>),
	Members(Vec<GroupMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DmProfile {
	pub card: ProfileCard,
	pub since: String,
	pub loading: bool,
	pub mutual_servers: Option<usize>,
	pub mutual_friends: Option<usize>,
}

pub const MUTUAL_ICONS: usize = 3;

fn mutual_text(count: Option<usize>, one: &str) -> Option<String> {
	match count? {
		0 => None,
		1 => Some(format!("1 {one}")),
		count => Some(format!("{count} {one}s")),
	}
}

impl DmProfile {
	pub fn mutual_servers_text(&self) -> Option<String> {
		mutual_text(self.mutual_servers, "Mutual Server")
	}

	pub fn mutual_friends_text(&self) -> Option<String> {
		mutual_text(self.mutual_friends, "Mutual Friend")
	}

	pub fn mutual_server_icons(&self) -> &[MutualGuild] {
		let guilds = &self.card.mutual_guilds;
		&guilds[..guilds.len().min(MUTUAL_ICONS)]
	}

	pub fn mutual_friend_icons(&self) -> &[MutualFriend] {
		let friends = &self.card.mutual_friends;
		&friends[..friends.len().min(MUTUAL_ICONS)]
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupMember {
	pub id: Snowflake,
	pub user: User,
	pub name: String,
	pub status: DmStatus,
	pub activity: Option<String>,
	pub owner: bool,
}

impl GroupMember {
	pub fn avatar_request(&self, cdn: &str, points: f64, scale: f64) -> Option<ImageRequest> {
		avatar_request(cdn, &self.user, points, scale, false)
	}
}

impl DmPanel {
	pub fn of(state: &State, channel_id: Snowflake, now: DateTime<Utc>) -> Self {
		let Some(channel) = state.channel(channel_id) else {
			return Self::Empty;
		};
		match channel.kind {
			ChannelType::Dm => channel
				.recipient_ids
				.first()
				.and_then(|user_id| ProfileCard::of(state, *user_id, None, now))
				.map_or(Self::Empty, |card| Self::Profile(Box::new(DmProfile::of(card)))),
			ChannelType::GroupDm => {
				let mut people = channel.recipient_ids.clone();
				if let Some(me) = state.current_user_id().filter(|me| !people.contains(me)) {
					people.push(me);
				}
				Self::Members(group_members(state, &people, channel.owner_id))
			}
			_ => Self::Empty,
		}
	}

	pub fn people(&self) -> Vec<Snowflake> {
		match self {
			Self::Empty => Vec::new(),
			Self::Profile(profile) => vec![profile.card.id],
			Self::Members(members) => members.iter().map(|member| member.id).collect(),
		}
	}

	pub fn touched_by(&self, changes: &Changes) -> bool {
		changes.private_channels
			|| changes.relationships
			|| changes.current_user
			|| changes.user_settings
			|| changes.guild_list
			|| self
				.people()
				.into_iter()
				.any(|user_id| changes.presence.contains(user_id) || changes.loaded.contains(user_id))
	}
}

impl DmProfile {
	fn of(card: ProfileCard) -> Self {
		let (mutual_servers, mutual_friends) = if card.loaded && !card.mine {
			(Some(card.mutual_server_count()), Some(card.mutual_friend_count))
		} else {
			(None, None)
		};
		Self {
			since: since_label(card.created),
			loading: card.loading,
			mutual_servers,
			mutual_friends,
			card,
		}
	}
}

pub fn since_label(day: NaiveDate) -> String {
	day.format("%b %-d, %Y").to_string()
}

fn group_members(state: &State, people: &[Snowflake], owner: Option<Snowflake>) -> Vec<GroupMember> {
	let mut members: Vec<GroupMember> = people
		.iter()
		.filter_map(|user_id| {
			let user = state.user(*user_id)?.as_ref().clone();
			let mine = state.current_user_id() == Some(*user_id);
			Some(GroupMember {
				id: *user_id,
				name: state.display_name(None, *user_id),
				status: if mine {
					DmStatus::from_kind(&state.own_status())
				} else {
					DmStatus::of(state, *user_id)
				},
				activity: state.activity_summary(*user_id),
				owner: owner == Some(*user_id),
				user,
			})
		})
		.collect();
	members.sort_by_key(|member| (member.name.to_lowercase(), member.id));
	members
}
