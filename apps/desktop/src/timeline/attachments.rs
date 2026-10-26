use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use bridge::{ATTACHMENT_BOX, EMBED_MEDIA_BOX, EMBED_THUMBNAIL, MessageEntry, STICKER_SIDE};
use media::ImageName;
use protocol::{Attachment, Embed, EmbedKind, EmbedMedia, FavoriteGif};
use session::Command;
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::input::{EventPhase, InputEvent, PointerButton};
use zgx::reactive::Signal;

use super::actions::aimed;
use super::body::{self, Look};
use super::icons::icon;
use super::pictures::{EMBED_ICON, Pictures, bare_box, bare_media, large_media, side_thumbnail};
use super::popout;
use super::rows::{RowEnv, TypeSize, tok};
use super::voice;

const EMBED_WIDTH: f32 = 432.;
const EMBED_ACCENT: f32 = 4.;
const EMBED_PADDING: f32 = 15.;
const EMBED_IMAGE_GAP: f32 = 12.;
const OVERLAY_INSET: f32 = 6.;
const STAR_BUTTON: f32 = 28.;
const STAR_ICON: f32 = 20.;
const FIELD_WIDTH: f32 = 242.;
const FILE_CARD_WIDTH: f32 = 432.;
const PLAY_DISC: f32 = 48.;
const SPOILER_STRIDE: usize = 1000;

pub fn picture(
	pictures: &Rc<Pictures>,
	name: Option<ImageName>,
	placeholder: Option<&str>,
	fit: ObjectFit,
) -> View {
	let shown = name.map(|name| pictures.picture(&name));
	let still: Option<Arc<ImageData>> = placeholder.and_then(|hash| pictures.placeholder(hash));
	div()
		.w_full()
		.h_full()
		.bg(tok("background_secondary"))
		.reactive_style(move || match shown.as_ref().and_then(Signal::get) {
			Some(image) => Styles::new().bg_image(image, fit),
			None => match &still {
				Some(still) => Styles::new().bg_image(still.clone(), ObjectFit::Fill),
				None => Styles::new(),
			},
		})
}

pub fn attachments(entry: &MessageEntry, env: &Rc<RowEnv>) -> Vec<View> {
	let message = &entry.message;
	message
		.attachments
		.iter()
		.map(|attachment| {
			if env.prefs.get().inline_media && bridge::is_image(attachment) {
				image_attachment(entry, attachment, env)
			} else if bridge::is_voice_message(message, attachment) {
				voice::voice_message(message, attachment, env).unwrap_or_else(|| file_card(attachment, env))
			} else if bridge::is_audio(attachment) {
				voice::audio_card(message, attachment, env).unwrap_or_else(|| file_card(attachment, env))
			} else {
				file_card(attachment, env)
			}
		})
		.collect()
}

fn image_attachment(entry: &MessageEntry, attachment: &Attachment, env: &Rc<RowEnv>) -> View {
	let (width, height) = bridge::fit_box(attachment.width, attachment.height, ATTACHMENT_BOX);
	let name = env
		.pictures
		.attachment(attachment, entry.message.channel_id, entry.message.id);
	let target = bridge::LightboxImage::attachment(attachment, entry.message.channel_id, entry.message.id);
	let frame = div()
		.mt(4.)
		.size(width, height)
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.child(picture(
			&env.pictures,
			name.clone(),
			attachment.placeholder.as_deref(),
			ObjectFit::Cover,
		));
	match bridge::favorite_gif_attachment(attachment) {
		Some(favorite) => gif(frame, name, favorite, target, env),
		None => frame.when_some(target, |frame, target| opens(frame, target, env)),
	}
}

fn gif(
	frame: View,
	name: Option<ImageName>,
	favorite: FavoriteGif,
	target: Option<bridge::LightboxImage>,
	env: &Rc<RowEnv>,
) -> View {
	let hovered = env.pictures.runtime().signal(false);
	let starred = Rc::new(Cell::new(false));
	let (tracker, pictures, watcher, pressed) = (
		hovered.clone(),
		env.pictures.clone(),
		env.clone(),
		starred.clone(),
	);
	let frames = env.clone();
	let mut frame = frame
		.relative()
		.cursor(Cursor::Pointer)
		.on_mount(move |node| {
			let mut known = frames.gif_frames.borrow_mut();
			known.retain(|handle| {
				handle
					.scene()
					.is_some_and(|scene| scene.borrow().contains(handle.node()))
			});
			known.push(node);
		})
		.on_event(move |event| match event.event {
			InputEvent::PointerEnter => {
				tracker.set(true);
				pictures.hover(name.as_ref());
				watcher.want_favorites();
			}
			InputEvent::PointerLeave => {
				tracker.set(false);
				pictures.hover(None);
			}
			InputEvent::PointerDown { .. } if event.phase == EventPhase::Capture => pressed.set(false),
			_ => {}
		});
	if let Some(target) = target {
		let (lightbox, starred) = (env.lightbox.clone(), starred.clone());
		frame = frame.on_click(move || {
			if !starred.replace(false) {
				lightbox.open(target.clone());
			}
		});
	}
	if !env.prefs.get().gif_autoplay {
		let shown = hovered.clone();
		frame = frame.child_when(move || (!shown.get()).then_some(()), |(), _| gif_pill());
	}
	let env = env.clone();
	frame.child_when(
		move || hovered.get().then_some(()),
		move |(), _| star(&favorite, &starred, &env),
	)
}

fn gif_pill() -> View {
	div()
		.absolute()
		.left(0.)
		.top(0.)
		.w_full()
		.h_full()
		.bg(tok("overlay_button_background"))
		.child(
			div()
				.absolute()
				.left(OVERLAY_INSET)
				.top(OVERLAY_INSET)
				.h(tok("tag_height"))
				.px(OVERLAY_INSET)
				.rounded(tok("radius_xs"))
				.bg(tok("overlay_button_background"))
				.child(
					text("GIF")
						.text_size(tok("font_size_tag"))
						.line_height(16.)
						.font_weight(700)
						.text_color(tok("overlay_text")),
				),
		)
}

fn star(favorite: &FavoriteGif, starred: &Rc<Cell<bool>>, env: &Rc<RowEnv>) -> View {
	let favorites = env.favorite_gifs.clone();
	let (marked, marked_url) = (favorites.clone(), favorite.url.clone());
	let (url, gif, sender, pressed) = (
		favorite.url.clone(),
		favorite.clone(),
		env.clone(),
		starred.clone(),
	);
	div()
		.id(format!("gif-favorite-{}", favorite.url))
		.absolute()
		.right(OVERLAY_INSET)
		.top(OVERLAY_INSET)
		.size(STAR_BUTTON, STAR_BUTTON)
		.rounded(tok("radius_xs"))
		.bg(tok("overlay_button_background"))
		.items_center()
		.justify_center()
		.cursor(Cursor::Pointer)
		.on_event(move |event| {
			if event.phase != EventPhase::Capture
				&& let InputEvent::PointerDown {
					button: PointerButton::Primary,
					..
				} = event.event
			{
				pressed.set(true);
			}
		})
		.on_click(move || {
			let favorite = !favorites.with_untracked(|set| set.contains(&url));
			sender.send(Command::SetGifFavorite {
				gif: gif.clone(),
				favorite,
			});
		})
		.child(switch(
			move || marked.with(|set| set.contains(&marked_url)),
			|starred: bool, _| {
				icon(
					if starred { "star_filled" } else { "star" },
					STAR_ICON,
					"overlay_text",
				)
			},
		))
}

fn opens(frame: View, target: bridge::LightboxImage, env: &Rc<RowEnv>) -> View {
	let lightbox = env.lightbox.clone();
	frame
		.cursor(Cursor::Pointer)
		.on_click(move || lightbox.open(target.clone()))
}

fn file_card(attachment: &Attachment, env: &Rc<RowEnv>) -> View {
	let url = attachment.url.clone().unwrap_or_default();
	let size = attachment
		.size
		.map(|bytes| bridge::file_size(bytes.max(0) as u64));
	let glyph = if bridge::is_video(attachment) {
		"play"
	} else {
		"file"
	};
	let (name_links, button_links) = (env.links.clone(), env.links.clone());
	let (name_url, button_url) = (url.clone(), url);
	row()
		.mt(4.)
		.items_center()
		.gap(12.)
		.w(FILE_CARD_WIDTH)
		.max_w_percent(100.)
		.p(12.)
		.rounded(tok("radius_md"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_3"))
		.child(icon(glyph, 32., "text_4"))
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					text(attachment.filename.as_str())
						.truncate()
						.type_size("font_md")
						.text_color(tok("link"))
						.cursor(Cursor::Pointer)
						.on_click(move || name_links.download(&name_url)),
				)
				.when_some(size, |column, size| {
					column.child(text(size).type_size("font_tiny").text_color(tok("text_muted")))
				}),
		)
		.child(
			div()
				.size(32., 32.)
				.items_center()
				.justify_center()
				.shrink_0()
				.cursor(Cursor::Pointer)
				.on_click(move || button_links.download(&button_url))
				.child(icon("download", 24., "interactive_normal")),
		)
}

pub fn stickers(entry: &MessageEntry, env: &Rc<RowEnv>) -> Vec<View> {
	entry
		.message
		.sticker_items
		.iter()
		.filter_map(|sticker| {
			let name = env.pictures.sticker(sticker)?;
			let view = div().mt(4.).size(STICKER_SIDE, STICKER_SIDE).child(picture(
				&env.pictures,
				Some(name),
				None,
				ObjectFit::Contain,
			));
			let target = bridge::sticker_expression(sticker);
			let view = match target.clone() {
				Some(target) => {
					popout::opens(view, bridge::PopoutSubject::Custom(target), &env.actions.popout)
				}
				None => view,
			};
			Some(aimed(view, &env.actions, target))
		})
		.collect()
}

pub fn embeds(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>) -> Vec<View> {
	entry
		.message
		.embeds
		.iter()
		.enumerate()
		.filter(|(_, embed)| {
			env.prefs.get().link_previews
				|| matches!(embed.kind, None | Some(EmbedKind::Rich | EmbedKind::PollResult))
		})
		.filter_map(|(index, embed)| embed_card(entry, index, embed, env, bump))
		.collect()
}

fn bare(embed: &Embed, media: &EmbedMedia, env: &Rc<RowEnv>) -> View {
	let (width, height) = bridge::fit_box(media.width, media.height, bare_box(embed));
	let name = env.pictures.embed_media(media, (width, height));
	let target = bridge::LightboxImage::embed(media);
	let frame = div()
		.mt(4.)
		.size(width, height)
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.child(picture(
			&env.pictures,
			name.clone(),
			media.placeholder.as_deref(),
			ObjectFit::Cover,
		));
	match bridge::favorite_gif(embed) {
		Some(favorite) => gif(frame, name, favorite, target, env),
		None => frame.when_some(target, |frame, target| opens(frame, target, env)),
	}
}

fn link_label(label: String, url: Option<String>, env: &Rc<RowEnv>) -> View {
	let label = text(label)
		.text_size(tok("font_size_md"))
		.line_height(tok("line_height_label"))
		.font_weight(600);
	match url {
		Some(url) => {
			let links = env.links.clone();
			label
				.text_color(tok("link"))
				.cursor(Cursor::Pointer)
				.on_click(move || links.open(&url))
		}
		None => label.text_color(tok("text_2")),
	}
}

fn embed_card(
	entry: &MessageEntry,
	index: usize,
	embed: &Embed,
	env: &Rc<RowEnv>,
	bump: &Signal<u32>,
) -> Option<View> {
	if matches!(embed.kind, Some(EmbedKind::Image | EmbedKind::Gifv)) {
		return bare_media(embed).map(|media| bare(embed, media, env));
	}
	let accent = embed
		.color
		.and_then(|color| u32::try_from(color).ok())
		.map_or(zgx::theme::Themed::Token(tok("border")), |color| {
			zgx::theme::Themed::Value(rgb(color))
		});
	let texts = entry.body.embeds.get(index);
	let mut column = column().grow().min_w(0.).gap(4.);
	if let Some(provider) = embed.provider.as_ref().and_then(|provider| provider.name.clone()) {
		column = column.child(
			text(provider)
				.text_size(tok("font_size_xs"))
				.line_height(16.)
				.text_color(tok("text_4")),
		);
	}
	if let Some(author) = &embed.author {
		let icon_url = author.proxy_icon_url.as_deref().or(author.icon_url.as_deref());
		let picture_view = icon_url.map(|url| {
			let name = env.pictures.embed_icon(url);
			div()
				.size(EMBED_ICON as f32, EMBED_ICON as f32)
				.shrink_0()
				.rounded(EMBED_ICON as f32 / 2.)
				.overflow_hidden()
				.child(picture(&env.pictures, Some(name), None, ObjectFit::Cover))
		});
		column = column.child(
			row()
				.items_center()
				.gap(8.)
				.when_some(picture_view, |row, view| row.child(view))
				.child(link_label(author.name.clone(), author.url.clone(), env)),
		);
	}
	if let Some(title) = &embed.title {
		column = column.child(link_label(title.clone(), embed.url.clone(), env));
	}
	let spoilers = (index + 1) * SPOILER_STRIDE;
	if let Some(description) = texts.and_then(|texts| texts.description.as_ref()) {
		column = column.child(body::render(
			description,
			env,
			&entry.names,
			entry.id,
			spoilers,
			bump,
			Look::message(14.),
		));
	}
	if !embed.fields.is_empty() {
		column = column.child(fields(
			entry,
			embed,
			texts,
			env,
			bump,
			spoilers + SPOILER_STRIDE / 2,
		));
	}
	if let Some(media) = large_media(embed)
		&& (media.proxy_url.is_some() || media.url.is_some())
	{
		column = column.child(large(media, embed, env));
	}
	if embed.footer.is_some() || embed.timestamp.is_some() {
		let footer = embed.footer.as_ref().map(|footer| footer.text.clone());
		let stamp = embed
			.timestamp
			.map(|time| bridge::message_time(time, bridge::today()));
		let line = [footer, stamp]
			.into_iter()
			.flatten()
			.collect::<Vec<_>>()
			.join(" \u{2022} ");
		column = column.child(
			text(line)
				.text_size(tok("font_size_xs"))
				.line_height(16.)
				.text_color(tok("text_4")),
		);
	}
	let thumbnail = side_thumbnail(embed).map(|media| {
		let name = env
			.pictures
			.embed_media(media, (EMBED_THUMBNAIL, EMBED_THUMBNAIL));
		div()
			.size(EMBED_THUMBNAIL, EMBED_THUMBNAIL)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.child(picture(
				&env.pictures,
				name,
				media.placeholder.as_deref(),
				ObjectFit::Cover,
			))
	});
	Some(
		row()
			.relative()
			.my(2.)
			.w(EMBED_WIDTH)
			.max_w_percent(100.)
			.rounded(tok("radius_md"))
			.overflow_hidden()
			.border(tok("border_thickness"))
			.border_color(tok("border"))
			.bg(tok("bg_3"))
			.child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.bottom(0.)
					.w(EMBED_ACCENT)
					.bg(accent),
			)
			.child(
				row()
					.grow()
					.min_w(0.)
					.gap(16.)
					.p(EMBED_PADDING)
					.child(column)
					.when_some(thumbnail, |row, thumbnail| row.child(thumbnail)),
			),
	)
}

fn fields(
	entry: &MessageEntry,
	embed: &Embed,
	texts: Option<&bridge::EmbedText>,
	env: &Rc<RowEnv>,
	bump: &Signal<u32>,
	spoilers: usize,
) -> View {
	row()
		.w_full()
		.flex_wrap()
		.gap(8.)
		.children(embed.fields.iter().enumerate().map(|(index, field)| {
			let value = texts.and_then(|texts| texts.fields.get(index)).map(|document| {
				body::render(
					document,
					env,
					&entry.names,
					entry.id,
					spoilers + index * 10,
					bump,
					Look::message(14.),
				)
			});
			let cell = column().min_w(0.);
			let cell = if field.inline {
				cell.w(FIELD_WIDTH)
			} else {
				cell.w_full()
			};
			cell.child(
				text(field.name.as_str())
					.type_size("font_sm")
					.font_weight(600)
					.text_color(tok("header_primary")),
			)
			.when_some(value, |cell, value| cell.child(value))
		}))
}

fn large(media: &EmbedMedia, embed: &Embed, env: &Rc<RowEnv>) -> View {
	let (width, height) = bridge::fit_box(media.width, media.height, EMBED_MEDIA_BOX);
	let name = env.pictures.embed_media(media, (width, height));
	let playable = embed.video.is_some();
	let target = embed
		.url
		.clone()
		.or_else(|| embed.video.as_ref().and_then(|video| video.url.clone()))
		.filter(|_| playable);
	let links = env.links.clone();
	let target_image = bridge::LightboxImage::embed(media).filter(|_| !playable);
	div()
		.relative()
		.mt(EMBED_IMAGE_GAP)
		.size(width, height)
		.rounded(tok("radius_sm"))
		.overflow_hidden()
		.when_some(target_image, |frame, target| opens(frame, target, env))
		.child(picture(
			&env.pictures,
			name,
			media.placeholder.as_deref(),
			ObjectFit::Cover,
		))
		.when_some(target, |frame, url| {
			frame
				.cursor(Cursor::Pointer)
				.on_click(move || links.open(&url))
				.child(
					div()
						.absolute()
						.left(0.)
						.top(0.)
						.w_full()
						.h_full()
						.items_center()
						.justify_center()
						.child(
							div()
								.size(PLAY_DISC, PLAY_DISC)
								.rounded(PLAY_DISC / 2.)
								.bg(tok("overlay"))
								.items_center()
								.justify_center()
								.child(icon("play", 24., "text_on_brand")),
						),
				)
		})
}
