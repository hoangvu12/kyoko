use protocol::{ChannelType, Message, Snowflake};
use session::{Alert, AlertKind};
use state::State;
use store::Settings;

const BODY_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastText {
	pub title: String,
	pub body: String,
}

pub trait AlertOutlet {
	fn chime(&self);
	fn toast(&self, alert: &Alert, text: ToastText);
}

pub fn raise_alerts(state: &State, settings: &Settings, alerts: &[Alert], outlet: &dyn AlertOutlet) {
	if alerts.is_empty() {
		return;
	}
	let notifications = &settings.notifications;
	if notifications.sounds && !settings.streamer_mode.mutes_sounds() {
		outlet.chime();
	}
	if !notifications.enabled {
		return;
	}
	for alert in alerts {
		outlet.toast(alert, toast_text(state, alert, notifications.show_content));
	}
}

pub fn toast_text(state: &State, alert: &Alert, show_content: bool) -> ToastText {
	let message = &alert.message;
	let author = state.display_name(alert.guild_id, message.author.id);
	ToastText {
		title: title(state, alert, author),
		body: if show_content {
			body(state, alert.guild_id, message)
		} else {
			hidden_body(alert.kind).to_owned()
		},
	}
}

fn title(state: &State, alert: &Alert, author: String) -> String {
	let channel = state.channel(alert.channel_id());
	match alert.guild_id.and_then(|id| state.guild(id)) {
		Some(guild) => {
			let place = channel
				.and_then(|channel| channel.name.as_deref())
				.unwrap_or("unknown");
			format!("{author} (#{place}, {})", guild.name)
		}
		None => match channel.filter(|channel| channel.kind == ChannelType::GroupDm) {
			Some(group) => match group.name.as_deref().filter(|name| !name.is_empty()) {
				Some(name) => format!("{author} ({name})"),
				None => author,
			},
			None => author,
		},
	}
}

fn hidden_body(kind: AlertKind) -> &'static str {
	match kind {
		AlertKind::Mention => "Mentioned you",
		AlertKind::DirectMessage | AlertKind::Message => "Sent a message",
	}
}

fn body(state: &State, guild_id: Option<Snowflake>, message: &Message) -> String {
	let text = readable(state, guild_id, &message.content);
	let text = text.trim();
	if !text.is_empty() {
		return shorten(text);
	}
	if !message.attachments.is_empty() {
		return "Sent an attachment".to_owned();
	}
	if !message.sticker_items.is_empty() {
		return "Sent a sticker".to_owned();
	}
	if !message.embeds.is_empty() {
		return "Sent an embed".to_owned();
	}
	"Sent a message".to_owned()
}

fn shorten(text: &str) -> String {
	match text.char_indices().nth(BODY_LIMIT) {
		Some((end, _)) => format!("{}…", &text[..end]),
		None => text.to_owned(),
	}
}

fn readable(state: &State, guild_id: Option<Snowflake>, content: &str) -> String {
	let mut out = String::with_capacity(content.len());
	let mut rest = content;
	while let Some(start) = rest.find('<') {
		out.push_str(&rest[..start]);
		let tail = &rest[start..];
		match tail
			.find('>')
			.and_then(|end| Some((token(state, guild_id, &tail[1..end])?, end)))
		{
			Some((replacement, end)) => {
				out.push_str(&replacement);
				rest = &tail[end + 1..];
			}
			None => {
				out.push('<');
				rest = &tail[1..];
			}
		}
	}
	out.push_str(rest);
	out
}

fn token(state: &State, guild_id: Option<Snowflake>, inner: &str) -> Option<String> {
	if let Some(id) = inner.strip_prefix("@&") {
		let role = guild_id
			.and_then(|guild| state.guild(guild))
			.and_then(|guild| guild.roles.get(&id_of(id)?))
			.map_or_else(|| "role".to_owned(), |role| role.name.clone());
		return Some(format!("@{role}"));
	}
	if let Some(id) = inner.strip_prefix('@') {
		let id = id_of(id.strip_prefix('!').unwrap_or(id))?;
		return Some(format!("@{}", state.display_name(guild_id, id)));
	}
	if let Some(id) = inner.strip_prefix('#') {
		let name = state
			.channel(id_of(id)?)
			.and_then(|channel| channel.name.clone())
			.unwrap_or_else(|| "channel".to_owned());
		return Some(format!("#{name}"));
	}
	let emoji = inner.strip_prefix("a:").or_else(|| inner.strip_prefix(':'))?;
	let (name, id) = emoji.split_once(':')?;
	id_of(id)?;
	Some(format!(":{name}:"))
}

fn id_of(text: &str) -> Option<Snowflake> {
	text.parse::<u64>().ok().map(Snowflake)
}
