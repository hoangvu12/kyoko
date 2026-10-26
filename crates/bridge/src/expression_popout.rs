use protocol::Snowflake;
use protocol::expressions::unicode_emoji_list;
use state::{Expression, State};

pub const DEFAULT_EMOJI_NOTE: &str = "A default emoji. You can use this emoji everywhere on Discord.";
const VARIATION_SELECTOR: char = '\u{fe0f}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopoutSubject {
	Custom(Expression),
	Unicode(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopoutServer {
	pub guild_id: Snowflake,
	pub name: String,
	pub icon: Option<String>,
	pub members: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionDetails {
	pub title: String,
	pub description: Option<String>,
	pub note: Option<String>,
	pub source_heading: Option<&'static str>,
	pub server: Option<PopoutServer>,
}

fn bare(glyph: &str) -> String {
	glyph
		.chars()
		.filter(|character| *character != VARIATION_SELECTOR)
		.collect()
}

fn server(state: &State, guild_id: Snowflake) -> Option<PopoutServer> {
	let guild = state.guild(guild_id)?;
	Some(PopoutServer {
		guild_id,
		name: guild.name.clone(),
		icon: guild.icon.clone(),
		members: guild.member_count,
	})
}

fn emoji_guild(state: &State, id: Snowflake) -> Option<Snowflake> {
	state
		.guilds()
		.find(|guild| guild.emojis.iter().any(|emoji| emoji.id == Some(id)))
		.map(|guild| guild.id)
}

fn sticker_guild(state: &State, id: Snowflake) -> Option<Snowflake> {
	state
		.guilds()
		.find(|guild| {
			state
				.guild_stickers(guild.id)
				.is_some_and(|stickers| stickers.iter().any(|sticker| sticker.id == id))
		})
		.map(|guild| guild.id)
}

pub fn members_label(members: i64) -> String {
	let digits = members.max(0).to_string();
	let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
	for (index, digit) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			grouped.push(',');
		}
		grouped.push(digit);
	}
	let noun = if members == 1 { "member" } else { "members" };
	format!("{grouped} {noun}")
}

pub fn expression_details(state: &State, subject: &PopoutSubject) -> ExpressionDetails {
	match subject {
		PopoutSubject::Unicode(glyph) => {
			let wanted = bare(glyph);
			let name = unicode_emoji_list()
				.iter()
				.find(|emoji| bare(emoji.emoji) == wanted)
				.map_or_else(|| glyph.clone(), |emoji| format!(":{}:", emoji.shortcode));
			ExpressionDetails {
				title: name,
				description: Some(DEFAULT_EMOJI_NOTE.to_owned()),
				note: None,
				source_heading: None,
				server: None,
			}
		}
		PopoutSubject::Custom(Expression::Emoji { id, name, .. }) => {
			let server = emoji_guild(state, *id).and_then(|guild_id| server(state, guild_id));
			let description = match &server {
				Some(server) => format!("This emoji is from {}.", server.name),
				None => "This emoji is from a server you are not in.".to_owned(),
			};
			ExpressionDetails {
				title: format!(":{name}:"),
				description: Some(description),
				note: None,
				source_heading: server.as_ref().map(|_| "This emoji is from"),
				server,
			}
		}
		PopoutSubject::Custom(Expression::Sticker { id, name, .. }) => {
			let server = sticker_guild(state, *id).and_then(|guild_id| server(state, guild_id));
			let guild_sticker = server.as_ref().and_then(|server| {
				state
					.guild_stickers(server.guild_id)?
					.iter()
					.find(|sticker| sticker.id == *id)
			});
			let pack = state.sticker_packs().and_then(|packs| {
				packs
					.iter()
					.find(|pack| pack.stickers.iter().any(|sticker| sticker.id == *id))
			});
			let note = guild_sticker
				.and_then(|sticker| sticker.description.clone())
				.or_else(|| pack.and_then(|pack| pack.description.clone()))
				.filter(|note| !note.is_empty());
			let description = pack.map(|pack| format!("from {}", pack.name));
			ExpressionDetails {
				title: name.clone(),
				description,
				note,
				source_heading: server.as_ref().map(|_| "This sticker is from"),
				server,
			}
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

impl Area {
	pub fn right(&self) -> f32 {
		self.x + self.width
	}

	pub fn bottom(&self) -> f32 {
		self.y + self.height
	}

	pub fn overlaps(&self, other: &Area) -> bool {
		self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
	}
}

pub fn place_popout(item: Area, size: (f32, f32), bounds: (f32, f32), gap: f32, margin: f32) -> (f32, f32) {
	let (width, height) = size;
	let (limit_x, limit_y) = (bounds.0 - margin - width, bounds.1 - margin - height);
	let clamp_y = |y: f32| y.min(limit_y).max(margin);
	let clamp_x = |x: f32| x.min(limit_x).max(margin);
	let beside_y = clamp_y(item.bottom() - height);
	if item.right() + gap <= limit_x {
		return (item.right() + gap, beside_y);
	}
	if item.x - gap - width >= margin {
		return (item.x - gap - width, beside_y);
	}
	let x = clamp_x(item.x);
	if item.y - gap - height >= margin {
		(x, item.y - gap - height)
	} else {
		(x, item.bottom() + gap)
	}
}
