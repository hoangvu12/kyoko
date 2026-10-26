use protocol::Snowflake;
use rest::UploadFile;
use rest::api::image_data_uri;
use state::{EMOJI_MAX_BYTES, ExpressionKind, STICKER_MAX_BYTES, emoji_name_problem, sticker_problem};

use super::guild_writes::{Call, Write};
use super::{Command, Core};

const KIB: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picture {
	Png,
	Gif,
	Jpeg,
	Webp,
}

impl Picture {
	fn sniff(bytes: &[u8]) -> Option<Picture> {
		if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
			Some(Picture::Png)
		} else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
			Some(Picture::Gif)
		} else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
			Some(Picture::Jpeg)
		} else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
			Some(Picture::Webp)
		} else {
			None
		}
	}

	fn mime(self) -> &'static str {
		match self {
			Picture::Png => "image/png",
			Picture::Gif => "image/gif",
			Picture::Jpeg => "image/jpeg",
			Picture::Webp => "image/webp",
		}
	}

	fn extension(self) -> &'static str {
		match self {
			Picture::Png => "png",
			Picture::Gif => "gif",
			Picture::Jpeg => "jpg",
			Picture::Webp => "webp",
		}
	}
}

impl Core {
	pub(super) fn on_upload_command(&mut self, command: Command) {
		match command {
			Command::CreateEmoji {
				guild_id,
				name,
				image,
			} => self.create_emoji(guild_id, name, &image),
			Command::CreateSticker {
				guild_id,
				name,
				description,
				tags,
				file,
			} => self.create_sticker(guild_id, name, description, tags, file),
			_ => {}
		}
	}

	fn create_emoji(&mut self, guild_id: Snowflake, name: String, image: &[u8]) {
		if !self.may_upload(guild_id) {
			return;
		}
		let Some(picture) = Picture::sniff(image) else {
			self.reject("An emoji must be a PNG, GIF, JPEG or WebP image");
			return;
		};
		if image.len() > EMOJI_MAX_BYTES {
			self.reject(format!(
				"An emoji image must be at most {} KB",
				EMOJI_MAX_BYTES / KIB
			));
			return;
		}
		if let Some(problem) = emoji_name_problem(&name) {
			self.reject(problem);
			return;
		}
		let kind = if picture == Picture::Gif {
			ExpressionKind::AnimatedEmoji
		} else {
			ExpressionKind::Emoji
		};
		if !self.has_free_slot(guild_id, kind) {
			return;
		}
		let image = image_data_uri(picture.mime(), image);
		self.queue_guild_write(
			guild_id,
			Write::new("create emoji", Call::CreateEmoji { name, image }),
		);
	}

	fn create_sticker(
		&mut self,
		guild_id: Snowflake,
		name: String,
		description: String,
		tags: String,
		file: Vec<u8>,
	) {
		if !self.may_upload(guild_id) {
			return;
		}
		let picture = match Picture::sniff(&file) {
			Some(picture @ (Picture::Png | Picture::Gif)) => picture,
			_ => {
				self.reject("A sticker must be a PNG, APNG or GIF image");
				return;
			}
		};
		if file.len() > STICKER_MAX_BYTES {
			self.reject(format!(
				"A sticker file must be at most {} KB",
				STICKER_MAX_BYTES / KIB
			));
			return;
		}
		if let Some(problem) = sticker_problem(&name, &description, &tags) {
			self.reject(problem);
			return;
		}
		if !self.has_free_slot(guild_id, ExpressionKind::Sticker) {
			return;
		}
		let file = UploadFile {
			name: format!("{}.{}", name.trim(), picture.extension()),
			bytes: file.into(),
			content_type: picture.mime().to_owned(),
		};
		let call = Call::CreateSticker {
			name: name.trim().to_owned(),
			description: description.trim().to_owned(),
			tags: tags.trim().to_owned(),
			file,
		};
		self.queue_guild_write(guild_id, Write::new("create sticker", call));
	}

	fn may_upload(&mut self, guild_id: Snowflake) -> bool {
		let allowed = self.state.read().may_create_expressions(guild_id);
		if !allowed {
			self.reject("You need the Create Expressions permission");
		}
		allowed
	}

	fn has_free_slot(&mut self, guild_id: Snowflake, kind: ExpressionKind) -> bool {
		let (slots, server) = {
			let state = self.state.read();
			(
				state.expression_slots(guild_id, kind),
				state
					.guild(guild_id)
					.map(|guild| guild.name.clone())
					.unwrap_or_default(),
			)
		};
		match slots {
			Some(slots) if slots.full() => {
				self.reject(format!(
					"{server} has no free {} slots ({} of {} used)",
					kind.noun(),
					slots.used,
					slots.max
				));
				false
			}
			_ => true,
		}
	}
}
