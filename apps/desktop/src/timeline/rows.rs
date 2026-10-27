use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use std::f32::consts::FRAC_PI_2;

use bridge::{
	EMBED_MEDIA_BOX, Highlight, MessageEntry, ReplyPreview, ReplyState, Row, STICKER_SIDE, SystemIcon,
};
use chrono::NaiveDate;
use media::cdn::Quality;
use protocol::{MessageFlags, MessageType, Snowflake};
use session::Command;
use zgx::canvas::GradientStop;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::style::ObjectFit;
use zgx::theme::{Themed, TokenId};

use super::actions::MessageActions;
use super::attachments;
use super::body::{self, Look};
use super::decorations;
use super::extras;
use super::icons::icon;
use super::kept;
use super::links::Links;
use super::pictures::{AVATAR, Pictures, REPLY_AVATAR};
use super::uploads;

const GUTTER: f32 = 72.;
const RIGHT: f32 = 48.;
const ROW_INSET_RIGHT: f32 = 8.;
const LINE: f32 = 22.;
const BODY: f32 = 16.;
const HEADER_GAP: f32 = 17.;
const REPLY_LINE: f32 = 18.;
const REPLY_GAP: f32 = 4.;
const REPLY_HEIGHT: f32 = REPLY_LINE + REPLY_GAP;
const AVATAR_LEFT: f32 = 16.;
const AVATAR_TOP: f32 = 4.;
const SPINE: f32 = 2.;
const HIGHLIGHT_LEFT: f32 = 4.;
const HIGHLIGHT_INSET: f32 = 8.;
const HIGHLIGHT_BAR: f32 = 4.;
const CLOCK_WIDTH: f32 = 56.;
const DAY_HEIGHT: f32 = 32.;
const UNREAD_HEIGHT: f32 = 10.;
const UNREAD_TOP: f32 = 6.;
const UNREAD_LEFT: f32 = 12.;
const UNREAD_RIGHT: f32 = 10.;
const WASH_STOP: f32 = 0.4;
const START_HEIGHT: f32 = 202.;
const SKELETON_HEIGHT: f32 = 240.;
const SKELETON_ROWS: usize = 4;
const FAILED_HEIGHT: f32 = 96.;
const AVERAGE_GLYPH: f32 = 7.5;
const EMBED_TEXT: f32 = 120.;
const POLL_CHROME: f32 = 120.;
const POLL_ROW: f32 = 52.;
const POLL_RESULT_BUTTON: f32 = 36.;
const REACTIONS: f32 = 28.;
const FILE_CARD: f32 = 70.;
const VOICE_MESSAGE: f32 = 52.;
const AUDIO_CARD: f32 = 118.;
const FOOTER_LINE: f32 = 16.;
const FOOTER_GAP: f32 = 2.;
const STATUS_ICON: f32 = 16.;
const FAILED_ICON: f32 = 12.;
const DOT: f32 = 6.;
const DOT_SHADES: [f32; 3] = [0.4, 0.7, 1.];
const SENDING_OPACITY: f32 = 0.5;

const DEFAULT_LINE: f32 = 1.618;

pub trait TypeSize {
	fn type_size(self, name: &str) -> Self;
}

impl TypeSize for View {
	fn type_size(self, name: &str) -> Self {
		let points = match name {
			"font_tiny" => 12.,
			"font_xs" => 13.,
			"font_sm" => 14.,
			"font_lg" => 20.,
			"font_xl" => 24.,
			_ => 16.,
		};
		self.text_size(tok(name)).line_height(points * DEFAULT_LINE)
	}
}

pub fn tok(name: &str) -> TokenId {
	zgx::theme::token(&crate::theme::token(name))
}

fn color_or(color: Option<u32>, fallback: &str) -> Themed<Color> {
	match color {
		Some(color) => Themed::Value(rgb(color)),
		None => Themed::Token(tok(fallback)),
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prefs {
	pub inline_media: bool,
	pub link_previews: bool,
	pub show_reactions: bool,
	pub quality: Quality,
	pub link_warning: bool,
	pub gif_autoplay: bool,
}

impl Prefs {
	pub fn of(settings: &store::Settings) -> Self {
		Self {
			inline_media: settings.chat.inline_media,
			link_previews: settings.chat.link_previews,
			show_reactions: settings.chat.show_reactions,
			quality: bridge::media_quality(settings),
			link_warning: settings.chat.link_warning,
			gif_autoplay: settings.media.gif_autoplay,
		}
	}
}

impl Default for Prefs {
	fn default() -> Self {
		Self {
			inline_media: true,
			link_previews: true,
			show_reactions: true,
			quality: Quality::Fitted,
			link_warning: true,
			gif_autoplay: true,
		}
	}
}

pub type Sender = RefCell<Option<Box<dyn Fn(Command)>>>;

pub struct RowEnv {
	pub pictures: Rc<Pictures>,
	pub links: Rc<Links>,
	pub channel_name: Signal<String>,
	pub code_font: String,
	pub prefs: Cell<Prefs>,
	pub revealed: RefCell<HashSet<(Snowflake, usize)>>,
	pub opened: RefCell<HashSet<Snowflake>>,
	pub picks: RefCell<HashMap<Snowflake, Vec<i64>>>,
	pub peeks: RefCell<HashSet<Snowflake>>,
	pub commands: Rc<Sender>,
	pub actions: Rc<MessageActions>,
	pub lightbox: Rc<super::lightbox::Lightbox>,
	pub display: bridge::Display,
	pub favorite_gifs: Signal<Rc<HashSet<String>>>,
	pub favorites_asked: Cell<bool>,
	pub gif_frames: RefCell<Vec<zgx::compose::NodeHandle>>,
	pub selects: RefCell<HashMap<(Snowflake, String), Rc<super::components::SelectState>>>,
}

impl RowEnv {
	pub fn want_favorites(&self) {
		if !self.favorites_asked.replace(true) {
			self.send(Command::LoadFavorites);
		}
	}

	pub fn pick_in_select(&self, message_id: Snowflake, custom_id: &str, values: &[&str], open: bool) {
		let key = (message_id, custom_id.to_owned());
		if let Some(state) = self.selects.borrow().get(&key) {
			state.pick(values.iter().map(|value| (*value).to_owned()).collect(), open);
		}
	}

	pub fn send(&self, command: Command) {
		if let Some(send) = self.commands.borrow().as_ref() {
			send(command);
		}
	}
}

pub fn render(row: &Row, env: &Rc<RowEnv>, hovered: &Signal<bool>, bump: &Signal<u32>) -> View {
	match row {
		Row::Loading => skeleton(),
		Row::Failed { channel_id, reason } => history_failed(*channel_id, reason, env),
		Row::Start => welcome(env),
		Row::Day(day) => day_separator(*day),
		Row::Unread => unread_marker(),
		Row::Message(entry) if entry.is_plain() && entry.grouped => {
			inset(follow_up(entry, env, hovered, bump))
		}
		Row::Message(entry) if entry.is_plain() => inset(first_of_group(entry, env, hovered, bump)),
		Row::Message(entry) if entry.kind == MessageType::PollResult => inset(super::polls::result_row_view(
			entry, env, GUTTER, RIGHT, HEADER_GAP,
		)),
		Row::Message(entry) => inset(system_row(entry, env)),
	}
}

fn inset(row: View) -> View {
	div().w_full().pr(ROW_INSET_RIGHT).child(row)
}

pub fn estimate(row: &Row, width: f32) -> f32 {
	match row {
		Row::Loading => SKELETON_HEIGHT,
		Row::Failed { .. } => FAILED_HEIGHT,
		Row::Start => START_HEIGHT,
		Row::Day(_) => DAY_HEIGHT,
		Row::Unread => UNREAD_HEIGHT,
		Row::Message(entry) if entry.kind == MessageType::PollResult => {
			HEADER_GAP + LINE + POLL_RESULT_BUTTON + 4.
		}
		Row::Message(entry) => {
			let header = if entry.grouped && entry.is_plain() {
				0.
			} else {
				HEADER_GAP + LINE
			};
			let reply = if entry.reply.is_some() || entry.interaction.is_some() {
				REPLY_HEIGHT
			} else {
				0.
			};
			let status = if entry.reply_state.is_some() { LINE } else { 0. };
			let footer = if entry.ephemeral {
				FOOTER_LINE + FOOTER_GAP
			} else {
				0.
			};
			let history = entry.history.len() as f32 * LINE;
			header
				+ reply + status
				+ footer + history
				+ body_lines(&entry.content, width - GUTTER - RIGHT) * LINE
				+ rich_height(entry)
				+ uploads::height(entry)
				+ 4.
		}
	}
}

fn rich_height(entry: &MessageEntry) -> f32 {
	let message = &entry.message;
	let mosaic = bridge::image_mosaic(&bridge::mosaic_images(&message.attachments));
	let grid = if mosaic.is_empty() { 0. } else { mosaic.height + 4. };
	let attachments: f32 = grid
		+ message
			.attachments
			.iter()
			.map(|attachment| {
				if bridge::is_image(attachment) {
					0.
				} else if bridge::is_voice_message(message, attachment) {
					VOICE_MESSAGE
				} else if bridge::is_audio(attachment) {
					AUDIO_CARD
				} else {
					FILE_CARD
				}
			})
			.sum::<f32>();
	let embeds: f32 = message
		.embeds
		.iter()
		.map(|embed| {
			let media = super::pictures::large_media(embed).map_or(0., |media| {
				bridge::fit_box(media.width, media.height, EMBED_MEDIA_BOX).1
			});
			EMBED_TEXT + media
		})
		.sum();
	let stickers = message.sticker_items.len() as f32 * (STICKER_SIDE + 4.);
	let poll = message
		.poll
		.as_ref()
		.map_or(0., |poll| POLL_CHROME + poll.answers.len() as f32 * POLL_ROW);
	let reactions = if message.reactions.is_empty() {
		0.
	} else {
		REACTIONS
	};
	attachments + embeds + stickers + poll + reactions + super::components::height(entry)
}

fn body_lines(content: &str, width: f32) -> f32 {
	if content.is_empty() {
		return 0.;
	}
	let per_line = (width.max(AVERAGE_GLYPH) / AVERAGE_GLYPH).floor().max(1.);
	content
		.lines()
		.map(|line| (line.chars().count() as f32 / per_line).ceil().max(1.))
		.sum()
}

fn wash(env: &RowEnv, name: &str) -> Vec<GradientStop> {
	let color = zgx::theme::Themes::of(env.pictures.runtime())
		.get(tok(name))
		.color()
		.unwrap_or(Color(0, 0, 0, 0));
	let Color(red, green, blue, _) = color;
	vec![
		GradientStop { offset: 0., color },
		GradientStop {
			offset: WASH_STOP,
			color,
		},
		GradientStop {
			offset: 1.,
			color: Color(red, green, blue, 0),
		},
	]
}

fn highlight_tokens(highlight: Highlight) -> Option<(&'static str, &'static str, &'static str)> {
	match highlight {
		Highlight::Mention => Some(("mention", "mention_hover", "accent_2")),
		Highlight::ReplyToYou => Some(("reply", "reply_hover", "text_2")),
		Highlight::None => None,
	}
}

fn message_frame(entry: &MessageEntry, env: &Rc<RowEnv>, hovered: &Signal<bool>) -> View {
	let actions = env.actions.clone();
	let id = entry.id;
	let tinted = kept::tinted(entry);
	let highlight = highlight_tokens(if entry.ephemeral {
		Highlight::Mention
	} else {
		entry.highlight
	});
	let (washed, over) = (env.clone(), hovered.clone());
	let frame = column()
		.relative()
		.w_full()
		.pl(GUTTER)
		.pr(RIGHT)
		.py(2.)
		.rounded(tok("radius_md"))
		.reactive_style(move || {
			let hovered = over.get();
			if actions.is_selected(id) {
				return Styles::new().bg(tok("message_selected"));
			}
			if tinted {
				return Styles::new().bg(tok("background_feedback_critical"));
			}
			match highlight {
				Some((rest, lit, _)) => {
					Styles::new().bg_gradient(FRAC_PI_2, wash(&washed, if hovered { lit } else { rest }))
				}
				None if hovered => Styles::new().bg(tok("message_hover")),
				None => Styles::new(),
			}
		})
		.when_some(highlight, |frame, (_, _, bar)| {
			frame.child(
				div()
					.absolute()
					.left(HIGHLIGHT_LEFT)
					.top(HIGHLIGHT_INSET)
					.bottom(HIGHLIGHT_INSET)
					.w(HIGHLIGHT_BAR)
					.rounded(HIGHLIGHT_BAR)
					.bg(tok(bar)),
			)
		})
		.when(
			entry.pending && entry.upload.is_none() && entry.reply_state.is_none(),
			|row| row.opacity(0.5),
		);
	if !bridge::has_actions(entry) || entry.deleted {
		return frame;
	}
	let shared = Rc::new(entry.clone());
	let more = env.pictures.runtime().signal(false);
	let (hovered, open, actions) = (hovered.clone(), more.clone(), env.actions.clone());
	let menus = env.actions.clone();
	frame.child_when(
		move || ((hovered.get() || open.get()) && !menus.has_menu_open(id)).then_some(()),
		move |(), _| actions.toolbar(&shared, more.clone()),
	)
}

fn content(entry: &MessageEntry, env: &Rc<RowEnv>, hovered: &Signal<bool>, bump: &Signal<u32>) -> Vec<View> {
	let mut out = kept::history(entry, env, bump, BODY);
	out.extend(reply_status(entry));
	if !entry.content.is_empty() && !super::pictures::only_link(entry) {
		let look = Look {
			size: BODY,
			color: kept::text_color(entry),
			jumbo: entry.body.jumbo,
			trailer: kept::trailer(entry),
			on_trailer: kept::history_toggle(entry, env, bump),
		};
		out.push(
			body::render(&entry.body.content, env, &entry.names, entry.id, 0, bump, look)
				.id(format!("message-text-{}", entry.id))
				.when(entry.pending && !entry.failed && entry.upload.is_some(), |body| {
					body.opacity(0.5)
				}),
		);
	} else if entry.deleted {
		out.push(
			text("(deleted)")
				.text_size(tok("font_size_sm"))
				.text_color(tok("red_2")),
		);
	}
	out.extend(attachments::attachments(entry, env));
	out.extend(uploads::upload(entry, env));
	if !entry.message.flags.contains(MessageFlags::SUPPRESS_EMBEDS) {
		out.extend(attachments::embeds(entry, env, bump));
	}
	out.extend(super::components::components(entry, env, bump));
	out.extend(attachments::stickers(entry, env));
	out.extend(extras::thread_chip(entry, env));
	out.extend(super::polls::poll(entry, env, bump));
	out.extend(super::reactions::chips(entry, env, hovered));
	out.extend(kept::accessories(entry, env));
	out.extend(interaction_failed(entry));
	out.extend(ephemeral_footer(entry, env));
	out
}

fn reply_status(entry: &MessageEntry) -> Option<View> {
	let status = |label: String| {
		text(label)
			.text_size(tok("font_size_md"))
			.line_height(LINE)
			.text_color(tok("text_4"))
	};
	Some(match entry.reply_state.as_ref()? {
		ReplyState::Sending => status("Sending command...".to_owned()).opacity(SENDING_OPACITY),
		ReplyState::Thinking => row()
			.items_center()
			.gap(8.)
			.h(LINE)
			.child(thinking_dots())
			.child(status(format!("{} is thinking...", entry.author))),
		failed => row()
			.items_center()
			.gap(6.)
			.h(LINE)
			.child(icon("warning_fill", STATUS_ICON, "red_2"))
			.child(
				text(failed.failure().unwrap_or_default().to_owned())
					.text_size(tok("font_size_md"))
					.line_height(LINE)
					.text_color(tok("red_2")),
			),
	})
}

fn thinking_dots() -> View {
	row()
		.shrink_0()
		.items_center()
		.gap(2.)
		.children(DOT_SHADES.iter().map(|shade| {
			div()
				.size(DOT, DOT)
				.rounded(DOT / 2.)
				.bg(tok("text_4"))
				.opacity(*shade)
		}))
}

fn interaction_failed(entry: &MessageEntry) -> Option<View> {
	entry.interaction_failed.then(|| {
		row()
			.items_center()
			.gap(4.)
			.mt(4.)
			.child(icon("warning_fill", FAILED_ICON, "red_2"))
			.child(
				text("This interaction failed")
					.text_size(tok("font_size_xs"))
					.line_height(FOOTER_LINE)
					.font_weight(500)
					.text_color(tok("red_2")),
			)
	})
}

fn ephemeral_footer(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	if !entry.ephemeral {
		return None;
	}
	let channel_id = entry.message.channel_id;
	let message_id = entry.id;
	let pending = entry.message.nonce.clone().filter(|_| entry.pending);
	let key = match &pending {
		Some(nonce) => nonce.0.clone(),
		None => message_id.to_string(),
	};
	let dismiss = move || match pending.clone() {
		Some(nonce) => Command::DiscardMessage { channel_id, nonce },
		None => Command::DismissMessage {
			channel_id,
			message_id,
		},
	};
	let small = |label: &str| {
		text(label.to_owned())
			.text_size(tok("font_size_xs"))
			.line_height(FOOTER_LINE)
			.text_color(tok("text_4"))
	};
	let env = env.clone();
	Some(
		row()
			.items_center()
			.gap(4.)
			.mt(FOOTER_GAP)
			.child(icon("eye", STATUS_ICON, "text_4"))
			.child(small("Only you can see this \u{b7}"))
			.child(
				small("Dismiss message")
					.id(format!("dismiss-message-{key}"))
					.text_color(tok("link"))
					.cursor(Cursor::Pointer)
					.on_click(move || env.send(dismiss())),
			),
	)
}

fn interaction_header(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	let line = entry.interaction.as_ref()?;
	let (actions, user) = (env.actions.clone(), line.user.id);
	let words = |label: String, weight: u16, color: Themed<Color>| {
		text(label)
			.shrink_0()
			.text_size(tok("font_size_sm"))
			.line_height(REPLY_LINE)
			.font_weight(weight)
			.text_color(color)
	};
	Some(
		row()
			.relative()
			.items_center()
			.gap(4.)
			.h(REPLY_LINE)
			.mb(REPLY_GAP)
			.child(spine())
			.child(small_avatar(
				&bridge::Avatar::of_user(&line.user),
				&line.name,
				env,
			))
			.child(
				words(line.name.clone(), 500, color_or(line.color, "text_2"))
					.cursor(Cursor::Pointer)
					.on_click(move || actions.profile(user)),
			)
			.child(words("used".to_owned(), 400, Themed::Token(tok("text_3"))))
			.child(words(
				format!("/{}", line.command),
				500,
				Themed::Token(tok("link")),
			)),
	)
}

fn spine() -> View {
	let avatar_middle = AVATAR_LEFT + AVATAR as f32 / 2.;
	div()
		.absolute()
		.left(avatar_middle - SPINE / 2. - GUTTER)
		.top(REPLY_LINE / 2.)
		.w(GUTTER - REPLY_GAP - avatar_middle + SPINE / 2.)
		.h(REPLY_LINE / 2. + 2.)
		.border_edges(zgx::scene::Insets {
			left: SPINE,
			top: SPINE,
			..Default::default()
		})
		.border_color(tok("text_5"))
		.rounded_corners(Corners {
			top_left: 6.,
			top_right: 0.,
			bottom_right: 0.,
			bottom_left: 0.,
		})
}

fn reply_header(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	let reply = entry.reply.as_ref()?;
	let muted = |label: &str| {
		text(label.to_owned())
			.text_size(tok("font_size_sm"))
			.italic(true)
			.text_color(tok("text_4"))
	};
	let inner = match reply {
		ReplyPreview::Unavailable => muted("Original message was deleted"),
		ReplyPreview::Loading => muted("Loading\u{2026}"),
		ReplyPreview::Message(quote) | ReplyPreview::Deleted(quote) => {
			let deleted = matches!(reply, ReplyPreview::Deleted(_));
			let snippet = if quote.attachment_only() {
				"Click to see attachment".to_owned()
			} else {
				quote.text.clone()
			};
			let target = entry
				.message
				.message_reference
				.as_ref()
				.and_then(|reference| reference.message_id);
			let actions = env.actions.clone();
			row()
				.items_center()
				.gap(4.)
				.min_w(0.)
				.when_some(target, move |preview, target| {
					preview
						.cursor(Cursor::Pointer)
						.on_click(move || actions.jump(target))
				})
				.child(small_avatar(&quote.avatar, &quote.name, env))
				.child(
					text(format!("@{}", quote.name))
						.shrink_0()
						.text_size(tok("font_size_sm"))
						.line_height(REPLY_LINE)
						.font_weight(500)
						.text_color(tok("text_2")),
				)
				.child(
					text(snippet)
						.grow()
						.min_w(0.)
						.truncate()
						.text_size(tok("font_size_sm"))
						.line_height(REPLY_LINE)
						.text_color(tok(if deleted { "red_2" } else { "text_3" })),
				)
		}
	};
	Some(
		row()
			.relative()
			.items_center()
			.gap(4.)
			.h(REPLY_LINE)
			.mb(REPLY_GAP)
			.child(spine())
			.child(inner),
	)
}

fn first_of_group(
	entry: &MessageEntry,
	env: &Rc<RowEnv>,
	hovered: &Signal<bool>,
	bump: &Signal<u32>,
) -> View {
	let time = bridge::message_time(entry.sent_at, bridge::today());
	let name_color = color_or(entry.author_color, "text_2");
	let reply = interaction_header(entry, env).or_else(|| reply_header(entry, env));
	let avatar_top = if reply.is_some() {
		AVATAR_TOP + REPLY_HEIGHT
	} else {
		AVATAR_TOP
	};
	let header = row()
		.items_center()
		.gap(4.)
		.h(LINE)
		.child({
			let (actions, author) = (env.actions.clone(), entry.message.author.id);
			text(entry.author.as_str())
				.text_size(tok("font_size_md"))
				.line_height(LINE)
				.font_weight(500)
				.text_color(name_color)
				.cursor(Cursor::Pointer)
				.on_click(move || actions.profile(author))
		})
		.when_some(entry.author_extra.clone(), |header, extra| {
			header.child(
				text(extra)
					.text_size(tok("font_size_md"))
					.line_height(LINE)
					.font_weight(500)
					.text_color(tok("text_4")),
			)
		})
		.when(entry.bot, |header| header.child(bot_tag()))
		.when_some(decorations::author_decorations(entry), |header, marks| {
			header.child(marks)
		})
		.child(
			text(time)
				.text_size(tok("font_size_xs"))
				.font_weight(500)
				.text_color(tok("text_5")),
		);
	message_frame(entry, env, hovered)
		.mt(HEADER_GAP)
		.when_some(reply, |row, reply| row.child(reply))
		.child({
			let (actions, author) = (env.actions.clone(), entry.message.author.id);
			avatar(entry, env, hovered)
				.absolute()
				.left(AVATAR_LEFT)
				.top(avatar_top)
				.cursor(Cursor::Pointer)
				.on_click(move || actions.profile(author))
		})
		.child(header)
		.children(content(entry, env, hovered, bump))
}

fn follow_up(entry: &MessageEntry, env: &Rc<RowEnv>, hovered: &Signal<bool>, bump: &Signal<u32>) -> View {
	let shown = hovered.clone();
	let clock = row()
		.absolute()
		.left(0.)
		.top(2.)
		.w(CLOCK_WIDTH)
		.h(LINE)
		.items_center()
		.justify_end()
		.child(
			text(bridge::clock_text(entry.sent_at))
				.text_size(tok("font_size_timestamp_alt"))
				.line_height(LINE)
				.text_color(tok("text_5")),
		)
		.reactive_style(move || {
			if shown.get() {
				Styles::new().visible()
			} else {
				Styles::new().invisible()
			}
		});
	message_frame(entry, env, hovered)
		.child(clock)
		.children(content(entry, env, hovered, bump))
}

fn system_icon(icon_kind: SystemIcon) -> (&'static str, &'static str) {
	match icon_kind {
		SystemIcon::Join => ("join", "success"),
		SystemIcon::Pin => ("pin", "interactive_normal"),
		SystemIcon::Thread => ("thread", "interactive_normal"),
		SystemIcon::Boost => ("nitro", "brand"),
		SystemIcon::Leave => ("close", "danger"),
		SystemIcon::Call => ("phone", "success"),
		SystemIcon::Rename => ("edit", "interactive_normal"),
		SystemIcon::Picture => ("image", "interactive_normal"),
		SystemIcon::Poll => ("poll", "text_4"),
		SystemIcon::Other => ("chat", "interactive_normal"),
	}
}

fn created_thread(entry: &MessageEntry) -> Option<Snowflake> {
	(entry.kind == MessageType::ThreadCreated)
		.then(|| entry.message.message_reference.as_ref()?.channel_id)
		.flatten()
}

fn system_row(entry: &MessageEntry, env: &Rc<RowEnv>) -> View {
	let Some(line) = entry.system.as_ref() else {
		return div();
	};
	let (glyph, color) = system_icon(line.icon);
	let actions = env.actions.clone();
	row()
		.when_some(created_thread(entry), move |row, thread_id| {
			row.cursor(Cursor::Pointer)
				.on_click(move || actions.open_thread(thread_id))
		})
		.relative()
		.w_full()
		.items_baseline()
		.gap(8.)
		.pl(GUTTER)
		.pr(RIGHT)
		.mt(HEADER_GAP)
		.py(2.)
		.rounded(tok("radius_md"))
		.hover(|style| style.bg(tok("message_hover")))
		.child(
			row()
				.absolute()
				.left(16.)
				.top(3.)
				.w(40.)
				.justify_end()
				.child(icon(glyph, 18., color)),
		)
		.child(
			text(line.text.as_str())
				.min_w(0.)
				.text_size(tok("font_size_md"))
				.line_height(LINE)
				.text_color(tok("text_4"))
				.text_wrap(true),
		)
		.child(
			text(bridge::message_time(entry.sent_at, bridge::today()))
				.shrink_0()
				.text_size(tok("font_size_xs"))
				.font_weight(500)
				.text_color(tok("text_5")),
		)
}

fn bot_tag() -> View {
	div()
		.h(tok("bot_tag_height"))
		.px(4.)
		.rounded(tok("radius_xs"))
		.bg(tok("accent_2"))
		.child(
			text("APP")
				.text_size(tok("font_size_tag"))
				.line_height(15.)
				.font_weight(600)
				.text_color(tok("text_0")),
		)
}

fn initial_of(name: &str) -> String {
	name.chars()
		.find(|character| character.is_alphanumeric())
		.map(|character| character.to_uppercase().collect())
		.unwrap_or_default()
}

fn small_avatar(avatar: &bridge::Avatar, name: &str, env: &Rc<RowEnv>) -> View {
	let size = REPLY_AVATAR as f32;
	let picture = env
		.pictures
		.picture(&env.pictures.member_avatar(avatar, REPLY_AVATAR, false));
	div()
		.size(size, size)
		.shrink_0()
		.rounded(size / 2.)
		.overflow_hidden()
		.bg(tok("brand"))
		.items_center()
		.justify_center()
		.child(
			text(initial_of(name))
				.text_size(size * 0.5)
				.font_weight(600)
				.text_color(tok("text_on_brand")),
		)
		.reactive_style(move || match picture.get() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
			None => Styles::new(),
		})
}

fn avatar(entry: &MessageEntry, env: &Rc<RowEnv>, hovered: &Signal<bool>) -> View {
	let size = AVATAR as f32;
	let fallback = color_or(entry.author_color, "brand");
	let initial = initial_of(&entry.author);
	let still = env
		.pictures
		.picture(&env.pictures.member_avatar(&entry.avatar, AVATAR, false));
	let moving = env
		.pictures
		.moving_avatar(&entry.avatar, AVATAR)
		.map(|name| env.pictures.picture(&name));
	let over = hovered.clone();
	let shown = Rc::new(move || {
		moving
			.as_ref()
			.filter(|_| over.get())
			.and_then(|moving| moving.get())
			.or_else(|| still.get())
	});
	let covered = shown.clone();
	div()
		.size(size, size)
		.rounded(size / 2.)
		.overflow_hidden()
		.bg(fallback)
		.items_center()
		.justify_center()
		.reactive_style(move || match shown() {
			Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
			None => Styles::new(),
		})
		.child(
			text(initial)
				.text_size(size * 0.42)
				.font_weight(600)
				.text_color(tok("text_on_brand"))
				.reactive_style(move || {
					if covered().is_some() {
						Styles::new().invisible()
					} else {
						Styles::new().visible()
					}
				}),
		)
}

fn rule(color: &str) -> View {
	div().grow().min_w(0.).h(1.).bg(tok(color))
}

fn day_separator(day: NaiveDate) -> View {
	row()
		.w_full()
		.items_center()
		.gap(8.)
		.pl(UNREAD_LEFT)
		.pr(UNREAD_RIGHT)
		.mt(16.)
		.child(rule("border"))
		.child(
			text(bridge::day_label(day))
				.text_size(tok("font_size_xs"))
				.line_height(16.)
				.font_weight(600)
				.text_color(tok("text_4")),
		)
		.child(rule("border"))
}

fn unread_marker() -> View {
	row()
		.w_full()
		.h(UNREAD_HEIGHT)
		.pt(UNREAD_TOP)
		.pl(UNREAD_LEFT)
		.pr(UNREAD_RIGHT)
		.child(
			div()
				.relative()
				.grow()
				.h(tok("divider_thickness"))
				.rounded(tok("divider_thickness"))
				.bg(tok("accent_new")),
		)
		.child(
			div()
				.shrink_0()
				.mt(-4.5)
				.h(tok("unread_pill_height"))
				.px(4.)
				.rounded(tok("radius_xs"))
				.bg(tok("accent_new"))
				.child(
					text("NEW")
						.text_size(tok("font_size_tag"))
						.line_height(13.)
						.font_weight(700)
						.text_color(tok("text_0")),
				),
		)
}

fn welcome(env: &Rc<RowEnv>) -> View {
	let title = env.channel_name.clone();
	let body = env.channel_name.clone();
	column()
		.gap(8.)
		.px(16.)
		.pt(32.)
		.pb(16.)
		.child(
			div()
				.size(68., 68.)
				.rounded(34.)
				.bg(tok("interactive_muted"))
				.items_center()
				.justify_center()
				.child(icon("hash", 42., "interactive_active")),
		)
		.child(
			text_signal(move || format!("Welcome to #{}!", title.get()))
				.text_size(32.)
				.line_height(40.)
				.font_weight(700)
				.text_color(tok("header_primary")),
		)
		.child(
			text_signal(move || format!("This is the start of the #{} channel.", body.get()))
				.text_size(tok("font_md"))
				.text_color(tok("text_muted")),
		)
}

fn history_failed(channel_id: Snowflake, reason: &str, env: &Rc<RowEnv>) -> View {
	let retry = env.clone();
	column()
		.id("history-failed")
		.gap(tok("space_8"))
		.px(16.)
		.py(24.)
		.items_center()
		.child(
			text(reason.to_owned())
				.text_size(tok("font_md"))
				.font_weight(500)
				.text_color(tok("red_2")),
		)
		.child(
			text("Retry")
				.text_size(tok("font_md"))
				.font_weight(500)
				.text_color(tok("link"))
				.cursor(Cursor::Pointer)
				.on_click(move || retry.send(Command::RetryHistory { channel_id })),
		)
}

fn skeleton() -> View {
	let line = |width: f32| div().h(12.).w(width).rounded(6.).bg(tok("interactive_muted"));
	column()
		.gap(16.)
		.p(16.)
		.opacity(0.6)
		.children((0..SKELETON_ROWS).map(move |index| {
			let index = index as f32;
			row()
				.gap(16.)
				.child(
					div()
						.size(AVATAR as f32, AVATAR as f32)
						.rounded(AVATAR as f32 / 2.)
						.bg(tok("interactive_muted")),
				)
				.child(
					column()
						.gap(8.)
						.child(line(90. + (index * 37.) % 80.))
						.child(line(240. + (index * 61.) % 220.))
						.when((index as usize).is_multiple_of(2), |lines| {
							lines.child(line(160.))
						}),
				)
		}))
}
