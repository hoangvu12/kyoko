use bridge::{
	ChannelPage, channel_right_groups, channel_settings, overwrite_from, overwrite_rows, right_flags,
};
use protocol::{ChannelType, Permissions, Snowflake};
use session::ChannelChanges;

use crate::harness::Running;

fn first_of(state: &state::State, guild_id: Snowflake, kind: ChannelType) -> Snowflake {
	let mut found: Vec<(i64, Snowflake)> = state
		.guild(guild_id)
		.expect("the guild")
		.channels
		.values()
		.filter(|channel| channel.kind == kind)
		.map(|channel| (channel.position.unwrap_or(0), channel.id))
		.collect();
	found.sort();
	found[0].1
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn channel_settings_project_the_form_and_diff_only_what_changed() {
	let mut running = Running::start("channel-settings-form").await;
	let guild_id = Snowflake(running.server.fixtures().guilds[0].id);
	running
		.until_state("the first guild", |state| state.guild(guild_id).is_some())
		.await;
	{
		let state = running.session().state().read();
		let voice = channel_settings(&state, first_of(&state, guild_id, ChannelType::GuildVoice))
			.expect("a manageable voice channel");
		assert_eq!(voice.label, "Voice");
		assert!(voice.audio && !voice.topic && !voice.archives && !voice.announces);
		assert_eq!(
			(voice.bitrate_min, voice.bitrate_max, voice.user_limit_max),
			(8_000, 128_000, 99)
		);
		assert_eq!(
			voice.pages,
			[
				ChannelPage::Overview,
				ChannelPage::Permissions,
				ChannelPage::Invites
			]
		);
		let mut wanted = voice.form.clone();
		wanted.bitrate = 96_000;
		wanted.region = "rotterdam".to_owned();
		wanted.name = format!("  {}  ", voice.form.name);
		assert_eq!(
			voice.form.changes(&wanted),
			ChannelChanges {
				bitrate: Some(96_000),
				rtc_region: Some(Some("rotterdam".to_owned())),
				..ChannelChanges::default()
			},
			"a name that only gained spaces is unchanged"
		);
		assert!(voice.form.changes(&voice.form).is_empty());

		let text = channel_settings(&state, first_of(&state, guild_id, ChannelType::GuildText))
			.expect("a manageable text channel");
		assert!(text.topic && text.archives && text.announces && !text.audio);
		let titles: Vec<&str> = channel_right_groups(ChannelType::GuildText)
			.iter()
			.map(|group| group.title)
			.collect();
		assert!(titles.contains(&"General Channel Permissions"));
		assert!(
			!titles.contains(&"Voice Channel Permissions"),
			"a text channel has no voice rights"
		);
		assert!(
			channel_right_groups(ChannelType::GuildVoice)
				.iter()
				.flat_map(|group| group.rights.iter())
				.all(|right| right.name != "Ban Members" && right.name != "Administrator"),
			"server-wide rights are not channel overwrites"
		);
		let rows = overwrite_rows(&state, text.id);
		assert!(rows[0].everyone, "@everyone always heads the list");

		let flags = right_flags();
		let view = flags
			.iter()
			.position(|flag| *flag == Permissions::VIEW_CHANNEL)
			.expect("View Channel")
			+ 1;
		let send = flags
			.iter()
			.position(|flag| *flag == Permissions::SEND_MESSAGES)
			.expect("Send Messages")
			+ 1;
		assert_eq!(
			overwrite_from(
				(Permissions::SEND_MESSAGES, Permissions::empty()),
				&[(view, -1), (send, 0)]
			),
			(Permissions::empty(), Permissions::VIEW_CHANNEL),
			"deny, inherit and allow replace what was set"
		);
	}
	running.stop().await;
}
