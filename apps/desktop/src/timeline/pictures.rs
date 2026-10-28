use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use bridge::{
	ATTACHMENT_BOX, Avatar, EMBED_MEDIA_BOX, EMBED_THUMBNAIL, MessageEntry, ReplyPreview, STICKER_SIDE,
};
use markdown::Node;
use media::cdn::Quality;
use media::{Gallery, ImageName, ImageRequest, Shape, SizeHint, Status, Variant};
use protocol::{
	Attachment, Embed, EmbedKind, EmbedMedia, MessageFlags, PartialEmoji, Snowflake, StickerItem, User,
};
use zgx::image::ImageData;
use zgx::reactive::{Runtime, Signal};

use super::voice::Listening;

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
	listening: Rc<Listening>,
	cdn: String,
	mock_origin: Option<String>,
	scale: Cell<f64>,
	quality: Cell<Quality>,
	avatars: RefCell<HashMap<(Avatar, u32, bool), ImageName>>,
	hovered_avatar: RefCell<Option<ImageName>>,
	hover_watchers: RefCell<Vec<Rc<dyn Fn()>>>,
	pictures: RefCell<HashMap<ImageName, Picture>>,
	placeholders: RefCell<HashMap<String, Option<Arc<ImageData>>>>,
}

impl Pictures {
	pub fn new(runtime: Runtime, gallery: Gallery, cdn: String, mock_origin: Option<String>) -> Self {
		Self {
			listening: Rc::new(Listening::new(&runtime, &gallery, mock_origin.clone())),
			runtime,
			gallery,
			cdn,
			mock_origin,
			scale: Cell::new(1.),
			quality: Cell::new(Quality::Fitted),
			avatars: RefCell::default(),
			hovered_avatar: RefCell::default(),
			hover_watchers: RefCell::default(),
			pictures: RefCell::default(),
			placeholders: RefCell::default(),
		}
	}

	pub fn runtime(&self) -> &Runtime {
		&self.runtime
	}

	pub fn listening(&self) -> Rc<Listening> {
		self.listening.clone()
	}

	pub fn set_quality(&self, quality: Quality) {
		self.quality.set(quality);
	}

	pub fn set_scale(&self, scale: f64) {
		if self.scale.replace(scale) != scale {
			self.avatars.borrow_mut().clear();
		}
	}

	pub fn scale(&self) -> f64 {
		self.scale.get()
	}

	pub fn cdn(&self) -> &str {
		&self.cdn
	}

	pub fn request(&self, mut request: ImageRequest) -> ImageName {
		request.url = bridge::local_media(&request.url, self.mock_origin.as_deref());
		request.fallback = request
			.fallback
			.map(|url| bridge::local_media(&url, self.mock_origin.as_deref()));
		self.gallery.request(request)
	}

	pub fn avatar(&self, user: &User, points: f64) -> Option<ImageName> {
		Some(self.member_avatar(&Avatar::of_user(user), points, false))
	}

	pub fn member_avatar(&self, avatar: &Avatar, points: f64, animate: bool) -> ImageName {
		let key = (avatar.clone(), points as u32, animate && avatar.is_animated());
		if let Some(name) = self.avatars.borrow().get(&key) {
			return name.clone();
		}
		let name = self.request(avatar.request(&self.cdn, points, self.scale.get(), key.2));
		self.avatars.borrow_mut().insert(key, name.clone());
		name
	}

	pub fn moving_avatar(&self, avatar: &Avatar, points: f64) -> Option<ImageName> {
		avatar
			.is_animated()
			.then(|| self.member_avatar(avatar, points, true))
	}

	pub fn hover_avatar(&self, name: &ImageName, over: bool) {
		let changed = {
			let mut hovered = self.hovered_avatar.borrow_mut();
			match (over, hovered.as_ref() == Some(name)) {
				(true, false) => {
					*hovered = Some(name.clone());
					true
				}
				(false, true) => {
					*hovered = None;
					true
				}
				_ => false,
			}
		};
		if !changed {
			return;
		}
		self.gallery.set_hovered(over.then_some(name.as_str()));
		let watchers: Vec<Rc<dyn Fn()>> = self.hover_watchers.borrow().clone();
		for watcher in watchers {
			watcher();
		}
	}

	pub fn hovered_avatar(&self) -> Option<ImageName> {
		self.hovered_avatar.borrow().clone()
	}

	pub fn on_avatar_hover(&self, watcher: impl Fn() + 'static) {
		self.hover_watchers.borrow_mut().push(Rc::new(watcher));
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
		tile: (f32, f32),
	) -> Option<ImageName> {
		let (width, height) = bridge::cover_box(attachment.width, attachment.height, tile);
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

	pub fn bare_embed(&self, embed: &Embed, media: &EmbedMedia, size: (f32, f32)) -> Option<ImageName> {
		if embed.kind != Some(EmbedKind::Gifv) {
			return self.embed_media(media, size);
		}
		let request = bridge::gifv_request(embed, (f64::from(size.0), f64::from(size.1)), self.scale.get())?;
		Some(self.request(request))
	}

	pub fn embed_icon(&self, url: &str) -> ImageName {
		self.request(bridge::embed_icon_request(url, EMBED_ICON, self.scale.get()))
	}

	pub fn sticker(&self, sticker: &StickerItem) -> Option<ImageName> {
		self.sticker_cell(sticker, STICKER_SIDE)
	}

	pub fn sticker_cell(&self, sticker: &StickerItem, points: f32) -> Option<ImageName> {
		let request = bridge::sticker_request(sticker, f64::from(points), self.scale.get())?;
		Some(self.request(request))
	}

	pub fn remote(&self, url: &str, size: (f32, f32)) -> ImageName {
		let scale = self.scale.get();
		let hint = SizeHint::new(
			media::cdn::device_pixels(f64::from(size.0), scale),
			media::cdn::device_pixels(f64::from(size.1), scale),
		);
		self.request(ImageRequest::new(url, Variant::new(hint, Shape::Square)))
	}

	pub fn lightbox(&self, image: &bridge::LightboxImage, window: (f32, f32)) -> Option<ImageName> {
		let request = bridge::lightbox_request(image, window, self.scale.get())?;
		Some(self.request(request))
	}

	pub fn pixels(&self, name: &ImageName) -> Option<media::Pixels> {
		self.gallery.pixels(name)
	}

	pub fn save_original(
		&self,
		url: &str,
		path: std::path::PathBuf,
	) -> tokio::sync::oneshot::Receiver<Result<(), String>> {
		let url = bridge::local_media(url, self.mock_origin.as_deref());
		let loader = self.gallery.loader().clone();
		let (sender, receiver) = tokio::sync::oneshot::channel();
		self.gallery.runtime().spawn(async move {
			let saved = match loader.bytes(&url).await {
				Ok(bytes) => tokio::fs::write(&path, &bytes)
					.await
					.map_err(|error| error.to_string()),
				Err(error) => Err(error.to_string()),
			};
			let _ = sender.send(saved);
		});
		receiver
	}

	pub fn hover(&self, name: Option<&ImageName>) {
		self.gallery.set_hovered(name.map(ImageName::as_str));
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
			names.push(self.member_avatar(&entry.avatar, AVATAR, false));
		}
		if let Some(ReplyPreview::Message(quote) | ReplyPreview::Deleted(quote)) = &entry.reply {
			names.push(self.member_avatar(&quote.avatar, REPLY_AVATAR, false));
		}
		let size = if entry.body.jumbo { JUMBO_EMOJI } else { EMOJI };
		for document in entry.body.documents() {
			self.collect_emoji(&document.nodes, size, &mut names);
		}
		if inline_media {
			let images = bridge::mosaic_images(&message.attachments);
			let mosaic = bridge::image_mosaic(&images);
			names.extend(images.iter().zip(&mosaic.tiles).filter_map(|(attachment, tile)| {
				self.attachment(attachment, message.channel_id, message.id, tile.size())
			}));
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
		if let Some(poll) = &message.poll {
			names.extend(
				poll.answers
					.iter()
					.filter_map(|answer| self.reaction(answer.poll_media.emoji.as_ref()?)),
			);
		}
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
				names.extend(self.bare_embed(
					embed,
					media,
					bridge::fit_box(media.width, media.height, bare_box(embed)),
				));
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

pub fn bare_box(embed: &Embed) -> (f32, f32) {
	if embed.kind == Some(EmbedKind::Gifv) {
		EMBED_MEDIA_BOX
	} else {
		ATTACHMENT_BOX
	}
}

pub fn only_link(entry: &MessageEntry) -> bool {
	let content = entry.content.trim();
	!content.is_empty()
		&& !entry.message.flags.contains(MessageFlags::SUPPRESS_EMBEDS)
		&& entry.message.embeds.iter().any(|embed| {
			matches!(embed.kind, Some(EmbedKind::Image | EmbedKind::Gifv))
				&& embed.url.as_deref() == Some(content)
		})
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
