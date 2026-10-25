use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bridge::{
	CategoryEntry, ChannelEntry, GuildHeader, Refresh, Sidebar, SidebarRow, VoiceMember, VoiceVideo,
};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx_script::{ScriptHost, Shared, Value};

use crate::rail::Icons;
use crate::ui::{Link, record};

const VOICE_AVATAR_POINTS: f64 = 24.;
pub(crate) struct SidebarView {
	sidebar: RefCell<Sidebar>,
	avatars: RefCell<HashMap<(Snowflake, Option<String>), ImageName>>,
	rows: Shared,
	guild_id: Shared,
	guild_name: Shared,
	header: Shared,
}

impl SidebarView {
	pub(crate) fn new(host: &ScriptHost) -> Self {
		Self {
			sidebar: RefCell::new(Sidebar::default()),
			avatars: RefCell::new(HashMap::new()),
			rows: host.shared("sidebar.rows", Value::List(Rc::new([]))),
			guild_id: host.shared("sidebar.guild_id", ""),
			guild_name: host.shared("sidebar.guild_name", ""),
			header: host.shared("sidebar.header", header_value(&GuildHeader::default())),
		}
	}

	pub(crate) fn reset(&self) {
		*self.sidebar.borrow_mut() = Sidebar::default();
		self.avatars.borrow_mut().clear();
		self.rows.set(Value::List(Rc::new([])));
		self.guild_id.set("".into());
		self.guild_name.set("".into());
		self.header.set(header_value(&GuildHeader::default()));
	}

	pub(crate) fn sidebar(&self) -> std::cell::Ref<'_, Sidebar> {
		self.sidebar.borrow()
	}

	pub(crate) fn apply(&self, state: &State, changes: &Changes) -> Refresh {
		self.sidebar.borrow_mut().apply(state, changes)
	}

	pub(crate) fn select(&self, state: &State, guild_id: Snowflake) -> Option<Command> {
		self.sidebar.borrow_mut().select(state, guild_id)
	}

	pub(crate) fn show_home(&self, state: &State) -> bool {
		self.sidebar.borrow_mut().show_home(state)
	}

	pub(crate) fn set_open(&self, channel_id: Option<Snowflake>) -> bool {
		self.sidebar.borrow_mut().set_open(channel_id)
	}

	pub(crate) fn publish(&self, state: &State, icons: &Icons<'_>) -> Vec<String> {
		let sidebar = self.sidebar.borrow();
		let mut names = Vec::new();
		let rows = sidebar
			.rows()
			.iter()
			.map(|row| match row {
				SidebarRow::Category(category) => category_value(category),
				SidebarRow::Channel(channel) => {
					let voice = channel
						.voice
						.iter()
						.map(|member| {
							let avatar = self.avatar(state, member.user_id, icons);
							names.extend(avatar.clone());
							voice_value(channel.id, member, avatar.as_deref())
						})
						.collect();
					channel_value(channel, Value::List(voice))
				}
			})
			.collect();
		self.rows.set(Value::List(rows));
		let selected = sidebar.selected();
		let id = selected.map(|id| id.to_string()).unwrap_or_default();
		self.guild_id.set(id.as_str().into());
		self.guild_name.set(sidebar.header().name.as_str().into());
		self.header.set(header_value(sidebar.header()));
		names
	}

	fn avatar(&self, state: &State, user_id: Snowflake, icons: &Icons<'_>) -> Option<String> {
		let user = state.user(user_id)?;
		let key = (user_id, user.avatar.clone());
		let mut avatars = self.avatars.borrow_mut();
		if let Some(name) = avatars.get(&key) {
			return Some(name.as_str().to_owned());
		}
		let request = bridge::avatar_request(icons.cdn, user, VOICE_AVATAR_POINTS, icons.scale, false)?;
		let name = icons.gallery.request(request);
		let text = name.as_str().to_owned();
		avatars.insert(key, name);
		Some(text)
	}
}

pub(crate) fn install_actions(host: &ScriptHost, link: &Rc<Link>) {
	let actions = Rc::downgrade(link);
	host.action("sidebar.select_channel", move |argument| {
		if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
			link.select_channel(channel_id);
		}
	});
	let actions = Rc::downgrade(link);
	host.action("sidebar.toggle_category", move |argument| {
		if let (Some(link), Some(category_id)) = (actions.upgrade(), snowflake(&argument)) {
			let command = link.sidebar().sidebar().toggle_command(category_id);
			if let Some(command) = command {
				link.send(command);
			}
		}
	});
	let actions = Rc::downgrade(link);
	host.action("sidebar.mark_channel_read", move |argument| {
		if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
			link.send(Command::MarkChannelRead { channel_id });
		}
	});
	let actions = Rc::downgrade(link);
	host.action("sidebar.mark_guild_read", move |_| {
		if let Some(link) = actions.upgrade() {
			let guild_id = link.sidebar().sidebar().selected();
			if let Some(guild_id) = guild_id {
				link.send(Command::MarkGuildRead { guild_id });
			}
		}
	});
	let actions = Rc::downgrade(link);
	host.action("sidebar.copy_channel_link", move |argument| {
		if let (Some(link), Some(channel_id)) = (actions.upgrade(), snowflake(&argument)) {
			let guild_id = link.sidebar().sidebar().selected();
			copy_link(guild_id, channel_id);
		}
	});
}

fn copy_link(guild_id: Option<Snowflake>, channel_id: Snowflake) {
	let scope = guild_id.map_or_else(|| "@me".to_owned(), |id| id.to_string());
	let link = format!("https://discord.com/channels/{scope}/{channel_id}");
	if let Err(error) = platform::clipboard::write_text(&link) {
		tracing::warn!(%error, "the channel link was not copied");
	}
}

fn snowflake(value: &Value) -> Option<Snowflake> {
	match value {
		Value::Str(text) => text.parse::<u64>().ok().map(Snowflake),
		_ => None,
	}
}

fn header_value(header: &GuildHeader) -> Value {
	record([
		("unread", header.unread.into()),
		("muted", header.muted.into()),
		("owner", header.owner.into()),
		("can_invite", header.can_invite.into()),
		("can_manage_channels", header.can_manage_channels.into()),
		("can_open_settings", header.can_open_settings.into()),
	])
}

fn category_value(category: &CategoryEntry) -> Value {
	record([
		("kind", "category".into()),
		("key", format!("c{}", category.id).as_str().into()),
		("id", category.id.to_string().as_str().into()),
		("name", category.name.as_str().into()),
		("collapsed", category.collapsed.into()),
		("muted", category.muted.into()),
	])
}

fn channel_value(channel: &ChannelEntry, voice: Value) -> Value {
	record([
		("kind", "channel".into()),
		("key", format!("h{}", channel.id).as_str().into()),
		("id", channel.id.to_string().as_str().into()),
		("type", channel.kind.as_str().into()),
		("icon", channel.icon.into()),
		("name", channel.name.as_str().into()),
		("unread", channel.unread.into()),
		("mentions", f64::from(channel.mentions).into()),
		("muted", channel.muted.into()),
		("locked", channel.locked.into()),
		("joined", channel.joined.into()),
		("selectable", channel.selectable().into()),
		("thread", channel.thread.into()),
		("last_thread", channel.last_thread.into()),
		("voice", voice),
	])
}

fn voice_value(channel_id: Snowflake, member: &VoiceMember, avatar: Option<&str>) -> Value {
	let video = match member.video {
		Some(VoiceVideo::Camera) => "camera",
		Some(VoiceVideo::Screen) => "screen",
		None => "",
	};
	let avatar = avatar.map(|name| format!("host:{name}")).unwrap_or_default();
	record([
		("key", format!("v{channel_id}-{}", member.user_id).as_str().into()),
		("id", member.user_id.to_string().as_str().into()),
		("name", member.name.as_str().into()),
		("avatar", avatar.as_str().into()),
		("muted", member.muted.into()),
		("deafened", member.deafened.into()),
		("video", video.into()),
		("live", member.live.into()),
	])
}
