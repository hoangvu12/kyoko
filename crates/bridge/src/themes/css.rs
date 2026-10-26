use std::collections::HashMap;

use super::color::parse_color;
use super::palette::{Layer, Scheme};

const MAX_DEPTH: usize = 16;
const MAX_FIELD_LENGTH: usize = 200;

pub const CSS_MAPPING: &[(&str, &[&str], &[&str])] = &[
	(
		"background_primary",
		&["--background-primary", "--background-base-low"],
		&[],
	),
	(
		"background_secondary",
		&["--background-secondary", "--background-base-lower"],
		&[],
	),
	(
		"background_tertiary",
		&["--background-tertiary", "--background-base-lowest"],
		&[],
	),
	(
		"background_floating",
		&["--background-floating", "--background-surface-highest"],
		&[],
	),
	("user_panel_background", &["--background-secondary-alt"], &[]),
	("modifier_hover", &["--background-modifier-hover"], &[]),
	("modifier_selected", &["--background-modifier-selected"], &[]),
	("modifier_active", &["--background-modifier-active"], &[]),
	("message_hover", &["--background-message-hover"], &[]),
	("text_normal", &["--text-normal", "--text-default"], &[]),
	("text_muted", &["--text-muted"], &[]),
	("header_primary", &["--header-primary", "--text-strong"], &[]),
	("header_secondary", &["--header-secondary", "--text-subtle"], &[]),
	("interactive_normal", &["--interactive-normal"], &[]),
	("interactive_hover", &["--interactive-hover"], &[]),
	("interactive_active", &["--interactive-active"], &[]),
	("interactive_muted", &["--interactive-muted"], &[]),
	("text_on_brand", &["--white-500"], &[]),
	(
		"brand",
		&["--brand-experiment", "--brand-500", "--brand-experiment-500"],
		&[],
	),
	("brand_hover", &["--brand-experiment-560", "--brand-560"], &[]),
	("link", &["--text-link"], &[]),
	("mention_background", &["--mention-background"], &[]),
	("mention_text", &["--mention-foreground"], &[]),
	("status_online", &["--status-online", "--status-positive"], &[]),
	("status_idle", &["--status-idle", "--status-warning"], &[]),
	("status_dnd", &["--status-dnd", "--status-danger"], &[]),
	("status_offline", &["--status-offline"], &[]),
	("danger", &["--status-danger", "--text-danger"], &[]),
	("danger_hover", &["--button-danger-background-hover"], &[]),
	("success", &["--status-positive", "--text-positive"], &[]),
	("warning", &["--status-warning", "--text-warning"], &[]),
	(
		"scrollbar_thumb",
		&["--scrollbar-auto-thumb", "--scrollbar-thin-thumb"],
		&[],
	),
	("scrollbar_track", &["--scrollbar-auto-track"], &[]),
	("divider", &["--background-modifier-accent"], &[]),
	(
		"input_background",
		&["--input-background", "--channeltextarea-background"],
		&[],
	),
	("button_secondary", &["--button-secondary-background"], &[]),
	(
		"button_secondary_hover",
		&["--button-secondary-background-hover"],
		&[],
	),
	("spoiler", &["--spoiler-hidden-background"], &[]),
	("focus_ring", &["--focus-primary"], &[]),
	("overlay", &["--background-scrim"], &[]),
	("poll_fill", &[], &["--poll-fill"]),
	("text_0", &[], &["--text-0"]),
	("text_1", &[], &["--text-1"]),
	("text_2", &[], &["--text-2"]),
	("text_3", &[], &["--text-3"]),
	("text_4", &[], &["--text-4"]),
	("text_5", &[], &["--text-5"]),
	("bg_1", &[], &["--bg-1"]),
	("bg_2", &[], &["--bg-2"]),
	("bg_3", &[], &["--bg-3"]),
	("bg_4", &[], &["--bg-4"]),
	("hover", &[], &["--hover"]),
	("active", &[], &["--active"]),
	("active_2", &[], &["--active-2"]),
	("message_hover", &[], &["--message-hover"]),
	("accent_1", &[], &["--accent-1"]),
	("accent_2", &[], &["--accent-2"]),
	("accent_3", &[], &["--accent-3"]),
	("accent_4", &[], &["--accent-4"]),
	("accent_5", &[], &["--accent-5"]),
	("accent_new", &[], &["--accent-new"]),
	("mention", &[], &["--mention"]),
	("mention_hover", &[], &["--mention-hover"]),
	("reply", &[], &["--reply"]),
	("reply_hover", &[], &["--reply-hover"]),
	("online", &[], &["--online"]),
	("dnd", &[], &["--dnd"]),
	("idle", &[], &["--idle"]),
	("streaming", &[], &["--streaming"]),
	("offline", &[], &["--offline"]),
	("border_light", &[], &["--border-light"]),
	("border", &[], &["--border"]),
	("border_hover", &[], &["--border-hover"]),
	("button_border", &[], &["--button-border"]),
	("red_1", &[], &["--red-1"]),
	("red_2", &[], &["--red-2"]),
	("red_3", &[], &["--red-3"]),
	("red_4", &[], &["--red-4"]),
	("red_5", &[], &["--red-5"]),
	("green_1", &[], &["--green-1"]),
	("green_2", &[], &["--green-2"]),
	("green_3", &[], &["--green-3"]),
	("green_4", &[], &["--green-4"]),
	("green_5", &[], &["--green-5"]),
	("blue_1", &[], &["--blue-1"]),
	("blue_2", &[], &["--blue-2"]),
	("blue_3", &[], &["--blue-3"]),
	("blue_4", &[], &["--blue-4"]),
	("blue_5", &[], &["--blue-5"]),
	("yellow_1", &[], &["--yellow-1"]),
	("yellow_2", &[], &["--yellow-2"]),
	("yellow_3", &[], &["--yellow-3"]),
	("yellow_4", &[], &["--yellow-4"]),
	("yellow_5", &[], &["--yellow-5"]),
	("purple_1", &[], &["--purple-1"]),
	("purple_2", &[], &["--purple-2"]),
	("purple_3", &[], &["--purple-3"]),
	("purple_4", &[], &["--purple-4"]),
	("purple_5", &[], &["--purple-5"]),
	("background_scrim_lightbox", &[], &["--background-scrim-lightbox"]),
	(
		"background_feedback_critical",
		&["--background-feedback-critical"],
		&[],
	),
	(
		"background_feedback_positive",
		&["--background-feedback-positive"],
		&[],
	),
	(
		"background_feedback_warning",
		&["--background-feedback-warning"],
		&[],
	),
	("background_feedback_info", &["--background-feedback-info"], &[]),
	("border_feedback_warning", &["--border-feedback-warning"], &[]),
	(
		"input_background_error",
		&["--input-background-error-default"],
		&[],
	),
	(
		"reaction_background_reacted",
		&["--reaction-background-reacted-default"],
		&[],
	),
	(
		"user_panel_button_active",
		&["--user-panel-button-active-background"],
		&[],
	),
	("poll_fill_winner", &[], &["--poll-fill-winner"]),
	("overlay_button_background", &["--overlay-button-background"], &[]),
	("overlay_text", &[], &["--overlay-text"]),
	("switch_thumb_background", &["--switch-thumb-background"], &[]),
];

pub const CSS_SIZES: &[(&str, &str)] = &[
	("font_size_tag", "--font-size-tag"),
	("font_size_timestamp_alt", "--font-size-timestamp-alt"),
	("font_size_xs", "--font-size-xs"),
	("font_size_sm", "--font-size-sm"),
	("font_size_md", "--font-size-md"),
	("font_size_lg", "--font-size-lg"),
	("font_size_xl", "--font-size-xl"),
	("line_height_xs", "--line-height-xs"),
	("line_height_sm", "--line-height-sm"),
	("line_height_md", "--line-height-md"),
	("line_height_lg", "--line-height-lg"),
	("line_height_xl", "--line-height-xl"),
	("gap", "--gap"),
	("divider_thickness", "--divider-thickness"),
	("border_thickness", "--border-thickness"),
	("chatbar_height", "--chatbar-height"),
	("top_strip_height", "--top-bar-height"),
	("window_control_size", "--window-control-size"),
	("window_controls_padding", "--window-controls-padding"),
	("list_item_hover_shift", "--list-item-hover-shift"),
	("member_hover_shift", "--member-hover-shift"),
	("space_1", "--space-1"),
	("space_2", "--space-2"),
	("space_4", "--space-4"),
	("space_6", "--space-6"),
	("space_8", "--space-8"),
	("space_12", "--space-12"),
	("space_16", "--space-16"),
	("space_20", "--space-20"),
	("space_24", "--space-24"),
	("space_32", "--space-32"),
	("space_40", "--space-40"),
	("space_xs", "--space-xs"),
	("space_sm", "--space-sm"),
	("space_md", "--space-md"),
	("space_lg", "--space-lg"),
	("radius_xs", "--radius-xs"),
	("radius_sm", "--radius-sm"),
	("radius_md", "--radius-md"),
	("radius_lg", "--radius-lg"),
	("guildbar_avatar_size", "--guildbar-avatar-size"),
	("guild_list_padding", "--custom-guild-list-padding"),
	("guild_list_width", "--custom-guild-list-width"),
	("guild_sidebar_width", "--custom-guild-sidebar-width"),
	("channel_header_height", "--custom-channel-header-height"),
	("member_list_panel_width", "--member-list-width"),
	("search_results_width", "--search-results-width"),
	("icon_size_xs", "--icon-size-xs"),
	("icon_size_sm", "--icon-size-sm"),
	("icon_size_md", "--icon-size-md"),
	("icon_size_lg", "--icon-size-lg"),
	("button_size", "--button-size"),
	("button_height_sm", "--button-height-sm"),
	("button_height_md", "--button-height-md"),
	("button_height_lg", "--button-height-lg"),
	("input_height", "--input-height"),
	("avatar_size_xs", "--avatar-size-xs"),
	("avatar_size_sm", "--avatar-size-sm"),
	("avatar_size_md", "--avatar-size-md"),
	("avatar_size_lg", "--avatar-size-lg"),
	("status_size", "--status-size"),
	("status_cutout", "--status-cutout"),
	("status_size_lg", "--status-size-lg"),
	("status_cutout_lg", "--status-cutout-lg"),
	("role_icon_size", "--role-icon-size"),
	("folder_size", "--folder-size"),
	("guild_pill_width", "--guild-pill-width"),
	("guild_pill_selected_height", "--guild-pill-selected-height"),
	("guild_pill_unread_height", "--guild-pill-unread-height"),
	("guild_separator_width", "--guild-separator-width"),
	("badge_size", "--badge-size"),
	("unread_dot_size", "--unread-dot-size"),
	("channel_item_height", "--channel-row-height"),
	("category_row_height", "--category-row-height"),
	("member_row_height", "--member-row-height"),
	("dm_row_height", "--dm-row-height"),
	("group_header_height", "--group-header-height"),
	("search_width", "--search-width"),
	("new_messages_bar_height", "--new-messages-bar-height"),
	("typing_height", "--typing-height"),
	("message_content_left", "--message-gutter"),
	("message_group_gap", "--message-group-gap"),
	("message_padding_right", "--message-padding-right"),
	("reply_spine_width", "--reply-spine-width"),
	("reply_spine_radius", "--reply-spine-radius"),
	("reaction_height", "--reaction-height"),
	("reaction_border_width", "--reaction-border-width"),
	("embed_max_width", "--embed-max-width"),
	("embed_accent_width", "--embed-accent-width"),
	("tag_height", "--tag-height"),
	("bot_tag_height", "--bot-tag-height"),
	("unread_pill_height", "--unread-pill-height"),
	("switch_width", "--switch-width"),
	("switch_height", "--switch-height"),
	("switch_thumb_size", "--switch-thumb-size"),
	("radio_size", "--radio-size"),
	("slider_track_height", "--slider-track-height"),
	("menu_width", "--menu-width"),
	("menu_item_height", "--menu-item-height"),
	("popout_width", "--popout-width"),
	("banner_height", "--banner-height"),
	("settings_banner_height", "--settings-banner-height"),
	("modal_width_sm", "--modal-width-sm"),
	("modal_width_wide", "--modal-width-md"),
	("expression_picker_width", "--expression-picker-width"),
	("expression_picker_height", "--expression-picker-height"),
	(
		"expression_picker_anchor_inset",
		"--expression-picker-anchor-inset",
	),
	("picker_tab_height", "--picker-tab-height"),
	("picker_search_height", "--picker-search-height"),
	("picker_header_height", "--picker-header-height"),
	("picker_rail_width", "--picker-rail-width"),
	("picker_footer_height", "--picker-footer-height"),
	("picker_section_header_height", "--picker-section-header-height"),
	("emoji_cell_size", "--emoji-cell-size"),
	("emoji_size", "--emoji-size"),
	("emoji_size_footer", "--emoji-size-footer"),
	("emoji_size_inline", "--emoji-size-inline"),
	("emoji_size_jumbo", "--emoji-size-jumbo"),
	("emoji_size_tooltip", "--emoji-size-tooltip"),
	("emoji_size_autocomplete", "--emoji-size-autocomplete"),
	("gif_tile_height", "--gif-tile-height"),
	("sticker_cell_size", "--sticker-cell-size"),
	("sticker_size_message", "--sticker-size-message"),
	("gif_max_width", "--gif-max-width"),
	("autocomplete_row_height", "--autocomplete-row-height"),
	("emoji_popout_width", "--emoji-popout-width"),
	("locked_opacity", "--locked-opacity"),
	("emoji_size_reaction", "--emoji-size-reaction"),
	("reaction_tooltip_width", "--reaction-tooltip-width"),
	("reactions_list_height", "--reactions-list-height"),
	("reactions_list_column_width", "--reactions-list-column-width"),
	("reactions_list_item_height", "--reactions-list-item-height"),
	("reactions_list_row_height", "--reactions-list-row-height"),
	("gif_message_width", "--gif-message-width"),
	("command_picker_height", "--command-picker-height"),
	("command_rail_width", "--command-rail-width"),
	("command_row_height", "--command-row-height"),
	("command_section_header_height", "--command-section-header-height"),
	("command_chip_height", "--command-chip-height"),
	("command_option_height", "--command-option-height"),
	("command_hint_height", "--command-hint-height"),
	("command_dropbox_height", "--command-dropbox-height"),
	("font_size_description", "--font-size-description"),
	("line_height_description", "--line-height-description"),
	("component_button_height", "--component-button-height"),
	("component_select_width", "--component-select-width"),
	("component_select_height", "--component-select-height"),
	("component_select_row_height", "--component-select-row-height"),
	("component_container_max_width", "--component-container-max-width"),
	("component_thumbnail_size", "--component-thumbnail-size"),
	("component_gallery_height", "--component-gallery-height"),
	("app_modal_paragraph_height", "--app-modal-paragraph-height"),
	("apps_launcher_width", "--apps-launcher-width"),
	("app_tile_size", "--app-tile-size"),
	("app_row_height", "--app-row-height"),
	("composer_bar_height", "--composer-bar-height"),
	("upload_card_size", "--upload-card-size"),
	("upload_thumbnail_height", "--upload-thumbnail-height"),
	("upload_toolbar_button_size", "--upload-toolbar-button-size"),
	("drop_card_width", "--drop-card-width"),
	("progress_height", "--progress-height"),
	("poll_max_width", "--poll-max-width"),
	("poll_row_height", "--poll-row-height"),
	("voice_message_width", "--voice-message-width"),
	("voice_message_height", "--voice-message-height"),
	("waveform_bar_width", "--waveform-bar-width"),
	("waveform_bar_max_height", "--waveform-bar-max-height"),
	("audio_card_width", "--audio-card-width"),
	("embed_image_height", "--embed-image-height"),
	("command_result_height", "--command-result-height"),
	("qr_size", "--qr-size"),
	("disabled_opacity", "--disabled-opacity"),
	("offline_opacity", "--offline-opacity"),
	("settings_dialog_inset", "--settings-dialog-inset"),
	("settings_dialog_max_width", "--settings-dialog-max-width"),
	("settings_dialog_nav_width", "--settings-nav-width"),
	("settings_topbar_height", "--settings-topbar-height"),
	("settings_column_width", "--settings-column-width"),
	("settings_item_height", "--settings-item-height"),
	("settings_row_gap", "--settings-row-gap"),
	("settings_divider_margin", "--settings-divider-margin"),
	("settings_page_padding_top", "--settings-page-padding-top"),
	("space_10", "--space-10"),
	("space_14", "--space-14"),
	("space_56", "--space-56"),
	("line_height_label", "--line-height-label"),
	("select_height", "--select-height"),
	("vc_switch_width", "--vc-switch-width"),
	("vc_switch_thumb_size", "--vc-switch-thumb-size"),
	("radio_border_selected", "--radio-border-selected"),
	("slider_grabber_size", "--slider-grabber-size"),
	("theme_swatch_size", "--theme-swatch-size"),
	("disabled_opacity_control", "--disabled-opacity-control"),
	("avatar_size_xl", "--avatar-size-xl"),
	("status_size_xl", "--status-size-xl"),
	("status_cutout_xl", "--status-cutout-xl"),
	("profile_dialog_width", "--profile-dialog-width"),
	("profile_dialog_height", "--profile-dialog-height"),
	("profile_dialog_banner_height", "--profile-banner-height"),
	("profile_column_width", "--profile-column-width"),
	("checkbox_size", "--checkbox-size"),
	("context_menu_width", "--context-menu-width"),
	("status_menu_width", "--status-menu-width"),
	("toast_width", "--toast-width"),
	("lightbox_max_width", "--lightbox-max-width"),
	("forum_card_thumb", "--forum-card-thumb"),
	("thread_panel_width", "--thread-panel-width"),
	("dm_profile_width", "--dm-profile-width"),
	("login_card_width", "--login-card-width"),
	("login_card_narrow", "--login-card-narrow"),
	("web_login_width", "--web-login-width"),
	("web_login_height", "--web-login-height"),
];

const GLOBAL_SELECTORS: &[&str] = &[":root", "html", "body", "#app-mount", "*"];
const GLOBAL_PREFIXES: &[&str] = &[".theme-", ".platform-", ".visual-refresh", ".app-focused"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeMeta {
	pub name: String,
	pub author: String,
	pub description: String,
	pub version: String,
	pub source: String,
	pub website: String,
}

pub fn parse_meta(text: &str, fallback_name: &str) -> ThemeMeta {
	let mut meta = ThemeMeta::default();
	if let Some(start) = text.find("/**") {
		let rest = &text[start + 3..];
		let body = rest.find("*/").map_or(rest, |end| &rest[..end]);
		for line in body.lines() {
			let line = line.trim().trim_start_matches('*').trim();
			let Some(tagged) = line.strip_prefix('@') else {
				continue;
			};
			let (tag, value) = tagged.split_once(char::is_whitespace).unwrap_or((tagged, ""));
			let value = clean(value);
			let slot = match tag {
				"name" => &mut meta.name,
				"author" => &mut meta.author,
				"description" => &mut meta.description,
				"version" => &mut meta.version,
				"source" => &mut meta.source,
				"website" => &mut meta.website,
				_ => continue,
			};
			if slot.is_empty() {
				*slot = value;
			}
		}
	}
	if meta.name.is_empty() {
		meta.name = clean(fallback_name);
	}
	meta
}

fn clean(value: &str) -> String {
	value
		.chars()
		.filter(|character| !character.is_control())
		.take(MAX_FIELD_LENGTH)
		.collect::<String>()
		.trim()
		.to_owned()
}

struct Declaration {
	name: String,
	value: String,
	scheme: Scheme,
}

pub fn import_css(text: &str) -> Layer {
	let declarations = declarations(text);
	let any = environment(&declarations, None);
	let light = environment(&declarations, Some(Scheme::Light));
	let dark = environment(&declarations, Some(Scheme::Dark));
	let mut layer = Layer::default();
	for (token, discord, midnight) in CSS_MAPPING {
		let candidates: Vec<&str> = discord.iter().chain(midnight.iter()).copied().collect();
		let shared = pick(&any, &candidates);
		if let Some(value) = &shared {
			layer.set_color(token, value.clone(), Scheme::Any);
		}
		for (scheme, environment) in [(Scheme::Light, &light), (Scheme::Dark, &dark)] {
			if let Some(value) = pick(environment, &candidates)
				&& Some(&value) != shared.as_ref()
			{
				layer.set_color(token, value, scheme);
			}
		}
	}
	for (token, name) in CSS_SIZES {
		if let Some(size) = pick_size(&any, name) {
			layer.set_size(token, size);
		}
	}
	layer
}

fn pick_size(environment: &HashMap<&str, &str>, name: &str) -> Option<f32> {
	let value = environment.get(name)?;
	let resolved = resolve(value, environment, 0)?;
	let number = resolved.strip_suffix("px").unwrap_or(&resolved).trim();
	number
		.parse::<f32>()
		.ok()
		.filter(|size| size.is_finite() && *size >= 0.)
}

fn pick(environment: &HashMap<&str, &str>, candidates: &[&str]) -> Option<String> {
	candidates.iter().find_map(|name| {
		let value = environment.get(name)?;
		let resolved = resolve(value, environment, 0)?;
		parse_color(&resolved).map(|color| color.hex())
	})
}

fn environment(declarations: &[Declaration], scheme: Option<Scheme>) -> HashMap<&str, &str> {
	let mut environment = HashMap::new();
	for declaration in declarations
		.iter()
		.filter(|declaration| declaration.scheme == Scheme::Any)
	{
		environment.insert(declaration.name.as_str(), declaration.value.as_str());
	}
	if let Some(scheme) = scheme {
		for declaration in declarations
			.iter()
			.filter(|declaration| declaration.scheme == scheme)
		{
			environment.insert(declaration.name.as_str(), declaration.value.as_str());
		}
	}
	environment
}

fn resolve(value: &str, environment: &HashMap<&str, &str>, depth: usize) -> Option<String> {
	if depth > MAX_DEPTH {
		return None;
	}
	let mut output = String::new();
	let mut rest = value;
	while let Some(start) = rest.find("var(") {
		output.push_str(&rest[..start]);
		let inner_start = start + 4;
		let end = closing(&rest[inner_start..])? + inner_start;
		let inner = &rest[inner_start..end];
		let (name, fallback) = match top_level_comma(inner) {
			Some(comma) => (inner[..comma].trim(), Some(inner[comma + 1..].trim())),
			None => (inner.trim(), None),
		};
		let replaced = match environment.get(name) {
			Some(found) => resolve(found, environment, depth + 1),
			None => None,
		}
		.or_else(|| fallback.and_then(|fallback| resolve(fallback, environment, depth + 1)))?;
		output.push_str(&replaced);
		rest = &rest[end + 1..];
	}
	output.push_str(rest);
	Some(output.trim().to_owned())
}

fn closing(text: &str) -> Option<usize> {
	let mut depth = 0usize;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' if depth == 0 => return Some(index),
			')' => depth -= 1,
			_ => {}
		}
	}
	None
}

fn top_level_comma(text: &str) -> Option<usize> {
	let mut depth = 0usize;
	for (index, character) in text.char_indices() {
		match character {
			'(' => depth += 1,
			')' => depth = depth.saturating_sub(1),
			',' if depth == 0 => return Some(index),
			_ => {}
		}
	}
	None
}

fn declarations(text: &str) -> Vec<Declaration> {
	let mut found = Vec::new();
	let mut stack: Vec<String> = Vec::new();
	let mut buffer = String::new();
	let mut depth = 0usize;
	let mut quote: Option<char> = None;
	let mut characters = text.chars().peekable();
	while let Some(character) = characters.next() {
		if let Some(open) = quote {
			buffer.push(character);
			if character == '\\' {
				if let Some(escaped) = characters.next() {
					buffer.push(escaped);
				}
			} else if character == open {
				quote = None;
			}
			continue;
		}
		match character {
			'/' if characters.peek() == Some(&'*') => {
				characters.next();
				let mut previous = '\0';
				for inside in characters.by_ref() {
					if previous == '*' && inside == '/' {
						break;
					}
					previous = inside;
				}
			}
			'"' | '\'' => {
				quote = Some(character);
				buffer.push(character);
			}
			'(' => {
				depth += 1;
				buffer.push(character);
			}
			')' => {
				depth = depth.saturating_sub(1);
				buffer.push(character);
			}
			'{' if depth == 0 => {
				stack.push(buffer.trim().to_owned());
				buffer.clear();
			}
			'}' if depth == 0 => {
				take(&buffer, &stack, &mut found);
				buffer.clear();
				stack.pop();
			}
			';' if depth == 0 => {
				take(&buffer, &stack, &mut found);
				buffer.clear();
			}
			_ => buffer.push(character),
		}
	}
	found
}

fn take(buffer: &str, stack: &[String], found: &mut Vec<Declaration>) {
	let Some((name, value)) = buffer.split_once(':') else {
		return;
	};
	let name = name.trim();
	if !name.starts_with("--") || name.len() <= 2 || stack.is_empty() {
		return;
	}
	let Some(scheme) = scope(stack) else {
		return;
	};
	let value = value.trim();
	let value = value
		.strip_suffix("!important")
		.map_or(value, str::trim_end)
		.to_owned();
	if value.is_empty() {
		return;
	}
	found.push(Declaration {
		name: name.to_owned(),
		value,
		scheme,
	});
}

fn scope(stack: &[String]) -> Option<Scheme> {
	let mut scheme = Scheme::Any;
	for selector in stack {
		let lower = selector.to_ascii_lowercase();
		if lower.starts_with('@') {
			if lower.contains("prefers-color-scheme") {
				scheme = if lower.contains("light") {
					Scheme::Light
				} else {
					Scheme::Dark
				};
			}
			continue;
		}
		if !is_global(&lower) {
			return None;
		}
		let light = lower.contains(".theme-light");
		let dark = lower.contains(".theme-dark") || lower.contains(".theme-midnight");
		match (light, dark) {
			(true, false) => scheme = Scheme::Light,
			(false, true) => scheme = Scheme::Dark,
			_ => {}
		}
	}
	Some(scheme)
}

fn is_global(selector: &str) -> bool {
	selector.split(',').all(|part| {
		let part = part.trim();
		!part.is_empty()
			&& part
				.split(|character: char| character.is_whitespace() || character == '>')
				.filter(|compound| !compound.is_empty())
				.all(is_global_compound)
	})
}

fn is_global_compound(compound: &str) -> bool {
	let compound = compound
		.trim_start_matches(":is(")
		.trim_start_matches(":where(")
		.trim_end_matches(')');
	let mut pieces = Vec::new();
	let mut start = 0;
	for (index, character) in compound.char_indices().skip(1) {
		if matches!(character, '.' | '#' | ':') && !compound[..index].ends_with(':') {
			pieces.push(&compound[start..index]);
			start = index;
		}
	}
	pieces.push(&compound[start..]);
	pieces.iter().all(|piece| {
		let piece = piece.trim_end_matches(')');
		piece.is_empty()
			|| GLOBAL_SELECTORS.contains(&piece)
			|| GLOBAL_PREFIXES.iter().any(|prefix| {
				piece.starts_with(prefix)
					&& piece[prefix.len()..].chars().all(|character| {
						character.is_ascii_alphanumeric() || character == '-' || character == '_'
					})
			})
	})
}
