use protocol::Snowflake;

const APP_SCHEMES: [&str; 2] = ["discord", "kyoko"];
const WEB_HOSTS: [&str; 4] = [
	"discord.com",
	"ptb.discord.com",
	"canary.discord.com",
	"discordapp.com",
];
const MAX_LENGTH: usize = 2048;
const DIRECT_MESSAGES: &str = "@me";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepLink {
	Home,
	Channel {
		guild_id: Option<Snowflake>,
		channel_id: Snowflake,
		message_id: Option<Snowflake>,
	},
}

pub fn parse_deep_link(url: &str) -> Option<DeepLink> {
	let url = url.trim();
	if url.len() > MAX_LENGTH {
		return None;
	}
	let (scheme, rest) = url.split_once(':')?;
	let scheme = scheme.to_ascii_lowercase();
	let rest = rest.strip_prefix("//").unwrap_or(rest);
	let rest = rest.split(['?', '#']).next().unwrap_or_default();
	let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
	let app = APP_SCHEMES.contains(&scheme.as_str());
	let web = matches!(scheme.as_str(), "http" | "https")
		&& WEB_HOSTS.contains(&host.to_ascii_lowercase().as_str());
	if !app && !web {
		return None;
	}
	let mut segments = [if app { host } else { "" }, path]
		.into_iter()
		.flat_map(|part| part.split('/'))
		.filter(|segment| !segment.is_empty() && *segment != "-");
	match segments.next() {
		Some(segment) if segment.eq_ignore_ascii_case("channels") => {
			channel(segments.next()?, segments.next()?, segments.next())
		}
		_ => app.then_some(DeepLink::Home),
	}
}

fn channel(guild: &str, channel: &str, message: Option<&str>) -> Option<DeepLink> {
	let guild_id = if guild == DIRECT_MESSAGES {
		None
	} else {
		Some(snowflake(guild)?)
	};
	let message_id = match message {
		Some(message) => Some(snowflake(message)?),
		None => None,
	};
	Some(DeepLink::Channel {
		guild_id,
		channel_id: snowflake(channel)?,
		message_id,
	})
}

fn snowflake(text: &str) -> Option<Snowflake> {
	if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
		return None;
	}
	text.parse::<u64>().ok().filter(|id| *id != 0).map(Snowflake)
}
