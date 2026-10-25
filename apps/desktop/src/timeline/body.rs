use std::cell::Cell;
use std::rc::Rc;

use chrono::Utc;
use markdown::highlight::{TokenKind, highlight};
use markdown::{Document, Node};
use protocol::Snowflake;
use zgx::compose::SpanHover;
use zgx::compose::prelude::*;
use zgx::reactive::Signal;
use zgx::scene::Color;
use zgx::theme::Themed;

use super::inline::{self, PLACEHOLDER, Paragraph, Placed};
use super::pictures::{EMOJI, JUMBO_EMOJI};
use super::rows::{RowEnv, tok};

const LINE_RATIO: f32 = 1.375;
const JUMBO_TEXT: f32 = 40.;
const CODE_LINE: f32 = 18.;
const SUBTEXT_LINE: f32 = 16.;
const LIST_INDENT: f32 = 16.;
const QUOTE_BAR: f32 = 4.;
const EMOJI_MARGIN: f32 = 1.;
const MENTION_WEIGHT: u16 = 500;
const ROLE_WASH: u8 = 51;

pub struct Look {
	pub size: f32,
	pub color: &'static str,
	pub jumbo: bool,
	pub trailer: Option<(&'static str, &'static str)>,
}

impl Look {
	pub fn message(size: f32) -> Self {
		Self {
			size,
			color: "text_normal",
			jumbo: false,
			trailer: None,
		}
	}
}

pub struct Body<'a> {
	env: &'a Rc<RowEnv>,
	names: &'a bridge::Names,
	message: Snowflake,
	bump: Signal<u32>,
	spoilers: Cell<usize>,
	look: Look,
}

#[derive(Clone)]
enum Click {
	Link(String),
	Spoiler(usize),
	Profile(Snowflake),
}

#[derive(Clone, Default)]
struct Style {
	weight: Option<u16>,
	italic: bool,
	underline: bool,
	strike: bool,
	color: Option<Themed<Color>>,
	background: Option<Themed<Color>>,
	mono: bool,
	click: Option<Click>,
}

#[derive(Default)]
struct Inline {
	spans: Vec<TextSpan>,
	length: usize,
	placed: Vec<Placed>,
}

impl Inline {
	fn is_empty(&self) -> bool {
		self.spans.is_empty()
	}
}

pub fn render(
	document: &Document,
	env: &Rc<RowEnv>,
	names: &bridge::Names,
	message: Snowflake,
	spoiler_base: usize,
	bump: &Signal<u32>,
	look: Look,
) -> View {
	let (size, line) = if look.jumbo {
		(JUMBO_TEXT, JUMBO_EMOJI as f32)
	} else {
		(look.size, look.size * LINE_RATIO)
	};
	let color = look.color;
	let body = Body {
		env,
		names,
		message,
		bump: bump.clone(),
		spoilers: Cell::new(spoiler_base),
		look,
	};
	column()
		.w_full()
		.min_w(0.)
		.text_size(size)
		.line_height(line)
		.text_color(tok(color))
		.children(body.blocks(&document.nodes, true))
}

impl Body<'_> {
	fn blocks(&self, nodes: &[Node], trailing: bool) -> Vec<View> {
		let mut out = Vec::new();
		let mut inline = Inline::default();
		for node in nodes {
			if node.is_block() {
				self.flush(&mut inline, &mut out);
				out.push(self.block(node));
			} else {
				self.inline(node, &Style::default(), &mut inline);
			}
		}
		if trailing && let Some((label, color)) = self.look.trailer {
			let muted = Style {
				color: Some(Themed::Token(tok(color))),
				..Style::default()
			};
			self.push(&mut inline, &format!(" {label}"), &muted);
		}
		self.flush(&mut inline, &mut out);
		out
	}

	fn flush(&self, inline: &mut Inline, out: &mut Vec<View>) {
		if !inline.is_empty() {
			out.push(self.paragraph(std::mem::take(inline)));
		}
	}

	fn paragraph(&self, inline: Inline) -> View {
		let side = self.emoji_side();
		inline::paragraph(
			Paragraph {
				spans: inline.spans,
				placed: inline.placed,
				side,
			},
			&self.env.pictures,
		)
	}

	fn inline_view(&self, nodes: &[Node]) -> View {
		let mut inline = Inline::default();
		for node in nodes {
			self.inline(node, &Style::default(), &mut inline);
		}
		self.paragraph(inline)
	}

	fn emoji_side(&self) -> f32 {
		if self.look.jumbo {
			JUMBO_EMOJI as f32
		} else {
			EMOJI as f32
		}
	}

	fn push(&self, inline: &mut Inline, text: &str, style: &Style) {
		if text.is_empty() {
			return;
		}
		inline.length += text.len();
		inline.spans.push(self.span(text, style));
	}

	fn span(&self, text: &str, style: &Style) -> TextSpan {
		let mut span = text_span(text.to_owned());
		if let Some(weight) = style.weight {
			span = span.font_weight(weight);
		}
		if style.italic {
			span = span.italic(true);
		}
		if style.underline {
			span = span.underline(Decoration::new(1.));
		}
		if style.strike {
			span = span.strikethrough(Decoration::new(1.));
		}
		if let Some(color) = style.color {
			span = span.text_color(color);
		}
		if let Some(background) = style.background {
			span = span.background(background);
		}
		if style.mono {
			span = span.font_family(self.env.code_font.as_str());
		}
		match style.click.clone() {
			Some(Click::Link(url)) => {
				let links = self.env.links.clone();
				span.hover(SpanHover {
					underline: Some(Decoration::new(1.)),
					text_color: None,
				})
				.on_click(move || links.open(&url))
			}
			Some(Click::Spoiler(index)) => {
				let (env, message, bump) = (self.env.clone(), self.message, self.bump.clone());
				span.hover(SpanHover::default()).on_click(move || {
					env.revealed.borrow_mut().insert((message, index));
					bump.update(|count| *count += 1);
				})
			}
			Some(Click::Profile(user)) => {
				let actions = self.env.actions.clone();
				span.hover(SpanHover::default())
					.on_click(move || actions.profile(user))
			}
			None => span,
		}
	}

	fn emoji(&self, inline: &mut Inline, id: u64, animated: bool) {
		let side = self.emoji_side();
		let name = self.env.pictures.emoji(id, animated, f64::from(side));
		let start = inline.length;
		inline.length += PLACEHOLDER.len();
		inline.placed.push(Placed {
			range: start..inline.length,
			name,
		});
		inline
			.spans
			.push(text_span(PLACEHOLDER).padding(side / 2. + EMOJI_MARGIN, 0.));
	}

	fn children(&self, nodes: &[Node], style: &Style, inline: &mut Inline) {
		for node in nodes {
			self.inline(node, style, inline);
		}
	}

	fn mention(&self, inline: &mut Inline, label: &str, color: Option<u32>, style: &Style) {
		let mut next = style.clone();
		next.weight = Some(MENTION_WEIGHT);
		match color {
			Some(color) => {
				next.color = Some(Themed::Value(rgb(color)));
				let [_, red, green, blue] = color.to_be_bytes();
				next.background = Some(Themed::Value(Color(red, green, blue, ROLE_WASH)));
			}
			None => {
				next.color = Some(Themed::Token(tok("mention_text")));
				next.background = Some(Themed::Token(tok("mention_background")));
			}
		}
		self.push(inline, label, &next);
	}

	fn inline(&self, node: &Node, style: &Style, inline: &mut Inline) {
		match node {
			Node::Text(text) => self.push(inline, text, style),
			Node::Bold(children) => {
				let next = Style {
					weight: Some(700),
					..style.clone()
				};
				self.children(children, &next, inline);
			}
			Node::Italic(children) => {
				let next = Style {
					italic: true,
					..style.clone()
				};
				self.children(children, &next, inline);
			}
			Node::Underline(children) => {
				let next = Style {
					underline: true,
					..style.clone()
				};
				self.children(children, &next, inline);
			}
			Node::Strike(children) => {
				let next = Style {
					strike: true,
					..style.clone()
				};
				self.children(children, &next, inline);
			}
			Node::Spoiler(children) => {
				let index = self.spoilers.get();
				self.spoilers.set(index + 1);
				let revealed = self.env.revealed.borrow().contains(&(self.message, index));
				let mut next = style.clone();
				if revealed {
					next.background = Some(Themed::Token(tok("modifier_active")));
				} else {
					next.color = Some(Themed::Token(tok("spoiler")));
					next.background = Some(Themed::Token(tok("spoiler")));
					next.click = Some(Click::Spoiler(index));
				}
				self.children(children, &next, inline);
			}
			Node::Code(code) => {
				let next = Style {
					background: Some(Themed::Token(tok("code_background"))),
					mono: true,
					..style.clone()
				};
				self.push(inline, code, &next);
			}
			Node::UserMention(id) => {
				let next = Style {
					click: Some(Click::Profile(Snowflake::from(*id))),
					..style.clone()
				};
				self.mention(inline, &self.names.user(*id), None, &next);
			}
			Node::RoleMention(id) => {
				let (label, color) = self.names.role(*id);
				self.mention(inline, &label, color, style);
			}
			Node::ChannelMention(id) => self.mention(inline, &self.names.channel(*id), None, style),
			Node::Everyone => self.mention(inline, "@everyone", None, style),
			Node::Here => self.mention(inline, "@here", None, style),
			Node::CommandMention { name, .. } => self.mention(inline, &format!("/{name}"), None, style),
			Node::GuildNav(kind) => self.mention(inline, bridge::guild_nav_label(*kind), None, style),
			Node::CustomEmoji { id, animated, .. } => self.emoji(inline, *id, *animated),
			Node::Timestamp { unix, style: kind } => {
				let label = bridge::timestamp_text(*unix, *kind, Utc::now());
				let next = Style {
					background: Some(Themed::Token(tok("code_background"))),
					..style.clone()
				};
				self.push(inline, &label, &next);
			}
			Node::Link { url, text, .. } => {
				let mut next = style.clone();
				next.color = Some(Themed::Token(tok("link")));
				if next.click.is_none() {
					next.click = Some(Click::Link(url.clone()));
				}
				match text {
					Some(children) => self.children(children, &next, inline),
					None => self.push(inline, url, &next),
				}
			}
			Node::LineBreak => self.push(inline, "\n", style),
			_ => {}
		}
	}

	fn block(&self, node: &Node) -> View {
		match node {
			Node::CodeBlock { lang, code } => self.code_block(lang.as_deref().unwrap_or_default(), code),
			Node::Heading { level, children } => {
				let (size, token) = match level {
					1 => (24., "font_xl"),
					2 => (20., "font_lg"),
					_ => (16., "font_md"),
				};
				div()
					.w_full()
					.mt(8.)
					.text_size(tok(token))
					.line_height(size * 1.25)
					.font_weight(700)
					.text_color(tok("header_primary"))
					.child(self.inline_view(children))
			}
			Node::Subtext(children) => div()
				.w_full()
				.text_size(tok("font_tiny"))
				.line_height(SUBTEXT_LINE)
				.text_color(tok("text_muted"))
				.child(self.inline_view(children)),
			Node::Quote(children) => row()
				.w_full()
				.gap(8.)
				.child(
					div()
						.w(QUOTE_BAR)
						.shrink_0()
						.rounded(2.)
						.bg(tok("interactive_muted")),
				)
				.child(column().grow().min_w(0.).children(self.blocks(children, false))),
			Node::List {
				ordered,
				start,
				items,
			} => column()
				.w_full()
				.pl(LIST_INDENT)
				.children(items.iter().enumerate().map(|(index, item)| {
					let marker = if *ordered {
						format!("{}.", *start as usize + index)
					} else {
						"\u{2022}".to_owned()
					};
					row()
						.w_full()
						.gap(8.)
						.child(text(marker).shrink_0().text_color(tok("text_muted")))
						.child(column().grow().min_w(0.).children(self.blocks(item, false)))
				})),
			_ => div(),
		}
	}

	fn code_block(&self, language: &str, code: &str) -> View {
		let code = code.trim_end().replace('\t', "    ");
		let mut spans = Vec::new();
		let mut covered = 0;
		for span in highlight(language, &code) {
			if span.start < covered || span.kind == TokenKind::Plain || span.end > code.len() {
				continue;
			}
			if span.start > covered {
				spans.push(text_span(code[covered..span.start].to_owned()));
			}
			let color = match span.kind {
				TokenKind::Keyword => "mention_text",
				TokenKind::String => "success",
				TokenKind::Comment => "text_muted",
				TokenKind::Number => "warning",
				TokenKind::Type => "link",
				_ => "text_normal",
			};
			spans.push(text_span(code[span.start..span.end].to_owned()).text_color(tok(color)));
			covered = span.end;
		}
		if covered < code.len() {
			spans.push(text_span(code[covered..].to_owned()));
		}
		div()
			.w_full()
			.mt(4.)
			.px(12.)
			.py(8.)
			.rounded(tok("radius_sm"))
			.bg(tok("code_background"))
			.border(1.)
			.border_color(tok("code_border"))
			.font_family(tok("code_font"))
			.text_size(tok("font_sm"))
			.line_height(CODE_LINE)
			.text_color(tok("text_normal"))
			.child(View::from(rich_text().children(spans)).w_full().text_wrap(true))
	}
}
