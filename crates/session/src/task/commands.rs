use protocol::fingerprint::{ContextProperties, RelationshipLocation};
use protocol::{Permissions, Snowflake, StatusKind};
use std::time::Duration;

use rest::RestClient;
use rest::api::{MentionsQuery, ReactionRef, ThreadList, ThreadSearch, ThreadSearchReply, image_data_uri};
use serde_json::{Value, json};
use state::Loaded;

use super::actions::{mute_config, object, some};
use super::outbox::ForumPost;
use super::{Command, Core};
use crate::FriendSource;

const PNG: &str = "image/png";
const DEFAULT_ARCHIVE_MINUTES: i64 = 4320;
const REACTION_PAGE: u8 = 100;
const INDEX_TRIES: usize = 5;
const INDEX_WAIT_SECONDS: f64 = 2.;
const INDEX_WAIT_MIN: f64 = 0.3;
const INDEX_WAIT_MAX: f64 = 15.;

fn sessions_or_empty(result: rest::Result<Value>) -> Value {
	result.unwrap_or_else(|error| {
		tracing::warn!(%error, "device sessions were not loaded");
		Value::Null
	})
}

fn friend_context(from: FriendSource) -> Option<ContextProperties> {
	match from {
		FriendSource::AddFriend => Some(ContextProperties::relationship(RelationshipLocation::AddFriend)),
		FriendSource::Profile => None,
	}
}

fn png(bytes: &[u8]) -> Value {
	Value::String(image_data_uri(PNG, bytes))
}

async fn search_threads(
	rest: &RestClient,
	channel_id: Snowflake,
	query: ThreadSearch,
) -> rest::Result<ThreadList> {
	for _ in 0..INDEX_TRIES {
		match rest.search_threads(channel_id, query).await? {
			ThreadSearchReply::Found(list) => return Ok(list),
			ThreadSearchReply::Indexing { retry_after } => {
				let wait = retry_after
					.unwrap_or(INDEX_WAIT_SECONDS)
					.clamp(INDEX_WAIT_MIN, INDEX_WAIT_MAX);
				tokio::time::sleep(Duration::from_secs_f64(wait)).await;
			}
		}
	}
	tracing::warn!("the thread index was still not ready");
	Ok(ThreadList::default())
}

impl Core {
	pub(super) fn on_extra(&mut self, command: Command) {
		match command {
			command @ (Command::CreateThread { .. }
			| Command::JoinThread { .. }
			| Command::LeaveThread { .. }
			| Command::ArchiveThread { .. }
			| Command::LoadArchivedThreads { .. }
			| Command::LoadForumPosts { .. }
			| Command::CreateForumPost { .. }
			| Command::LoadPins { .. }
			| Command::LoadRecentMentions { .. }
			| Command::RemoveRecentMention { .. }
			| Command::PinMessage { .. }
			| Command::UnpinMessage { .. }
			| Command::LoadProfile { .. }
			| Command::LoadReactionUsers { .. }
			| Command::VotePoll { .. }
			| Command::EndPoll { .. }) => self.on_text_command(command),
			command @ (Command::CreateInvite { .. }
			| Command::ResolveInvite { .. }
			| Command::AcceptInvite { .. }
			| Command::LoadOnboarding { .. }
			| Command::SubmitOnboarding { .. }
			| Command::LoadMemberVerification { .. }
			| Command::AcceptMemberVerification { .. }
			| Command::ForwardMessage { .. }) => self.on_community_command(command),
			command @ (Command::SendSticker { .. }
			| Command::LoadGifTrending
			| Command::LoadTrendingGifs
			| Command::SearchGifs { .. }
			| Command::SuggestGifs { .. }
			| Command::SelectGif { .. }
			| Command::SendGif { .. }
			| Command::LoadStickerPacks
			| Command::LoadTopEmojis { .. }
			| Command::LoadGuildStickers { .. }
			| Command::CreateSticker { .. }
			| Command::CreateEmoji { .. }
			| Command::EditSticker { .. }
			| Command::DeleteSticker { .. }
			| Command::EditEmoji { .. }) => self.on_expression_command(command),
			Command::RankEmoji {
				guild_id,
				query,
				candidates,
			} => self.rank_emoji(guild_id, query, candidates),
			Command::LoadFavorites => self.want_frecency(),
			Command::SetGifFavorite { gif, favorite } => self.set_gif_favorite(gif, favorite),
			Command::SetEmojiFavorite { entry, favorite } => self.set_emoji_favorite(entry, favorite),
			Command::CountCommandUse { key } => self.count_command_use(key),
			Command::SendPoll { channel_id, poll } => self.send_poll(channel_id, poll),
			Command::SuppressEmbeds {
				channel_id,
				message_id,
			} => self.suppress_embeds(channel_id, message_id),
			command @ (Command::AddFriend { .. }
			| Command::AcceptFriend { .. }
			| Command::RemoveRelationship { .. }
			| Command::BlockUser { .. }
			| Command::IgnoreUser { .. }
			| Command::UnignoreUser { .. }
			| Command::LeaveGuild { .. }
			| Command::CreateGroupDm { .. }
			| Command::AddGroupRecipient { .. }
			| Command::RemoveGroupRecipient { .. }
			| Command::RenameGroup { .. }
			| Command::CloseDm { .. }
			| Command::MuteChannel { .. }
			| Command::UnmuteChannel { .. }
			| Command::MuteGuild { .. }
			| Command::UnmuteGuild { .. }
			| Command::SetChannelNotifications { .. }
			| Command::SetGuildNotifications { .. }
			| Command::MarkGuildRead { .. }
			| Command::MarkChannelRead { .. }
			| Command::MarkAllRead
			| Command::MarkUnread { .. }
			| Command::UpdateUserSettings(_)
			| Command::SetGuildLayout(_)
			| Command::CreateGuild { .. }
			| Command::LoadSessions
			| Command::LogoutSessions(_)
			| Command::UpdateGuildSettingsRaw { .. }
			| Command::SetCustomStatus { .. }
			| Command::SetStatus(_)
			| Command::SetActivitySharing(_)
			| Command::EditProfile { .. }
			| Command::SetGuildProfile { .. }) => self.on_social_command(command),
			command => self.on_admin_command(command),
		}
	}

	fn on_text_command(&mut self, command: Command) {
		match command {
			Command::CreateThread {
				channel_id,
				message_id,
				name,
				auto_archive_minutes,
				private,
			} => {
				let private = private && message_id.is_none();
				let (needed, label) = if private {
					(Permissions::CREATE_PRIVATE_THREADS, "Create Private Threads")
				} else {
					(Permissions::CREATE_PUBLIC_THREADS, "Create Public Threads")
				};
				if self.channel_allows(channel_id, needed, label) {
					self.spawn_open("create thread", move |rest| async move {
						rest.start_thread(channel_id, message_id, &name, auto_archive_minutes, private)
							.await
					});
				}
			}
			Command::JoinThread { thread_id } => self.spawn_action("join thread", move |rest| async move {
				rest.join_thread(thread_id).await
			}),
			Command::LeaveThread { thread_id } => self.spawn_action("leave thread", move |rest| async move {
				rest.leave_thread(thread_id).await
			}),
			Command::ArchiveThread { thread_id, archived } => {
				self.spawn_action("archive thread", move |rest| async move {
					rest.modify_channel(thread_id, json!({ "archived": archived }))
						.await
						.map(drop)
				});
			}
			Command::LoadArchivedThreads { channel_id, more } => {
				let offset = {
					let state = self.state.read();
					if more && !state.archived_threads_has_more(channel_id) {
						return;
					}
					if more {
						state.archived_threads(channel_id).len() as u32
					} else {
						0
					}
				};
				self.spawn_load("archived threads", move |rest| async move {
					let query = ThreadSearch {
						archived: Some(true),
						offset,
					};
					let list = search_threads(&rest, channel_id, query).await?;
					Ok(Loaded::ArchivedThreads {
						channel_id,
						threads: list.threads.into_vec(),
						members: list.members.into_vec(),
						has_more: list.has_more,
						append: offset > 0,
					})
				});
			}
			Command::LoadForumPosts { channel_id, more } => {
				let offset = {
					let state = self.state.read();
					match state.forum_page(channel_id) {
						Some((_, false)) if more => return,
						Some((loaded, _)) if more => loaded as u32,
						_ => 0,
					}
				};
				self.spawn_load("forum posts", move |rest| async move {
					let query = ThreadSearch {
						archived: None,
						offset,
					};
					let list = search_threads(&rest, channel_id, query).await?;
					Ok(Loaded::ForumPosts {
						channel_id,
						threads: list.threads.into_vec(),
						members: list.members.into_vec(),
						starters: list.first_messages.into_vec(),
						has_more: list.has_more,
						append: offset > 0,
					})
				});
			}
			Command::CreateForumPost {
				channel_id,
				name,
				content,
				tags,
				files,
			} => {
				if self.channel_allows(channel_id, Permissions::SEND_MESSAGES, "Send Messages") {
					let auto_archive_minutes = self
						.state
						.read()
						.channel(channel_id)
						.and_then(|forum| forum.default_auto_archive_duration)
						.unwrap_or(DEFAULT_ARCHIVE_MINUTES);
					self.post_forum(ForumPost {
						channel_id,
						name,
						content,
						tags,
						files,
						auto_archive_minutes,
					});
				}
			}
			Command::LoadPins { channel_id } => self.load_pins(channel_id),
			Command::LoadRecentMentions {
				everyone,
				roles,
				guild_id,
			} => self.load_recent_mentions(MentionsQuery {
				everyone,
				roles,
				guild_id,
				..MentionsQuery::default()
			}),
			Command::RemoveRecentMention { message_id } => self.remove_recent_mention(message_id),
			Command::PinMessage {
				channel_id,
				message_id,
			} => {
				if self.channel_allows(channel_id, Permissions::PIN_MESSAGES, "Pin Messages") {
					self.spawn_action("pin message", move |rest| async move {
						rest.pin_message(channel_id, message_id).await
					});
				}
			}
			Command::UnpinMessage {
				channel_id,
				message_id,
			} => {
				if self.channel_allows(channel_id, Permissions::PIN_MESSAGES, "Pin Messages") {
					self.spawn_action("unpin message", move |rest| async move {
						rest.unpin_message(channel_id, message_id).await
					});
				}
			}
			Command::LoadProfile { user_id, guild_id } => self.load_profile(user_id, guild_id),
			Command::LoadReactionUsers {
				channel_id,
				message_id,
				emoji,
				burst,
				after,
			} => self.spawn_load("reactions", move |rest| async move {
				let reaction = ReactionRef {
					channel_id,
					message_id,
					emoji: &emoji,
					burst,
				};
				let users = rest.reactions(reaction, after, REACTION_PAGE).await?;
				Ok(Loaded::ReactionUsers {
					message_id,
					emoji,
					burst,
					users: users.into_vec(),
					append: after.is_some(),
					limit: usize::from(REACTION_PAGE),
				})
			}),
			Command::VotePoll {
				channel_id,
				message_id,
				answer_ids,
			} => self.spawn_action("vote", move |rest| async move {
				rest.vote_poll(channel_id, message_id, &answer_ids).await
			}),
			Command::EndPoll {
				channel_id,
				message_id,
			} => self.spawn_action("end poll", move |rest| async move {
				rest.end_poll(channel_id, message_id).await.map(drop)
			}),
			_ => {}
		}
	}

	pub(super) fn reload_pins(&self, channel_id: Snowflake) {
		if self.state.read().pins(channel_id).is_some() {
			self.load_pins(channel_id);
		}
	}

	pub(super) fn load_pins(&self, channel_id: Snowflake) {
		self.spawn_load("pins", move |rest| async move {
			let messages = rest.pins(channel_id).await?;
			Ok(Loaded::Pins { channel_id, messages })
		});
	}

	fn on_social_command(&mut self, command: Command) {
		match command {
			Command::AddFriend { username, from } => {
				let context = friend_context(from);
				self.spawn_action("friend request", move |rest| async move {
					rest.send_friend_request(&username, context.as_ref()).await
				});
			}
			Command::AcceptFriend { user_id } => self.spawn_action("accept friend", move |rest| async move {
				rest.add_relationship(user_id).await
			}),
			Command::RemoveRelationship { user_id } => {
				self.spawn_action("remove friend", move |rest| async move {
					rest.remove_relationship(user_id).await
				});
			}
			Command::BlockUser { user_id } => {
				self.spawn_action(
					"block user",
					move |rest| async move { rest.block_user(user_id).await },
				)
			}
			Command::IgnoreUser { user_id } => self.spawn_action("ignore user", move |rest| async move {
				rest.ignore_user(user_id).await
			}),
			Command::UnignoreUser { user_id } => self.spawn_action("unignore user", move |rest| async move {
				rest.unignore_user(user_id).await
			}),
			Command::LeaveGuild { guild_id } => self.spawn_action("leave server", move |rest| async move {
				rest.leave_guild(guild_id).await
			}),
			Command::CreateGroupDm { recipient_ids } => {
				self.spawn_open("create group", move |rest| async move {
					rest.create_group_dm(&recipient_ids).await
				});
			}
			Command::AddGroupRecipient { channel_id, user_id } => {
				self.spawn_action("add to group", move |rest| async move {
					rest.add_group_recipient(channel_id, user_id).await
				});
			}
			Command::RemoveGroupRecipient { channel_id, user_id } => {
				self.spawn_action("remove from group", move |rest| async move {
					rest.remove_group_recipient(channel_id, user_id).await
				});
			}
			Command::RenameGroup { channel_id, name } => {
				self.spawn_action("rename group", move |rest| async move {
					rest.modify_channel(channel_id, json!({ "name": name }))
						.await
						.map(drop)
				});
			}
			Command::CloseDm { channel_id } => self
				.spawn_action("close conversation", move |rest| async move {
					rest.delete_channel(channel_id).await.map(drop)
				}),
			Command::MuteChannel {
				channel_id,
				guild_id,
				until,
			} => self.patch_channel_override(
				channel_id,
				guild_id,
				json!({ "muted": true, "mute_config": mute_config(true, until) }),
			),
			Command::UnmuteChannel { channel_id, guild_id } => {
				self.patch_channel_override(
					channel_id,
					guild_id,
					json!({ "muted": false, "mute_config": null }),
				);
			}
			Command::MuteGuild { guild_id, until } => self.patch_guild_mute(Some(guild_id), true, until),
			Command::UnmuteGuild { guild_id } => self.patch_guild_mute(Some(guild_id), false, None),
			Command::SetChannelNotifications { channel_id, level } => {
				let guild_id = self.state.read().channel_guild(channel_id);
				self.patch_channel_override(
					channel_id,
					guild_id,
					json!({ "message_notifications": i64::from(level) }),
				);
			}
			Command::SetGuildNotifications {
				guild_id,
				level,
				suppress_everyone,
				suppress_roles,
				mobile_push,
			} => {
				let body = json!({
					"message_notifications": i64::from(level),
					"suppress_everyone": suppress_everyone,
					"suppress_roles": suppress_roles,
					"mobile_push": mobile_push,
				});
				self.spawn_action("notification settings", move |rest| async move {
					rest.patch_guild_settings(guild_id, body).await.map(drop)
				});
			}
			Command::MarkGuildRead { guild_id } => self.mark_guild_read(guild_id),
			Command::MarkChannelRead { channel_id } => self.mark_channel_read(channel_id),
			Command::MarkAllRead => self.mark_all_read(),
			Command::MarkUnread {
				channel_id,
				message_id,
			} => self.mark_unread(channel_id, message_id),
			Command::UpdateUserSettings(changes) => self
				.spawn_load("user settings", move |rest| async move {
					Ok(Loaded::UserSettings(rest.patch_user_settings(changes).await?))
				}),
			Command::SetGuildLayout(layout) => self.set_guild_layout(&layout),
			Command::CreateGuild { name, icon } => {
				self.select_created_guild = true;
				self.spawn_action("create server", move |rest| async move {
					rest.create_guild(&name, icon.as_deref()).await.map(drop)
				});
			}
			Command::LoadSessions => self.spawn_load("devices", |rest| async move {
				Ok(Loaded::AuthSessions(sessions_or_empty(
					rest.auth_sessions().await,
				)))
			}),
			Command::LogoutSessions(id_hashes) => self.spawn_load("log out devices", |rest| async move {
				rest.logout_sessions(&id_hashes).await?;
				Ok(Loaded::AuthSessions(sessions_or_empty(
					rest.auth_sessions().await,
				)))
			}),
			Command::UpdateGuildSettingsRaw { guild_id, changes } => {
				self.spawn_action("notification settings", move |rest| async move {
					rest.patch_guild_settings(guild_id, changes).await.map(drop)
				});
			}
			Command::SetCustomStatus { text, emoji, expires } => self.set_custom_status(text, emoji, expires),
			Command::SetStatus(status) => self.set_status(status),
			Command::SetActivitySharing(shared) => self.set_activity_sharing(shared),
			Command::EditProfile {
				global_name,
				bio,
				avatar_png,
				banner_png,
				pronouns,
			} => {
				let account = object([
					("global_name", some(global_name)),
					("avatar", avatar_png.as_deref().map(png)),
				]);
				let profile = object([
					("bio", some(bio)),
					("pronouns", some(pronouns)),
					("banner", banner_png.as_deref().map(png)),
				]);
				self.spawn_action("edit profile", move |rest| async move {
					if account.as_object().is_some_and(|fields| !fields.is_empty()) {
						rest.patch_current_user(account).await?;
					}
					if profile.as_object().is_some_and(|fields| !fields.is_empty()) {
						rest.patch_current_profile(profile).await?;
					}
					Ok(())
				});
			}
			Command::SetGuildProfile {
				guild_id,
				nick,
				avatar_png,
				bio,
			} => {
				let body = object([
					("nick", some(nick)),
					("avatar", avatar_png.as_deref().map(png)),
					("bio", some(bio)),
				]);
				self.spawn_action("server profile", move |rest| async move {
					rest.modify_current_member(guild_id, body).await.map(drop)
				});
			}
			_ => {}
		}
	}

	pub(super) fn set_afk(&mut self, afk: bool) {
		if self.afk == afk {
			return;
		}
		self.afk = afk;
		self.publish_presence();
	}

	pub(super) fn set_presence_command(&mut self, status: StatusKind, custom_status: Option<String>) {
		self.set_status(status);
		self.set_custom_status(custom_status, None, None);
	}
}
