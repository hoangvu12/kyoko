use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bridge::MemberGate;
use chrono::Utc;
use protocol::{NotificationLevel, Snowflake};
use session::Command;
use state::Changes;
use zgx_script::{ScriptHost, Value};

use super::{Link, field, number, snowflake, text};
use crate::community::{Dialog, Request};

const PREVIEW_LENGTH: usize = 160;

#[derive(Default)]
pub(super) struct Joins {
	seen: Cell<u64>,
	arriving: Cell<Option<Snowflake>>,
	wanted: Cell<Option<Snowflake>>,
	offered: Cell<Option<(Snowflake, MemberGate)>>,
	requested: RefCell<Vec<(Snowflake, MemberGate)>>,
}

impl Joins {
	fn ask(&self, guild_id: Snowflake, gate: MemberGate) -> bool {
		let mut requested = self.requested.borrow_mut();
		if requested.contains(&(guild_id, gate)) {
			return false;
		}
		requested.push((guild_id, gate));
		true
	}
}

fn level_name(level: NotificationLevel) -> &'static str {
	match level {
		NotificationLevel::AllMessages => "all",
		NotificationLevel::OnlyMentions => "mentions",
		NotificationLevel::NoMessages => "nothing",
		_ => "inherit",
	}
}

fn level_of(name: &str) -> Option<NotificationLevel> {
	match name {
		"all" => Some(NotificationLevel::AllMessages),
		"mentions" => Some(NotificationLevel::OnlyMentions),
		"nothing" => Some(NotificationLevel::NoMessages),
		"inherit" => Some(NotificationLevel::Inherit),
		_ => None,
	}
}

fn minutes(argument: &Value) -> Option<i64> {
	number(argument, "minutes")
		.filter(|minutes| *minutes > 0.)
		.map(|minutes| minutes as i64)
}

fn unmute(argument: &Value) -> bool {
	matches!(field(argument, "unmute"), Some(Value::Bool(true)))
}

impl Link {
	pub(super) fn install_community(host: &ScriptHost, link: &Rc<Link>) {
		let hooks = Rc::downgrade(link);
		link.community.on_request(move |request| {
			if let Some(link) = hooks.upgrade() {
				match request {
					Request::Command(command) => link.send(command),
					Request::Copy(text) => {
						if let Err(error) = platform::clipboard::write_text(&text) {
							tracing::warn!(%error, "the invite link was not copied");
						}
					}
				}
			}
		});
		let hooks = Rc::downgrade(link);
		link.community.on_images_changed(move |()| {
			if let Some(link) = hooks.upgrade() {
				link.refresh_visible();
			}
		});
		let hooks = Rc::downgrade(link);
		link.timeline_view.on_forward(move |(channel_id, message_id)| {
			if let Some(link) = hooks.upgrade() {
				link.open_forward(channel_id, message_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("rail.add_server", move |_| {
			if let Some(link) = actions.upgrade() {
				link.community.open(Dialog::AddServer);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.invite", move |argument| {
			if let Some(link) = actions.upgrade() {
				link.open_invite(snowflake(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.notification_settings", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(guild_id) = link.sidebar.sidebar().selected()
			{
				link.community.open(Dialog::Notifications(guild_id));
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.leave_guild", move |_| {
			if let Some(link) = actions.upgrade() {
				link.open_leave();
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.mute_guild", move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(guild_id) = link.sidebar.sidebar().selected()
			{
				let command = if unmute(&argument) {
					Command::UnmuteGuild { guild_id }
				} else {
					Command::MuteGuild {
						guild_id,
						until: bridge::mute_until(minutes(&argument), Utc::now()),
					}
				};
				link.send(command);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.mute_channel", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let Some(channel_id) = field(&argument, "id").and_then(snowflake) else {
				return;
			};
			let guild_id = link.sidebar.sidebar().selected();
			let command = if unmute(&argument) {
				Command::UnmuteChannel { channel_id, guild_id }
			} else {
				Command::MuteChannel {
					channel_id,
					guild_id,
					until: bridge::mute_until(minutes(&argument), Utc::now()),
				}
			};
			link.send(command);
		});
		let actions = Rc::downgrade(link);
		host.action("sidebar.channel_notifications", move |argument| {
			let Some(link) = actions.upgrade() else {
				return;
			};
			let channel_id = field(&argument, "id").and_then(snowflake);
			let level = field(&argument, "level").and_then(text).and_then(level_of);
			if let (Some(channel_id), Some(level)) = (channel_id, level) {
				link.send(Command::SetChannelNotifications { channel_id, level });
			}
		});
		let actions = Rc::downgrade(link);
		host.function("sidebar.channel_level", move |argument| {
			let level = actions
				.upgrade()
				.zip(snowflake(&argument))
				.and_then(|(link, channel_id)| {
					let shared = link.state()?;
					let state = shared.read();
					Some(bridge::channel_notifications(&state, channel_id))
				})
				.unwrap_or(NotificationLevel::Inherit);
			Value::from(level_name(level))
		});
	}

	fn open_forward(&self, channel_id: Snowflake, message_id: Snowflake) {
		let preview = self
			.timeline
			.borrow()
			.messages()
			.find(|entry| entry.id == message_id)
			.map(|entry| bridge::snippet(&entry.body.content, &entry.names, PREVIEW_LENGTH))
			.unwrap_or_default();
		self.community.open(Dialog::Forward {
			channel_id,
			message_id,
			preview,
		});
	}

	fn open_invite(&self, channel_id: Option<Snowflake>) {
		let Some(shared) = self.state() else {
			return;
		};
		let channel_id = channel_id.or_else(|| {
			let guild_id = self.sidebar.sidebar().selected()?;
			bridge::invite_channel(&shared.read(), guild_id)
		});
		if let Some(channel_id) = channel_id {
			self.community.open(Dialog::Invite(channel_id));
		}
	}

	fn open_leave(&self) {
		let Some(guild_id) = self.sidebar.sidebar().selected() else {
			return;
		};
		let name = self.sidebar.sidebar().header().name.clone();
		self.community.open(Dialog::Leave { guild_id, name });
	}

	pub(super) fn community_images(&self) -> Vec<String> {
		self.community.images().iter().map(ToString::to_string).collect()
	}

	pub(super) fn offer_gate(&self, guild_id: Snowflake) {
		self.joins.wanted.set(Some(guild_id));
		self.joins.offered.set(None);
		self.follow_gate();
	}

	pub(super) fn community_apply(&self, changes: &Changes) {
		if !changes.loaded.is_empty() || changes.guild_list || !changes.members.is_empty() {
			self.community.refresh();
		}
		self.follow_join();
		if self.joins.wanted.get().is_some() {
			self.follow_gate();
		}
	}

	fn follow_join(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		let arrived = {
			let state = shared.read();
			if let Some(join) = state.last_join()
				&& join.serial > self.joins.seen.get()
			{
				self.joins.seen.set(join.serial);
				self.joins.arriving.set(join.guild_id);
				if let Some(guild_id) = join.guild_id {
					if join.verification {
						self.joins.ask(guild_id, MemberGate::Screening);
					}
					if join.onboarding {
						self.joins.ask(guild_id, MemberGate::Onboarding);
					}
				}
			}
			self.joins
				.arriving
				.get()
				.filter(|guild_id| state.guild(*guild_id).is_some())
		};
		if let Some(guild_id) = arrived {
			self.joins.arriving.set(None);
			self.select_guild(guild_id);
		}
	}

	fn follow_gate(&self) {
		let Some(guild_id) = self.joins.wanted.get() else {
			return;
		};
		let Some(shared) = self.state() else {
			return;
		};
		let state = shared.read();
		if state.guild(guild_id).is_none() || self.sidebar.sidebar().selected() != Some(guild_id) {
			self.joins.wanted.set(None);
			return;
		}
		let Some(gate) = bridge::guild_gate(&state, guild_id, Utc::now()) else {
			self.joins.wanted.set(None);
			return;
		};
		if self.joins.offered.get() == Some((guild_id, gate)) {
			return;
		}
		let ready = match gate {
			MemberGate::Screening => state.member_verification(guild_id).is_some(),
			MemberGate::Onboarding => bridge::onboarding_ready(&state, guild_id),
		};
		if !ready {
			if self.joins.ask(guild_id, gate) {
				self.send(match gate {
					MemberGate::Screening => Command::LoadMemberVerification {
						guild_id,
						invite_code: None,
					},
					MemberGate::Onboarding => Command::LoadOnboarding { guild_id },
				});
			}
			return;
		}
		if self.community.is_open() {
			return;
		}
		drop(state);
		self.joins.offered.set(Some((guild_id, gate)));
		self.community.open(match gate {
			MemberGate::Screening => Dialog::Screening(guild_id),
			MemberGate::Onboarding => Dialog::Onboarding(guild_id),
		});
	}

	pub(super) fn drive_add_server(&self) {
		self.community.open(Dialog::AddServer);
	}

	pub(super) fn drive_forward(&self) {
		let target = self.timeline.borrow().open().map(|open| open.channel_id).zip(
			self.timeline
				.borrow()
				.messages()
				.rev()
				.find(|entry| !entry.pending && entry.system.is_none())
				.map(|entry| entry.id),
		);
		match target {
			Some((channel_id, message_id)) => self.open_forward(channel_id, message_id),
			None => tracing::warn!("no message to forward"),
		}
	}

	pub(super) fn drive_guild_notifications(&self) {
		match self.sidebar.sidebar().selected() {
			Some(guild_id) => self.community.open(Dialog::Notifications(guild_id)),
			None => tracing::warn!("no server is open"),
		}
	}

	pub(super) fn community_settled(&self) -> bool {
		self.community.settled()
	}
}
