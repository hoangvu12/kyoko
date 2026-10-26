use std::rc::Rc;

use bridge::{InboxTab, today};
use media::ImageName;
use protocol::Snowflake;
use session::Command;
use state::{Changes, State};
use zgx_script::ScriptHost;

use super::{Link, number};
use crate::capture;
use crate::inbox::{Filter, InboxShown, Request};

impl Link {
	pub(super) fn install_inbox(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.action("inbox.open", move |argument| {
			if let Some(link) = actions.upgrade() {
				let at = (
					number(&argument, "x").unwrap_or_default(),
					number(&argument, "y").unwrap_or_default(),
				);
				link.toggle_inbox(at);
			}
		});
		let actions = Rc::downgrade(link);
		link.inbox_popout.on_request(move |request| {
			if let Some(link) = actions.upgrade() {
				link.inbox_request(request);
			}
		});
		let actions = Rc::downgrade(link);
		link.inbox_popout.on_images_changed(move |()| {
			if let Some(link) = actions.upgrade() {
				link.refresh_visible();
			}
		});
	}

	fn size_inbox(&self) {
		let (width, height) = self.window.info.bounds.with_untracked(|bounds| bounds.size);
		let scale = self.scale_factor() as f32;
		if width > 0 && height > 0 && scale > 0. {
			self.inbox_popout
				.set_window(width as f32 / scale, height as f32 / scale);
		}
	}

	pub(super) fn toggle_inbox(&self, at: (f32, f32)) {
		self.size_inbox();
		self.inbox_popout.toggle(at);
		self.refresh_inbox();
	}

	fn inbox_request(&self, request: Request) {
		match request {
			Request::Load(InboxTab::Mentions, filter) => self.load_mentions(filter),
			Request::Load(..) => self.refresh_inbox(),
			Request::JumpTo {
				guild_id,
				channel_id,
				message_id,
			} => {
				self.show_channel(guild_id, channel_id);
				match self.view_showing(channel_id) {
					Some(view) => view.jump_to(message_id),
					None => tracing::debug!("the channel of the mention cannot be opened"),
				}
			}
			Request::OpenChannel { guild_id, channel_id } => self.show_channel(guild_id, channel_id),
			Request::Remove(message_ids) => {
				for message_id in message_ids {
					self.send(Command::RemoveRecentMention { message_id });
				}
			}
			Request::MarkRead(channel_id) => self.send(Command::MarkChannelRead { channel_id }),
			Request::MarkGuildRead(guild_id) => self.send(Command::MarkGuildRead { guild_id }),
			Request::Accept(user_id) => self.send(Command::AcceptFriend { user_id }),
			Request::Ignore(user_id) => self.send(Command::RemoveRelationship { user_id }),
		}
	}

	fn load_mentions(&self, filter: Filter) {
		let guild_id = filter
			.this_server
			.then(|| self.sidebar.sidebar().selected())
			.flatten();
		self.send(Command::LoadRecentMentions {
			everyone: filter.everyone,
			roles: filter.roles,
			guild_id,
		});
		self.refresh_inbox();
	}

	fn show_channel(&self, guild_id: Option<Snowflake>, channel_id: Snowflake) {
		if self.view_showing(channel_id).is_some() {
			return;
		}
		match guild_id {
			Some(guild_id) => self.open(Some(guild_id), channel_id),
			None => self.select_dm(channel_id),
		}
		if let Some(shared) = self.state() {
			self.publish_actions(&shared.read(), &Changes::default());
		}
	}

	pub(super) fn inbox_escape(&self) -> bool {
		self.inbox_popout.escape()
	}

	pub(super) fn inbox_images(&self) -> Vec<String> {
		self.inbox_popout
			.images()
			.iter()
			.map(ToString::to_string)
			.collect()
	}

	pub(super) fn inbox_apply(&self, state: &State, changes: &Changes) {
		if !self.inbox_popout.is_open() {
			return;
		}
		let relevant = changes.recent_mentions
			|| changes.read_state
			|| changes.relationships
			|| changes.guild_list
			|| changes.current_user
			|| !changes.channel_lists.is_empty()
			|| !changes.members.is_empty();
		if relevant {
			self.publish_inbox(state);
		}
	}

	fn refresh_inbox(&self) {
		if let Some(shared) = self.state() {
			self.publish_inbox(&shared.read());
		}
	}

	fn publish_inbox(&self, state: &State) {
		if !self.inbox_popout.is_open() {
			return;
		}
		let today = today();
		let mut shown = InboxShown::default();
		match self.inbox_popout.tab() {
			InboxTab::Mentions => {
				shown.mentions = bridge::mention_groups(state, today);
				for message in state.recent_mentions().unwrap_or_default() {
					self.add_avatar(&mut shown.avatars, &message.author);
				}
				let places = shown
					.mentions
					.iter()
					.flatten()
					.map(|group| group.place.guild_id)
					.collect::<Vec<_>>();
				shown.icons = self.guild_icons(state, places);
			}
			InboxTab::Unreads => {
				shown.unreads = bridge::unread_groups(state);
				let guilds = shown.unreads.iter().map(|group| Some(group.guild_id)).collect();
				shown.icons = self.guild_icons(state, guilds);
			}
			InboxTab::ForYou => {
				shown.for_you = bridge::for_you(state, today);
				for entry in &shown.for_you {
					if let Some(user) = state.user(entry.user_id) {
						self.add_avatar(&mut shown.avatars, user);
					}
				}
			}
		}
		self.inbox_popout.show(shown);
	}

	fn add_avatar(&self, avatars: &mut Vec<(Snowflake, ImageName)>, user: &protocol::User) {
		if avatars.iter().any(|(known, _)| *known == user.id) {
			return;
		}
		if let Some(name) = self.inbox_popout.card_avatar(user) {
			avatars.push((user.id, name));
		}
	}

	fn guild_icons(&self, state: &State, guilds: Vec<Option<Snowflake>>) -> Vec<(Snowflake, ImageName)> {
		let mut icons: Vec<(Snowflake, ImageName)> = Vec::new();
		for guild_id in guilds.into_iter().flatten() {
			if icons.iter().any(|(known, _)| *known == guild_id) {
				continue;
			}
			if let Some(hash) = state.guild(guild_id).and_then(|guild| guild.icon.clone()) {
				icons.push((guild_id, self.inbox_popout.guild_icon(guild_id, &hash)));
			}
		}
		icons
	}

	pub(super) async fn drive_inbox(&self) {
		self.toggle_inbox(capture::TOP_BAR_INBOX);
		self.until_capture(|link| link.inbox_settled()).await;
	}

	fn inbox_settled(&self) -> bool {
		self.inbox_popout.settled()
			&& self
				.state()
				.is_some_and(|shared| shared.read().recent_mentions().is_some())
	}
}
