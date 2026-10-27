use std::time::Duration;

use mockcord::Recording;
use protocol::{ChannelType, PermissionOverwrite, PermissionOverwriteKind, Permissions, Snowflake};
use serde_json::{Value, json};
use session::{ChannelChanges, Command, RestTuning};
use state::State;

use crate::harness::{Options, Running};

const GAP: Duration = Duration::from_millis(100);
const SPOILER: u64 = 1 << 21;

fn tuning() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		guild_write_gap: GAP,
		..RestTuning::default()
	}
}

struct Guild {
	id: Snowflake,
	me: Snowflake,
	text: Snowflake,
	voice: Snowflake,
	members_role: Snowflake,
}

async fn start(name: &'static str) -> (Running, Guild) {
	let running = Running::start_with(Options::new(name).rest(tuning())).await;
	running.connected().await;
	let guild_id = Snowflake(running.fixtures().guilds[0].id);
	running
		.until("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	let guild = {
		let state = running.session().state().read();
		let found = state.guild(guild_id).expect("the guild is loaded");
		let first = |kind: ChannelType| {
			let mut channels: Vec<(i64, Snowflake)> = found
				.channels
				.values()
				.filter(|channel| channel.kind == kind)
				.map(|channel| (channel.position.unwrap_or(0), channel.id))
				.collect();
			channels.sort();
			channels
				.first()
				.map(|(_, id)| *id)
				.expect("a channel of the kind")
		};
		Guild {
			id: guild_id,
			me: state.current_user_id().expect("signed in"),
			text: first(ChannelType::GuildText),
			voice: first(ChannelType::GuildVoice),
			members_role: found
				.roles
				.values()
				.find(|role| role.name == "Members")
				.map(|role| role.id)
				.expect("a Members role"),
		}
	};
	(running, guild)
}

fn patches(recording: &Recording, channel_id: Snowflake) -> Vec<Value> {
	recording
		.requests_to("PATCH", &format!("/channels/{channel_id}"))
		.filter_map(|request| request.json())
		.collect()
}

async fn demote(running: &Running, guild: &Guild) {
	let user = running.fixtures().current_user.clone();
	running.mock().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({
			"guild_id": guild.id.to_string(), "user": user, "roles": [guild.members_role.to_string()],
			"joined_at": "2025-01-01T00:00:00+00:00",
		}),
	);
	running
		.until("the current user without Manage Channels", |state: &State| {
			!state
				.guild_permissions(guild.id, guild.me)
				.contains(Permissions::MANAGE_CHANNELS)
		})
		.await;
}

async fn quiet() {
	tokio::time::sleep(GAP * 4).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_channel_edit_patches_only_the_changed_fields_within_discords_limits() {
	let (running, guild) = start("channel-settings-patch").await;
	running.session().send(Command::EditChannel {
		channel_id: guild.voice,
		changes: ChannelChanges {
			topic: Some("voice channels have no topic here".to_owned()),
			spoiler: Some(true),
			bitrate: Some(500_000),
			user_limit: Some(150),
			rtc_region: Some(Some("rotterdam".to_owned())),
			video_quality: Some(2),
			auto_archive: Some(1_440),
			..ChannelChanges::default()
		},
	});
	let voice = running
		.until_wire("the voice channel edit", |recording| {
			patches(recording, guild.voice).pop()
		})
		.await;
	assert_eq!(
		voice,
		json!({
			"flags": SPOILER,
			"bitrate": 128_000,
			"user_limit": 99,
			"rtc_region": "rotterdam",
			"video_quality_mode": 2,
		}),
		"a tier 1 server caps the bitrate at 128 kbps and a voice channel at 99 users"
	);
	running.session().send(Command::EditChannel {
		channel_id: guild.text,
		changes: ChannelChanges {
			name: Some("  news-and-notes  ".to_owned()),
			topic: Some("what changed".to_owned()),
			nsfw: Some(true),
			slowmode: Some(99_999),
			announcement: Some(true),
			auto_archive: Some(1_440),
			bitrate: Some(64_000),
			..ChannelChanges::default()
		},
	});
	let text = running
		.until_wire("the text channel edit", |recording| {
			patches(recording, guild.text).pop()
		})
		.await;
	assert_eq!(
		text,
		json!({
			"name": "news-and-notes",
			"topic": "what changed",
			"nsfw": true,
			"rate_limit_per_user": 21_600,
			"type": 5,
			"default_auto_archive_duration": 1_440,
		})
	);
	running
		.until("both edits in state", |state: &State| {
			let voice = state.channel(guild.voice);
			let text = state.channel(guild.text);
			voice.is_some_and(|voice| {
				voice.bitrate == Some(128_000)
					&& voice.rtc_region.as_deref() == Some("rotterdam")
					&& voice.flags.bits() & SPOILER != 0
			}) && text
				.is_some_and(|text| text.kind == ChannelType::GuildAnnouncement && text.nsfw == Some(true))
		})
		.await;
	running.session().send(Command::EditChannel {
		channel_id: guild.voice,
		changes: ChannelChanges {
			rtc_region: Some(None),
			..ChannelChanges::default()
		},
	});
	let automatic = running
		.until_wire("the region back to automatic", |recording| {
			let found = patches(recording, guild.voice);
			(found.len() >= 2).then(|| found[1].clone())
		})
		.await;
	assert_eq!(automatic, json!({ "rtc_region": null }));
	running.session().send(Command::EditChannel {
		channel_id: guild.text,
		changes: ChannelChanges::default(),
	});
	quiet().await;
	assert_eq!(
		patches(&running.mock().recording(), guild.text).len(),
		1,
		"an edit with nothing changed sends nothing"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manage_channels_is_checked_per_channel_before_any_request() {
	let (running, guild) = start("channel-settings-gating").await;
	demote(&running, &guild).await;
	let grant = {
		let state = running.session().state().read();
		let mut channel = state.channel(guild.voice).expect("the voice channel").clone();
		let mut overwrites = channel.permission_overwrites.into_vec();
		overwrites.push(PermissionOverwrite {
			id: guild.me,
			kind: PermissionOverwriteKind::Member,
			allow: Permissions::MANAGE_CHANNELS,
			deny: Permissions::empty(),
		});
		channel.permission_overwrites = overwrites.into();
		serde_json::to_value(&channel).expect("a channel serializes")
	};
	running.mock().dispatch("CHANNEL_UPDATE", &grant);
	running
		.until(
			"Manage Channels granted on the voice channel only",
			|state: &State| {
				state
					.my_permissions(guild.voice)
					.contains(Permissions::MANAGE_CHANNELS)
			},
		)
		.await;
	running.session().send(Command::EditChannel {
		channel_id: guild.text,
		changes: ChannelChanges {
			name: Some("refused".to_owned()),
			..ChannelChanges::default()
		},
	});
	running
		.until("the refusal", |state: &State| {
			state
				.last_rejection()
				.is_some_and(|rejection| rejection.reason.contains("Manage Channels"))
		})
		.await;
	running.session().send(Command::LoadChannelInvites {
		channel_id: guild.text,
	});
	running.session().send(Command::SetChannelOverwrite {
		channel_id: guild.voice,
		target_id: guild.members_role,
		kind: PermissionOverwriteKind::Role,
		allow: Permissions::SPEAK,
		deny: Permissions::empty(),
	});
	running.session().send(Command::EditChannel {
		channel_id: guild.voice,
		changes: ChannelChanges {
			user_limit: Some(5),
			..ChannelChanges::default()
		},
	});
	let allowed = running
		.until_wire("the edit Manage Channels allows", |recording| {
			patches(recording, guild.voice).pop()
		})
		.await;
	assert_eq!(allowed, json!({ "user_limit": 5 }));
	quiet().await;
	let recording = running.mock().recording();
	assert!(patches(&recording, guild.text).is_empty());
	assert_eq!(
		recording
			.requests_to("GET", &format!("/channels/{}/invites", guild.text))
			.count(),
		0
	);
	assert_eq!(
		recording
			.requests_to(
				"PUT",
				&format!("/channels/{}/permissions/{}", guild.voice, guild.members_role)
			)
			.count(),
		0,
		"Manage Permissions is a separate right"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overwrites_and_channel_invites_go_to_their_channel() {
	let (running, guild) = start("channel-settings-overwrites").await;
	running.session().send(Command::SetChannelOverwrite {
		channel_id: guild.text,
		target_id: guild.members_role,
		kind: PermissionOverwriteKind::Role,
		allow: Permissions::SEND_MESSAGES,
		deny: Permissions::ATTACH_FILES,
	});
	let path = format!("/channels/{}/permissions/{}", guild.text, guild.members_role);
	let body = running
		.until_wire("the overwrite", |recording| {
			recording
				.requests_to("PUT", &path)
				.filter_map(|request| request.json())
				.last()
		})
		.await;
	assert_eq!(
		body,
		json!({
			"type": 0,
			"allow": Permissions::SEND_MESSAGES.bits().to_string(),
			"deny": Permissions::ATTACH_FILES.bits().to_string(),
		})
	);
	running
		.until("the overwrite in state", |state: &State| {
			state.channel(guild.text).is_some_and(|channel| {
				channel.permission_overwrites.iter().any(|overwrite| {
					overwrite.id == guild.members_role && overwrite.deny == Permissions::ATTACH_FILES
				})
			})
		})
		.await;
	let invited = Snowflake(running.fixtures().guilds[0].message_channel_ids[0]);
	running
		.session()
		.send(Command::LoadChannelInvites { channel_id: invited });
	running
		.until("the channel's invites", |state: &State| {
			state
				.channel_invites(invited)
				.is_some_and(|invites| !invites.is_empty())
		})
		.await;
	let (code, count) = {
		let state = running.session().state().read();
		let invites = state.channel_invites(invited).unwrap_or_default();
		(invites[0].code.clone(), invites.len())
	};
	running.session().send(Command::RevokeChannelInvite {
		channel_id: invited,
		code: code.clone(),
	});
	running
		.until("the revoked invite gone", |state: &State| {
			state.channel_invites(invited).is_some_and(|invites| {
				invites.len() + 1 == count && invites.iter().all(|invite| invite.code != code)
			})
		})
		.await;
	let recording = running.mock().recording();
	assert_eq!(
		recording
			.requests_to("DELETE", &format!("/invites/{code}"))
			.count(),
		1
	);
	assert_eq!(
		recording
			.requests_to("GET", &format!("/channels/{invited}/invites"))
			.count(),
		2,
		"the list loads again after a revoke"
	);
	running
		.session()
		.send(Command::LoadVoiceRegions { guild_id: guild.id });
	running
		.until("the voice regions", |state: &State| {
			state.voice_regions(guild.id).is_some_and(|regions| {
				regions.iter().any(|region| region.id == "rotterdam")
					&& regions.iter().all(|region| !region.deprecated)
			})
		})
		.await;
	running.stop().await;
}
