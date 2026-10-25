use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use bridge::{ATTACHMENT_BOX, EMBED_MEDIA_BOX, EMBED_THUMBNAIL, MessageEntry, ReplyPreview, STICKER_SIDE};
use markdown::Node;
use media::cdn::Quality;
use media::{Gallery, ImageName, ImageRequest, Status};
use protocol::{
	Attachment, Embed, EmbedKind, EmbedMedia, MessageFlags, PartialEmoji, Snowflake, StickerItem, User,
};
use zgx::image::ImageData;
use zgx::reactive::{Runtime, Signal};

pub const AVATAR: f64 = 40.;
pub const REPLY_AVATAR: f64 = 16.;
pub const EMOJI: f64 = 22.;
pub const JUMBO_EMOJI: f64 = 48.;
pub const REACTION_EMOJI: f64 = 16.;
pub const EMBED_ICON: f64 = 24.;
const PLACEHOLDER_SIDE: u32 = 32;
const PLACEHOLDER_LIMIT: usize = 256;

pub type Picture = Signal<Option<Arc<ImageData>>>;

pub struct Pictures {
	runtime: Runtime,
	gallery: Gallery,
	cdn: String,
	mock_origin: Option<String>,
	scale: Cell<f64>,
	quality: Cell<Quality>,
	avatars: RefCell<HashMap<(Snowflake, String, u32), ImageName>>,
	pictures: RefCell<HashMap<ImageName, Picture>>,
	placeholders: RefCell<HashMap<String, Option<Arc<ImageData>>>>,
}

impl Pictures {
	pub fn new(runtime: Runtime, gallery: Gallery, cdn: String, mock_origin: Option<String>) -> Self {
		Self {
			runtime,
			gallery,
			cdn,
			mock_origin,
			scale: Cell::new(1.),
			quality: Cell::new(Quality::Fitted),
			avatars: RefCell::default(),
			pictures: RefCell::default(),
			placeholders: RefCell::default(),
		}
	}

	pub fn runtime(&self) -> &Runtime {
		&self.runtime
	}

	pub fn set_quality(&self, quality: Quality) {
		self.quality.set(quality);
	}

	pub fn set_scale(&self, scale: f64) {
		if self.scale.replace(scale) != scale {
			self.avatars.borrow_mut().clear();
		}
	}

	fn request(&self, mut request: ImageRequest) -> ImageName {
		request.url = bridge::local_media(&request.url, self.mock_origin.as_deref());
		self.gallery.request(request)
	}

	pub fn avatar(&self, user: &User, points: f64) -> Option<ImageName> {
		let key = (user.id, user.avatar.clone().unwrap_or_default(), points as u32);
		if let Some(name) = self.avatars.borrow().get(&key) {
			return Some(name.clone());
		}
		let request = bridge::avatar_request(&self.cdn, user, points, self.scale.get(), false)?;
		let name = self.request(request);
		self.avatars.borrow_mut().insert(key, name.clone());
		Some(name)
	}

	pub fn emoji(&self, id: u64, animated: bool, points: f64) -> ImageName {
		self.request(bridge::emoji_request(
			&self.cdn,
			Snowflake(id),
			animated,
			points,
			self.scale.get(),
		))
	}

	pub fn guild_icon(&self, guild_id: Snowflake, hash: &str, points: f32) -> ImageName {
		self.request(bridge::guild_icon_request(
			&self.cdn,
			guild_id,
			hash,
			f64::from(points),
			self.scale.get(),
			false,
		))
	}

	pub fn reaction(&self, emoji: &PartialEmoji) -> Option<ImageName> {
		let id = emoji.id?;
		Some(self.emoji(id.0, emoji.animated, REACTION_EMOJI))
	}

	pub fn attachment(
		&self,
		attachment: &Attachment,
		channel_id: Snowflake,
		message_id: Snowflake,
	) -> Option<ImageName> {
		let (width, height) = bridge::fit_box(attachment.width, attachment.height, ATTACHMENT_BOX);
		let request = bridge::attachment_request(
			attachment,
			channel_id,
			message_id,
			(f64::from(width), f64::from(height)),
			self.scale.get(),
			self.quality.get(),
		)?;
		Some(self.request(request))
	}

	pub fn embed_media(&self, media: &EmbedMedia, size: (f32, f32)) -> Option<ImageName> {
		let request = bridge::embed_request(media, (f64::from(size.0), f64::from(size.1)), self.scale.get())?;
		Some(self.request(request))
	}

	pub fn embed_icon(&self, url: &str) -> ImageName {
		self.request(bridge::embed_icon_request(url, EMBED_ICON, self.scale.get()))
	}

	pub fn sticker(&self, sticker: &StickerItem) -> Option<ImageName> {
		let request = bridge::sticker_request(sticker, f64::from(STICKER_SIDE), self.scale.get())?;
		Some(self.request(request))
	}

	pub fn picture(&self, name: &ImageName) -> Picture {
		self.pictures
			.borrow_mut()
			.entry(name.clone())
			.or_insert_with(|| self.runtime.signal(None))
			.clone()
	}

	pub fn is_settled(&self, name: &ImageName) -> bool {
		let shown = self
			.pictures
			.borrow()
			.get(name)
			.is_some_and(|picture| picture.with_untracked(Option::is_some));
		shown || matches!(self.gallery.status(name), Some(Status::Failed) | None)
	}

	pub fn show(&self, name: &ImageName, image: Option<Arc<ImageData>>) {
		let picture = match image {
			Some(_) => Some(self.picture(name)),
			None => self.pictures.borrow().get(name).cloned(),
		};
		if let Some(picture) = picture {
			picture.update(|slot| *slot = image);
		}
	}

	pub fn placeholder(&self, hash: &str) -> Option<Arc<ImageData>> {
		if let Some(found) = self.placeholders.borrow().get(hash) {
			return found.clone();
		}
		let decoded = media::placeholder_from_discord(hash, PLACEHOLDER_SIDE, PLACEHOLDER_SIDE)
			.ok()
			.and_then(|image| ImageData::new(image.width, image.height, image.rgba).ok())
			.map(Arc::new);
		let mut placeholders = self.placeholders.borrow_mut();
		if placeholders.len() >= PLACEHOLDER_LIMIT {
			placeholders.clear();
		}
		placeholders.insert(hash.to_owned(), decoded.clone());
		decoded
	}

	pub fn names_in(&self, entry: &MessageEntry, inline_media: bool) -> Vec<ImageName> {
		let message = &entry.message;
		let mut names = Vec::new();
		if entry.is_plain() && !entry.grouped {
			names.extend(self.avatar(&message.author, AVATAR));
		}
		if let Some(ReplyPreview::Message(quote) | ReplyPreview::Deleted(quote)) = &entry.reply {
			names.extend(self.avatar(&quote.author, REPLY_AVATAR));
		}
		let size = if entry.body.jumbo { JUMBO_EMOJI } else { EMOJI };
		for document in entry.body.documents() {
			self.collect_emoji(&document.nodes, size, &mut names);
		}
		if inline_media {
			names.extend(
				message
					.attachments
					.iter()
					.filter(|attachment| bridge::is_image(attachment))
					.filter_map(|attachment| self.attachment(attachment, message.channel_id, message.id)),
			);
		}
		if !message.flags.contains(MessageFlags::SUPPRESS_EMBEDS) {
			for embed in message.embeds.iter() {
				self.collect_embed(embed, &mut names);
			}
		}
		names.extend(
			message
				.sticker_items
				.iter()
				.filter_map(|sticker| self.sticker(sticker)),
		);
		names.extend(
			message
				.reactions
				.iter()
				.filter_map(|reaction| self.reaction(&reaction.emoji)),
		);
		names.dedup();
		names
	}

	fn collect_emoji(&self, nodes: &[Node], size: f64, names: &mut Vec<ImageName>) {
		for node in nodes {
			match node {
				Node::CustomEmoji { id, animated, .. } => names.push(self.emoji(*id, *animated, size)),
				Node::Bold(children)
				| Node::Italic(children)
				| Node::Underline(children)
				| Node::Strike(children)
				| Node::Spoiler(children)
				| Node::Quote(children)
				| Node::Subtext(children)
				| Node::Heading { children, .. }
				| Node::Link {
					text: Some(children), ..
				} => self.collect_emoji(children, EMOJI, names),
				Node::List { items, .. } => {
					for item in items {
						self.collect_emoji(item, EMOJI, names);
					}
				}
				_ => {}
			}
		}
	}

	fn collect_embed(&self, embed: &Embed, names: &mut Vec<ImageName>) {
		if matches!(embed.kind, Some(EmbedKind::Image | EmbedKind::Gifv)) {
			if let Some(media) = bare_media(embed) {
				names.extend(
					self.embed_media(media, bridge::fit_box(media.width, media.height, ATTACHMENT_BOX)),
				);
			}
			return;
		}
		if let Some(url) = embed
			.author
			.as_ref()
			.and_then(|author| author.proxy_icon_url.as_deref().or(author.icon_url.as_deref()))
		{
			names.push(self.embed_icon(url));
		}
		if let Some(media) = large_media(embed) {
			names
				.extend(self.embed_media(media, bridge::fit_box(media.width, media.height, EMBED_MEDIA_BOX)));
		}
		if let Some(thumbnail) = side_thumbnail(embed) {
			names.extend(self.embed_media(thumbnail, (EMBED_THUMBNAIL, EMBED_THUMBNAIL)));
		}
	}
}

pub fn bare_media(embed: &Embed) -> Option<&EmbedMedia> {
	embed
		.image
		.as_ref()
		.or(embed.thumbnail.as_ref())
		.or(embed.video.as_ref())
}

pub fn large_media(embed: &Embed) -> Option<&EmbedMedia> {
	match (&embed.image, &embed.video, &embed.thumbnail) {
		(Some(image), None, _) => Some(image),
		(_, Some(video), thumbnail) => thumbnail.as_ref().or(Some(video)),
		(None, None, _) => None,
	}
}

pub fn side_thumbnail(embed: &Embed) -> Option<&EmbedMedia> {
	embed
		.thumbnail
		.as_ref()
		.filter(|_| embed.image.is_none() && embed.video.is_none())
}
