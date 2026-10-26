use std::cell::RefCell;
use std::rc::Rc;

use bridge::MessageEntry;
use bridge::components::{ChoiceOption, string_options};
use protocol::{
	ButtonStyle, Component, ComponentType, EmbedMedia, MessageFlags, PartialEmoji, Snowflake, UnfurledMedia,
};
use session::Command;
use zgx::compose::prelude::*;
use zgx::compose::{Placement, PlacementEdge, PlacementSide};
use zgx::reactive::{Effect, Signal};
use zgx::style::ObjectFit;

use super::attachments::picture;
use super::body::{self, Look};
use super::icons::icon;
use super::rows::{RowEnv, tok};
use crate::emoji;

const GALLERY_WIDTH: f32 = 468.;
const CONTAINER_MAX: f32 = 520.;
const GALLERY_ROW: f32 = 150.;
const GALLERY_GAP: f32 = 4.;
const TEXT_SIZE: f32 = 14.;
const SPOILER_BASE: usize = 50_000;
const ROW_ESTIMATE: f32 = 40.;
const SELECT_ESTIMATE: f32 = 48.;
const CONTAINER_ESTIMATE: f32 = 360.;
const SPACING_SMALL: f32 = 4.;
const SPACING_LARGE: f32 = 12.;

pub struct SelectState {
	open: Signal<bool>,
	chosen: Signal<Option<Vec<String>>>,
	sent: RefCell<Vec<String>>,
	watcher: RefCell<Option<Effect>>,
}

impl SelectState {
	pub fn pick(&self, values: Vec<String>, open: bool) {
		self.chosen.set(Some(values));
		self.open.set(open);
	}
}

#[derive(Clone)]
struct Owner {
	env: Rc<RowEnv>,
	channel_id: Snowflake,
	message_id: Snowflake,
	waiting: Option<String>,
	names: std::sync::Arc<bridge::Names>,
	bump: Signal<u32>,
	options: std::sync::Arc<bridge::components::SelectOptions>,
	attachments: Rc<Vec<protocol::Attachment>>,
}

impl Owner {
	fn press(&self, component: &Component, values: Option<Vec<String>>) {
		let Some(custom_id) = component.custom_id.clone() else {
			return;
		};
		self.env.send(Command::PressComponent {
			channel_id: self.channel_id,
			message_id: self.message_id,
			custom_id,
			kind: component.kind,
			values,
		});
	}

	fn waits_on(&self, component: &Component) -> bool {
		self.waiting.is_some() && self.waiting == component.custom_id
	}
}

pub fn height(entry: &MessageEntry) -> f32 {
	entry
		.message
		.components
		.iter()
		.map(|component| match component.kind {
			ComponentType::ActionRow if component.components.iter().any(Component::is_select) => {
				SELECT_ESTIMATE
			}
			ComponentType::ActionRow => ROW_ESTIMATE,
			ComponentType::Container => CONTAINER_ESTIMATE,
			ComponentType::TextDisplay => ROW_ESTIMATE,
			_ => 0.,
		})
		.sum()
}

pub fn components(entry: &MessageEntry, env: &Rc<RowEnv>, bump: &Signal<u32>) -> Option<View> {
	let message = &entry.message;
	if message.components.is_empty() {
		return None;
	}
	let owner = Owner {
		env: env.clone(),
		channel_id: message.channel_id,
		message_id: entry.id,
		waiting: entry.waiting_component.clone(),
		names: entry.names.clone(),
		bump: bump.clone(),
		options: entry.select_options.clone(),
		attachments: Rc::new(message.attachments.to_vec()),
	};
	let v2 = message.flags.contains(MessageFlags::IS_COMPONENTS_V2);
	let blocks: Vec<View> = message
		.components
		.iter()
		.filter_map(|component| block(component, &owner))
		.collect();
	Some(
		column()
			.w_full()
			.min_w(0.)
			.gap(tok("space_8"))
			.when(!v2 || !entry.content.is_empty(), |column| {
				column.mt(tok("space_8"))
			})
			.children(blocks),
	)
}

fn block(component: &Component, owner: &Owner) -> Option<View> {
	Some(match component.kind {
		ComponentType::ActionRow => action_row(component, owner),
		ComponentType::Button => press_button(component, owner),
		ComponentType::StringSelect
		| ComponentType::UserSelect
		| ComponentType::RoleSelect
		| ComponentType::MentionableSelect
		| ComponentType::ChannelSelect => select(component, owner),
		ComponentType::Container => container(component, owner),
		ComponentType::Section => section(component, owner),
		ComponentType::TextDisplay => text_display(component, owner),
		ComponentType::Thumbnail => thumbnail(component, owner)?,
		ComponentType::MediaGallery => gallery(component, owner)?,
		ComponentType::File => file(component, owner),
		ComponentType::Separator => separator(component),
		ComponentType::TextInput | ComponentType::Label | ComponentType::Unknown(_) => return None,
	})
}

fn action_row(component: &Component, owner: &Owner) -> View {
	row().w_full().flex_wrap().gap(tok("space_8")).children(
		component
			.components
			.iter()
			.filter_map(|child| block(child, owner)),
	)
}

pub fn emoji_view(emoji: &PartialEmoji, env: &Rc<RowEnv>, side: f32) -> Option<View> {
	match (emoji.id, emoji.name.as_deref()) {
		(Some(id), _) => {
			let name = env.pictures.emoji(id.0, emoji.animated, f64::from(side));
			Some(div().size(side, side).shrink_0().child(picture(
				&env.pictures,
				Some(name),
				None,
				ObjectFit::Contain,
			)))
		}
		(None, Some(name)) if !name.is_empty() => Some(emoji::glyph(name, side)),
		_ => None,
	}
}

fn button_colors(style: ButtonStyle) -> (&'static str, &'static str, &'static str, &'static str) {
	match style {
		ButtonStyle::Primary => ("accent_3", "accent_4", "button_border", "text_0"),
		ButtonStyle::Success => ("green_3", "green_4", "button_border", "text_0"),
		ButtonStyle::Danger => ("red_3", "red_4", "button_border", "text_0"),
		_ => ("bg_2", "bg_1", "border_light", "text_3"),
	}
}

fn press_button(component: &Component, owner: &Owner) -> View {
	let style = component.button_style();
	let link = style == ButtonStyle::Link;
	let disabled = component.disabled || style == ButtonStyle::Premium || style.is_unknown();
	let loading = owner.waits_on(component);
	let (rest, hovered, border, color) = button_colors(style);
	let key = component
		.custom_id
		.clone()
		.or_else(|| component.url.clone())
		.unwrap_or_default();
	let label = component.label.clone().unwrap_or_default();
	let mut content = row()
		.items_center()
		.gap(tok("space_8"))
		.when(loading, |content| content.opacity(0.))
		.when_some(
			component
				.emoji
				.as_ref()
				.and_then(|emoji| emoji_view(emoji, &owner.env, 20.)),
			|content, emoji| content.child(emoji),
		);
	if !label.is_empty() {
		content = content.child(
			text(label)
				.text_size(tok("font_size_sm"))
				.line_height(tok("line_height_sm"))
				.font_weight(500),
		);
	}
	if link {
		content = content.child(icon("external_link", 16., color));
	}
	let view = div()
		.id(format!("component-{}-{key}", owner.message_id))
		.relative()
		.shrink_0()
		.h(tok("component_button_height"))
		.px(tok("space_16"))
		.items_center()
		.justify_center()
		.rounded(tok("radius_sm"))
		.bg(tok(rest))
		.border(tok("border_thickness"))
		.border_color(tok(border))
		.text_color(tok(color))
		.child(content)
		.when(loading, |view| {
			view.child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.w_full()
					.h_full()
					.items_center()
					.justify_center()
					.child(icon("spinner", 16., color)),
			)
		});
	if disabled {
		return view.opacity(0.5);
	}
	if loading {
		return view;
	}
	let (owner, pressed) = (owner.clone(), component.clone());
	view.cursor(Cursor::Pointer)
		.hover(move |style| style.bg(tok(hovered)))
		.on_click(move || match (link, pressed.url.as_deref()) {
			(true, Some(url)) => owner.env.links.open(url),
			(true, None) => {}
			(false, _) => owner.press(&pressed, None),
		})
}

fn options_of(component: &Component, owner: &Owner) -> Vec<ChoiceOption> {
	match component.kind {
		ComponentType::StringSelect => string_options(component),
		_ => component
			.custom_id
			.as_ref()
			.and_then(|custom_id| owner.options.get(custom_id).cloned())
			.unwrap_or_default(),
	}
}

fn select_state(owner: &Owner, component: &Component, defaults: Vec<String>) -> Rc<SelectState> {
	let key = (owner.message_id, component.custom_id.clone().unwrap_or_default());
	if let Some(found) = owner.env.selects.borrow().get(&key) {
		return found.clone();
	}
	let runtime = owner.env.pictures.runtime();
	let state = Rc::new(SelectState {
		open: runtime.signal(false),
		chosen: runtime.signal(None),
		sent: RefCell::new(defaults),
		watcher: RefCell::new(None),
	});
	let multi = component.max_values.unwrap_or(1) > 1;
	if multi {
		let (watched, sender, pressed) = (Rc::downgrade(&state), owner.clone(), component.clone());
		let minimum = component.min_values.unwrap_or(1).max(0) as usize;
		let mut was = false;
		let open = state.open.clone();
		let effect = runtime.effect(move || {
			let now = open.get();
			if was
				&& !now && let Some(state) = watched.upgrade()
			{
				let chosen = state.chosen.with_untracked(Clone::clone);
				if let Some(chosen) = chosen
					&& chosen != *state.sent.borrow()
					&& chosen.len() >= minimum
				{
					*state.sent.borrow_mut() = chosen.clone();
					sender.press(&pressed, Some(chosen));
				}
			}
			was = now;
		});
		*state.watcher.borrow_mut() = Some(effect);
	}
	owner.env.selects.borrow_mut().insert(key, state.clone());
	state
}

fn shown_choice(option: &ChoiceOption, env: &Rc<RowEnv>, last: bool) -> View {
	let label = if last {
		option.label.clone()
	} else {
		format!("{},", option.label)
	};
	row()
		.items_center()
		.gap(tok("space_8"))
		.shrink_0()
		.when_some(
			option
				.emoji
				.as_ref()
				.and_then(|emoji| emoji_view(emoji, env, 20.)),
			|row, emoji| row.child(emoji),
		)
		.child(text(label))
}

fn select(component: &Component, owner: &Owner) -> View {
	let options = Rc::new(options_of(component, owner));
	let defaults: Vec<String> = options
		.iter()
		.filter(|option| option.default)
		.map(|option| option.value.clone())
		.collect();
	let state = select_state(owner, component, defaults.clone());
	let multi = component.max_values.unwrap_or(1) > 1;
	let maximum = component.max_values.unwrap_or(1).max(1) as usize;
	let disabled = component.disabled;
	let loading = owner.waits_on(component);
	let placeholder = component
		.placeholder
		.clone()
		.unwrap_or_else(|| "Make a selection".to_owned());
	let key = component.custom_id.clone().unwrap_or_default();
	let (chosen, env, shown_options) = (state.chosen.clone(), owner.env.clone(), options.clone());
	let shown_defaults = defaults.clone();
	let summary = switch(
		move || chosen.get().unwrap_or_else(|| shown_defaults.clone()),
		move |values: Vec<String>, _| {
			let picked: Vec<&ChoiceOption> = shown_options
				.iter()
				.filter(|option| values.contains(&option.value))
				.collect();
			if picked.is_empty() {
				return text(placeholder.clone()).text_color(tok("text_4"));
			}
			let mut line = row()
				.items_center()
				.gap(tok("space_4"))
				.min_w(0.)
				.overflow_hidden();
			for (index, option) in picked.iter().enumerate() {
				line = line.child(shown_choice(option, &env, index + 1 == picked.len()));
			}
			line
		},
	);
	let open = state.open.clone();
	let chevron_open = open.clone();
	let field = button()
		.id(format!("component-{}-{key}", owner.message_id))
		.w(tok("component_select_width"))
		.max_w_percent(100.)
		.h(tok("component_select_height"))
		.pl(tok("space_12"))
		.pr(tok("space_8"))
		.gap(tok("space_8"))
		.items_center()
		.justify_between()
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.text_color(tok("text_3"))
		.text_size(tok("font_size_sm"))
		.line_height(tok("line_height_sm"))
		.font_weight(500)
		.child(summary.grow().min_w(0.))
		.child(if loading {
			icon("spinner", 16., "text_3")
		} else {
			switch(
				move || chevron_open.get(),
				|open: bool, _| icon(if open { "chevron_up" } else { "chevron_down" }, 20., "text_3"),
			)
		});
	if disabled || loading {
		return field.opacity(if disabled { 0.5 } else { 1. });
	}
	let toggle = open.clone();
	let field = field.cursor(Cursor::Pointer).on_click(move || {
		toggle.set(!toggle.with_untracked(|open| *open));
	});
	let rows: Vec<View> = options
		.iter()
		.map(|option| {
			let (state, owner, pressed) = (state.clone(), owner.clone(), component.clone());
			let value = option.value.clone();
			let fallback = defaults.clone();
			let chosen = state.chosen.clone();
			let marked = {
				let (chosen, value, fallback) = (chosen.clone(), value.clone(), fallback.clone());
				move || chosen.get().unwrap_or_else(|| fallback.clone()).contains(&value)
			};
			select_row(option, &owner.env, multi, marked).on_click(move || {
				let mut current = state
					.chosen
					.with_untracked(Clone::clone)
					.unwrap_or_else(|| fallback.clone());
				if multi {
					match current.iter().position(|picked| *picked == value) {
						Some(at) => {
							current.remove(at);
						}
						None if current.len() < maximum => current.push(value.clone()),
						None => {}
					}
					state.chosen.set(Some(current));
				} else {
					state.chosen.set(Some(vec![value.clone()]));
					state.open.set(false);
					*state.sent.borrow_mut() = vec![value.clone()];
					owner.press(&pressed, Some(vec![value.clone()]));
				}
			})
		})
		.collect();
	popover(format!("{key} options"), open, field)
		.placement(Placement {
			side: PlacementSide::Bottom,
			align: PlacementEdge::Start,
			offset: 0.,
		})
		.match_anchor_width()
		.py(tok("space_4"))
		.px(tok("space_4"))
		.gap(0.)
		.rounded(tok("radius_sm"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.children(rows)
}

fn select_row(
	option: &ChoiceOption,
	env: &Rc<RowEnv>,
	multi: bool,
	marked: impl Fn() -> bool + 'static,
) -> View {
	let mark = if multi {
		crate::kit::checkbox(marked)
	} else {
		icon("check", 20., "accent_2").reactive_style(move || {
			if marked() {
				Styles::new().visible()
			} else {
				Styles::new().invisible()
			}
		})
	};
	row()
		.w_full()
		.h(tok("component_select_row_height"))
		.px(tok("space_8"))
		.gap(tok("space_8"))
		.items_center()
		.rounded(tok("radius_xs"))
		.cursor(Cursor::Pointer)
		.hover(|style| style.bg(tok("hover")))
		.when_some(
			option
				.emoji
				.as_ref()
				.and_then(|emoji| emoji_view(emoji, env, 20.)),
			|row, emoji| row.child(emoji),
		)
		.child(
			column()
				.grow()
				.min_w(0.)
				.child(
					text(option.label.clone())
						.truncate()
						.text_size(tok("font_size_sm"))
						.line_height(tok("line_height_sm"))
						.font_weight(500)
						.text_color(tok("text_3")),
				)
				.when_some(option.description.clone(), |column, description| {
					column.child(
						text(description)
							.truncate()
							.text_size(tok("font_size_xs"))
							.line_height(tok("line_height_xs"))
							.text_color(tok("text_4")),
					)
				}),
		)
		.child(mark)
}

fn container(component: &Component, owner: &Owner) -> View {
	let accent = component.accent_color.and_then(|color| u32::try_from(color).ok());
	column()
		.relative()
		.w_full()
		.max_w(CONTAINER_MAX)
		.gap(tok("space_8"))
		.p(tok("space_16"))
		.rounded(tok("radius_md"))
		.overflow_hidden()
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.when_some(accent, |column, color| {
			column.child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.bottom(0.)
					.w(tok("divider_thickness"))
					.bg(rgb(color)),
			)
		})
		.children(
			component
				.components
				.iter()
				.filter_map(|child| block(child, owner)),
		)
}

fn section(component: &Component, owner: &Owner) -> View {
	let accessory = component.accessory.as_deref();
	let centred = accessory.is_some_and(|accessory| accessory.kind == ComponentType::Button);
	row()
		.w_full()
		.gap(tok("space_16"))
		.when(centred, |row| row.items_center())
		.when(!centred, |row| row.items_start())
		.child(
			column().grow().min_w(0.).children(
				component
					.components
					.iter()
					.filter_map(|child| block(child, owner)),
			),
		)
		.when_some(
			accessory.and_then(|accessory| block(accessory, owner)),
			|row, view| row.child(view),
		)
}

fn text_display(component: &Component, owner: &Owner) -> View {
	let content = component.content.clone().unwrap_or_default();
	body::render(
		&markdown::parse(&content),
		&owner.env,
		&owner.names,
		owner.message_id,
		SPOILER_BASE,
		&owner.bump,
		Look {
			size: TEXT_SIZE,
			color: "text_3",
			jumbo: false,
			trailer: None,
			on_trailer: None,
		},
	)
}

fn embed_media(media: &UnfurledMedia) -> EmbedMedia {
	EmbedMedia {
		url: Some(media.url.clone()),
		proxy_url: media.proxy_url.clone(),
		height: media.height,
		width: media.width,
		content_type: media.content_type.clone(),
		placeholder: media.placeholder.clone(),
	}
}

fn tile(media: &UnfurledMedia, owner: &Owner, size: (f32, f32)) -> View {
	let media = embed_media(media);
	let name = owner.env.pictures.embed_media(&media, size);
	let target = bridge::LightboxImage::embed(&media);
	let lightbox = owner.env.lightbox.clone();
	div()
		.rounded(tok("radius_sm"))
		.overflow_hidden()
		.child(picture(
			&owner.env.pictures,
			name,
			media.placeholder.as_deref(),
			ObjectFit::Cover,
		))
		.when_some(target, move |frame, target| {
			frame
				.cursor(Cursor::Pointer)
				.on_click(move || lightbox.open(target.clone()))
		})
}

fn thumbnail(component: &Component, owner: &Owner) -> Option<View> {
	let media = component.media.as_ref()?;
	let side = 85.;
	Some(
		tile(media, owner, (side, side))
			.shrink_0()
			.w(tok("component_thumbnail_size"))
			.h(tok("component_thumbnail_size")),
	)
}

fn gallery(component: &Component, owner: &Owner) -> Option<View> {
	let items: Vec<&UnfurledMedia> = component.items.iter().map(|item| &item.media).collect();
	let full = GALLERY_WIDTH;
	Some(match items.as_slice() {
		[] => return None,
		[only] => tile(only, owner, (full, 220.))
			.w_full()
			.h(tok("component_gallery_height")),
		[left, right] => row()
			.w_full()
			.h(tok("component_gallery_height"))
			.gap(GALLERY_GAP)
			.child(
				tile(left, owner, (full / 2., 220.))
					.grow()
					.flex_basis(0.)
					.h_full(),
			)
			.child(
				tile(right, owner, (full / 2., 220.))
					.grow()
					.flex_basis(0.)
					.h_full(),
			),
		[first, second, third] => row()
			.w_full()
			.h(tok("component_gallery_height"))
			.gap(GALLERY_GAP)
			.child(
				tile(first, owner, (full * 2. / 3., 220.))
					.flex_grow(2.)
					.flex_basis(0.)
					.h_full(),
			)
			.child(
				column()
					.flex_grow(1.)
					.flex_basis(0.)
					.h_full()
					.gap(GALLERY_GAP)
					.child(tile(second, owner, (full / 3., 110.)).w_full().grow())
					.child(tile(third, owner, (full / 3., 110.)).w_full().grow()),
			),
		many => column()
			.w_full()
			.gap(GALLERY_GAP)
			.children(many.chunks(3).map(|chunk| {
				row()
					.w_full()
					.h(GALLERY_ROW)
					.gap(GALLERY_GAP)
					.children(chunk.iter().map(|media| {
						tile(media, owner, (full / 3., GALLERY_ROW))
							.grow()
							.flex_basis(0.)
							.h_full()
					}))
			})),
	})
}

fn file(component: &Component, owner: &Owner) -> View {
	let url = component
		.file
		.as_ref()
		.map(|file| file.url.clone())
		.unwrap_or_default();
	let named = url.strip_prefix("attachment://").unwrap_or(&url).to_owned();
	let attachment = owner
		.attachments
		.iter()
		.find(|attachment| attachment.filename == named)
		.cloned()
		.unwrap_or_else(|| protocol::Attachment {
			filename: component.name.clone().unwrap_or(named),
			size: component.size,
			url: Some(url),
			..protocol::Attachment::default()
		});
	super::attachments::file_card(&attachment, &owner.env)
}

fn separator(component: &Component) -> View {
	let space = if component.spacing == Some(2) {
		SPACING_LARGE
	} else {
		SPACING_SMALL
	};
	let line = div().w_full().my(space).h(tok("border_thickness"));
	if component.divider.unwrap_or(true) {
		line.bg(tok("border"))
	} else {
		line
	}
}
