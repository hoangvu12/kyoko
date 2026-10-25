use bridge::{DeepLink, parse_deep_link};
use protocol::Snowflake;

fn channel(guild: Option<u64>, channel: u64, message: Option<u64>) -> Option<DeepLink> {
	Some(DeepLink::Channel {
		guild_id: guild.map(Snowflake),
		channel_id: Snowflake(channel),
		message_id: message.map(Snowflake),
	})
}

#[test]
fn discord_scheme_links_open_channels_and_messages() {
	assert_eq!(
		parse_deep_link("discord://-/channels/10/20"),
		channel(Some(10), 20, None)
	);
	assert_eq!(
		parse_deep_link("discord://-/channels/10/20/30"),
		channel(Some(10), 20, Some(30))
	);
	assert_eq!(
		parse_deep_link("discord:///channels/@me/42"),
		channel(None, 42, None)
	);
	assert_eq!(
		parse_deep_link("DISCORD://channels/10/20?x=1"),
		channel(Some(10), 20, None)
	);
	assert_eq!(
		parse_deep_link("kyoko://-/channels/@me/42/43"),
		channel(None, 42, Some(43))
	);
}

#[test]
fn web_links_to_discord_hosts_open_channels() {
	assert_eq!(
		parse_deep_link("https://discord.com/channels/10/20/30"),
		channel(Some(10), 20, Some(30))
	);
	assert_eq!(
		parse_deep_link("https://canary.discord.com/channels/@me/5#top"),
		channel(None, 5, None)
	);
	assert_eq!(parse_deep_link("https://discord.com/invite/abc"), None);
	assert_eq!(parse_deep_link("https://example.com/channels/10/20"), None);
}

#[test]
fn bare_app_links_only_bring_the_window_forward() {
	assert_eq!(parse_deep_link("discord://"), Some(DeepLink::Home));
	assert_eq!(parse_deep_link("discord://-/"), Some(DeepLink::Home));
	assert_eq!(parse_deep_link("discord://-/store"), Some(DeepLink::Home));
}

#[test]
fn malformed_links_are_ignored() {
	assert_eq!(parse_deep_link("discord://-/channels/abc/20"), None);
	assert_eq!(parse_deep_link("discord://-/channels/10"), None);
	assert_eq!(parse_deep_link("discord://-/channels/10/0"), None);
	assert_eq!(
		parse_deep_link("discord://-/channels/10/99999999999999999999999"),
		None
	);
	assert_eq!(parse_deep_link("file:///C:/channels/10/20"), None);
	assert_eq!(parse_deep_link("not a link"), None);
	assert_eq!(
		parse_deep_link(&format!("discord://-/channels/10/{}", "1".repeat(3000))),
		None
	);
}
