use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use bridge::{DmPanel, DmProfile, GroupMember};
use chrono::Utc;
use media::{ImageName, ImageRequest};
use protocol::Snowflake;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::record;

const PROFILE_AVATAR: f64 = 80.;
const MEMBER_AVATAR: f64 = 32.;

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
			profile: host.shared("dms.profile", profile_value(None, [None, None, None])),
			members: host.shared("dms.members", Value::List(Rc::new([]))),
		}
	}

	pub(crate) fn reset(&self) {
		self.open.set(None);
		*self.panel.borrow_mut() = DmPanel::Empty;
		self.images.borrow_mut().clear();
		self.shown.borrow_mut().clear();
		self.profile.set(profile_value(None, [None, None, None]));
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
				self.profile.set(profile_value(
					Some(profile),
					[avatar.as_deref(), banner.as_deref(), tag.as_deref()],
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
				self.profile.set(profile_value(None, [None, None, None]));
				self.members.set(Value::List(rows.into()));
			}
			DmPanel::Empty => {
				self.profile.set(profile_value(None, [None, None, None]));
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

fn profile_value(profile: Option<&DmProfile>, [avatar, banner, tag_badge]: [Option<&str>; 3]) -> Value {
	let Some(profile) = profile else {
		return record([
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
		]);
	};
	let card = &profile.card;
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
