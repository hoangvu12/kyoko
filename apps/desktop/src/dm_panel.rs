use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use bridge::{DmPanel, DmProfile, GroupMember, ProfileCard, Relation};
use chrono::Utc;
use media::{ImageName, ImageRequest};
use protocol::Snowflake;
use session::{Command, FriendSource};
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::record;

const PROFILE_AVATAR: f64 = 80.;
const MEMBER_AVATAR: f64 = 32.;
const MUTUAL_ICON: f64 = 20.;

pub(crate) struct DmPanelView {
	open: Cell<Option<Snowflake>>,
	panel: RefCell<DmPanel>,
	images: RefCell<HashMap<String, ImageName>>,
	shown: RefCell<Vec<String>>,
	profile: Shared,
	members: Shared,
}

impl DmPanelView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			open: Cell::new(None),
			panel: RefCell::new(DmPanel::Empty),
			images: RefCell::new(HashMap::new()),
			shown: RefCell::new(Vec::new()),
			profile: host.shared("dms.profile", empty_profile()),
			members: host.shared("dms.members", Value::List(Rc::new([]))),
		}
	}

	pub(crate) fn reset(&self) {
		self.open.set(None);
		*self.panel.borrow_mut() = DmPanel::Empty;
		self.images.borrow_mut().clear();
		self.shown.borrow_mut().clear();
		self.profile.set(empty_profile());
		self.members.set(Value::List(Rc::new([])));
	}

	pub(crate) fn images(&self) -> Vec<String> {
		self.shown.borrow().clone()
	}

	pub(crate) fn set_open(&self, channel_id: Option<Snowflake>) -> bool {
		if self.open.get() == channel_id {
			return false;
		}
		self.open.set(channel_id);
		true
	}

	pub(crate) fn recipient(&self) -> Option<Snowflake> {
		match &*self.panel.borrow() {
			DmPanel::Profile(profile) => Some(profile.card.id),
			_ => None,
		}
	}

	pub(crate) fn friend_command(&self) -> Option<Command> {
		let panel = self.panel.borrow();
		let DmPanel::Profile(profile) = &*panel else {
			return None;
		};
		let card = &profile.card;
		match card.relation {
			Relation::None if !card.mine && !card.bot => Some(Command::AddFriend {
				username: card.username.clone(),
				from: FriendSource::Profile,
			}),
			Relation::Incoming => Some(Command::AcceptFriend { user_id: card.id }),
			_ => None,
		}
	}

	pub(crate) fn refresh(&self, state: &State, changes: Option<&Changes>) -> bool {
		if let Some(changes) = changes
			&& !self.panel.borrow().touched_by(changes)
		{
			return false;
		}
		let next = self.open.get().map_or(DmPanel::Empty, |channel_id| {
			DmPanel::of(state, channel_id, Utc::now())
		});
		if *self.panel.borrow() == next {
			return false;
		}
		*self.panel.borrow_mut() = next;
		true
	}

	pub(crate) fn publish(&self, icons: &Icons<'_>) -> bool {
		let mut names = Vec::new();
		let panel = self.panel.borrow();
		match &*panel {
			DmPanel::Profile(profile) => {
				let avatar = profile
					.card
					.avatar_request(icons.cdn, PROFILE_AVATAR, icons.scale)
					.map(|request| self.image(request, icons));
				let banner = profile
					.card
					.banner_request(icons.cdn)
					.map(|request| self.image(request, icons));
				let tag = profile
					.card
					.tag
					.as_ref()
					.and_then(bridge::GuildTag::badge_request)
					.map(|request| self.image(request, icons));
				names.extend(avatar.clone());
				names.extend(banner.clone());
				names.extend(tag.clone());
				let servers: Vec<Value> = profile
					.mutual_server_icons()
					.iter()
					.map(|guild| {
						let icon =
							ProfileCard::guild_icon_request(guild, icons.cdn, MUTUAL_ICON, icons.scale)
								.map(|request| self.image(request, icons));
						names.extend(icon.clone());
						mutual_value(guild.id, icon.as_deref(), &guild.name)
					})
					.collect();
				let friends: Vec<Value> = profile
					.mutual_friend_icons()
					.iter()
					.map(|friend| {
						let icon = friend
							.user
							.as_ref()
							.and_then(|user| {
								bridge::avatar_request(icons.cdn, user, MUTUAL_ICON, icons.scale, false)
							})
							.map(|request| self.image(request, icons));
						names.extend(icon.clone());
						mutual_value(friend.id, icon.as_deref(), &friend.name)
					})
					.collect();
				self.profile.set(profile_value(
					profile,
					[avatar.as_deref(), banner.as_deref(), tag.as_deref()],
					[servers, friends],
				));
				self.members.set(Value::List(Rc::new([])));
			}
			DmPanel::Members(members) => {
				let rows: Vec<Value> = members
					.iter()
					.map(|member| {
						let avatar = member
							.avatar_request(icons.cdn, MEMBER_AVATAR, icons.scale)
							.map(|request| self.image(request, icons));
						names.extend(avatar.clone());
						member_value(member, avatar.as_deref())
					})
					.collect();
				self.profile.set(empty_profile());
				self.members.set(Value::List(rows.into()));
			}
			DmPanel::Empty => {
				self.profile.set(empty_profile());
				self.members.set(Value::List(Rc::new([])));
			}
		}
		let changed = *self.shown.borrow() != names;
		*self.shown.borrow_mut() = names;
		changed
	}

	fn image(&self, request: ImageRequest, icons: &Icons<'_>) -> String {
		let mut images = self.images.borrow_mut();
		let name = images
			.entry(request.url.clone())
			.or_insert_with(|| icons.gallery.request(request));
		name.as_str().to_owned()
	}
}

fn host_image(name: Option<&str>) -> String {
	name.map(|name| format!("host:{name}")).unwrap_or_default()
}

fn accent_text(accent: Option<u32>) -> String {
	accent
		.map(|color| format!("#{:06x}", color & 0x00ff_ffff))
		.unwrap_or_default()
}

fn empty_profile() -> Value {
	record([
		("open", false.into()),
		("id", "".into()),
		("name", "".into()),
		("username", "".into()),
		("tag", "".into()),
		("tag_badge", "".into()),
		("avatar", "".into()),
		("banner", "".into()),
		("accent", "".into()),
		("status", "offline".into()),
		("bio", "".into()),
		("since", "".into()),
		("servers", "".into()),
		("friends", "".into()),
		("server_icons", Value::List(Rc::new([]))),
		("friend_icons", Value::List(Rc::new([]))),
		("friend", "".into()),
		("loading", false.into()),
		("theme_top", "".into()),
		("theme_bottom", "".into()),
		("effect", false.into()),
	])
}

fn mutual_value(id: Snowflake, icon: Option<&str>, name: &str) -> Value {
	let initial: String = name.chars().next().map(String::from).unwrap_or_default();
	record([
		("key", format!("m{id}").as_str().into()),
		("src", host_image(icon).as_str().into()),
		("initial", initial.as_str().into()),
	])
}

fn friend_state(card: &ProfileCard) -> &'static str {
	match card.relation {
		_ if card.mine || card.bot => "",
		Relation::None => "add",
		Relation::Incoming => "accept",
		Relation::Outgoing => "sent",
		Relation::Friend | Relation::Blocked => "",
	}
}

fn profile_value(
	profile: &DmProfile,
	[avatar, banner, tag_badge]: [Option<&str>; 3],
	[servers, friends]: [Vec<Value>; 2],
) -> Value {
	let card = &profile.card;
	let (top, bottom) = card
		.theme
		.map_or((None, None), |theme| (Some(theme.primary), Some(theme.accent)));
	record([
		("open", true.into()),
		("id", card.id.to_string().as_str().into()),
		("name", card.name.as_str().into()),
		("username", card.handle.as_str().into()),
		(
			"tag",
			card.tag.as_ref().map_or("", |tag| tag.text.as_str()).into(),
		),
		("tag_badge", host_image(tag_badge).as_str().into()),
		("avatar", host_image(avatar).as_str().into()),
		("banner", host_image(banner).as_str().into()),
		("accent", accent_text(card.accent).as_str().into()),
		("status", card.status.as_str().into()),
		("bio", card.bio.as_deref().unwrap_or_default().into()),
		("since", profile.since.as_str().into()),
		(
			"servers",
			profile.mutual_servers_text().unwrap_or_default().as_str().into(),
		),
		(
			"friends",
			profile.mutual_friends_text().unwrap_or_default().as_str().into(),
		),
		("server_icons", Value::List(servers.into())),
		("friend_icons", Value::List(friends.into())),
		("friend", friend_state(card).into()),
		("loading", profile.loading.into()),
		("theme_top", accent_text(top).as_str().into()),
		("theme_bottom", accent_text(bottom).as_str().into()),
		("effect", card.effect.into()),
	])
}

fn member_value(member: &GroupMember, avatar: Option<&str>) -> Value {
	record([
		("key", format!("g{}", member.id).as_str().into()),
		("id", member.id.to_string().as_str().into()),
		("name", member.name.as_str().into()),
		("avatar", host_image(avatar).as_str().into()),
		("status", member.status.as_str().into()),
		("activity", member.activity.as_deref().unwrap_or_default().into()),
		("owner", member.owner.into()),
	])
}
