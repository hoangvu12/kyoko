use std::rc::Rc;
use std::sync::Arc;

use bridge::{ATTACHMENT_BOX, EMBED_MEDIA_BOX, EMBED_THUMBNAIL, MessageEntry, STICKER_SIDE};
use media::ImageName;
use protocol::{Attachment, Embed, EmbedKind, EmbedMedia};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::reactive::Signal;

use super::actions::aimed;
use super::body::{self, Look};
use super::icons::icon;
use super::pictures::{EMBED_ICON, Pictures, bare_media, large_media, side_thumbnail};
use super::rows::{RowEnv, TypeSize, tok};

const EMBED_WIDTH: f32 = 520.;
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
	div()
		.mt(4.)
		.size(width, height)
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.when_some(target, |frame, target| opens(frame, target, env))
		.child(picture(
			&env.pictures,
			name,
			attachment.placeholder.as_deref(),
			ObjectFit::Cover,
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
		.border(1.)
		.border_color(tok("code_border"))
		.bg(tok("background_secondary"))
		.child(icon(glyph, 32., "interactive_normal"))
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
			Some(aimed(view, &env.actions, bridge::sticker_expression(sticker)))
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

fn bare(media: &EmbedMedia, env: &Rc<RowEnv>) -> View {
	let (width, height) = bridge::fit_box(media.width, media.height, ATTACHMENT_BOX);
	let name = env.pictures.embed_media(media, (width, height));
	let target = bridge::LightboxImage::embed(media);
	div()
		.mt(4.)
		.size(width, height)
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.when_some(target, |frame, target| opens(frame, target, env))
		.child(picture(
			&env.pictures,
			name,
			media.placeholder.as_deref(),
			ObjectFit::Cover,
		))
}

fn link_label(label: String, url: Option<String>, env: &Rc<RowEnv>) -> View {
	let label = text(label).type_size("font_md").font_weight(600);
	match url {
		Some(url) => {
			let links = env.links.clone();
			label
				.text_color(tok("link"))
				.cursor(Cursor::Pointer)
				.on_click(move || links.open(&url))
		}
		None => label.text_color(tok("header_primary")),
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
		return bare_media(embed).map(|media| bare(media, env));
	}
	let accent = embed
		.color
		.and_then(|color| u32::try_from(color).ok())
		.map_or(zgx::theme::Themed::Token(tok("divider")), |color| {
			zgx::theme::Themed::Value(rgb(color))
		});
	let texts = entry.body.embeds.get(index);
	let mut column = column().grow().min_w(0.).gap(4.);
	if let Some(provider) = embed.provider.as_ref().and_then(|provider| provider.name.clone()) {
		column = column.child(
			text(provider)
				.type_size("font_tiny")
				.text_color(tok("text_muted")),
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
		column = column.child(text(line).type_size("font_tiny").text_color(tok("text_muted")));
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
			.mt(4.)
			.w(EMBED_WIDTH)
			.max_w_percent(100.)
			.rounded_corners(Corners {
				top_left: 0.,
				top_right: 4.,
				bottom_right: 4.,
				bottom_left: 0.,
			})
			.border_edges(zgx::scene::Insets {
				left: 4.,
				..Default::default()
			})
			.border_color(accent)
			.bg(tok("background_secondary"))
			.child(
				row()
					.grow()
					.min_w(0.)
					.gap(16.)
					.py(8.)
					.pl(12.)
					.pr(16.)
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
		.mt(4.)
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
