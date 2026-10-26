use std::path::PathBuf;

use bridge::themes::{
	COLORS, CSS_MAPPING, CSS_SIZES, DEFAULT_THEME, DERIVED_COLORS, Layer, MOCKUP_ROLES, Palette, Scheme,
	ThemeSource, Themes, UNTOKENED, color_token, import_css, link_lines, link_urls, parse_color, parse_meta,
	set_theme_links, size_token, theme_link, toggle_theme,
};
use session::ThemeLinkCopy;
use store::ThemeSettings;

const VENCORD_THEME: &str = r#"/**
 * @name Midnight Ocean
 * @author someone
 * @description Deep blue with a teal accent.
 * @version 2.1.0
 * @source https://github.com/someone/ocean
 * @website https://someone.github.io/ocean
 */
@import url("https://fonts.googleapis.com/css2?family=Inter&display=swap");

:root {
	--accent-hue: 180;
	--accent: hsl(var(--accent-hue) 60% 40%);
	--background-primary: #0b1d2a;
	--background-secondary: rgb(8 22 33 / 100%);
	--brand-experiment: var(--accent);
	--text-normal: var(--undefined-variable, #e0f0ff);
	--text-muted: var(--undefined-variable);
	--background-tertiary: url("data:image/png;base64,AAAA;BBBB");
	--interactive-normal: not-a-colour;
	--text-link: #12345;
	--font-primary: "Inter", sans-serif;
	--background-floating: #101010 !important;
}

.theme-light {
	--background-primary: #f0f8ff;
	--brand-experiment: #007777;
}

.theme-dark, .theme-darker {
	--background-secondary-alt: #050f17;
}

[class*="sidebar"] {
	--background-secondary: #ff0000;
}

.app-mount .chat-area {
	background: red;
	color: blue;
}
"#;

const MIDNIGHT: &str = include_str!("fixtures/midnight-vencord.theme.css");

#[test]
fn the_midnight_flavour_imports_one_to_one_into_the_built_in_midnight_theme() {
	let meta = parse_meta(MIDNIGHT, "midnight-vencord.theme");
	assert_eq!(meta.name, "midnight (vencord)");
	assert_eq!(meta.author, "refact0r");
	assert_eq!(meta.version, "2.1.1");

	let layer = import_css(MIDNIGHT);
	for (token, _, variables) in CSS_MAPPING {
		let declared = variables
			.iter()
			.any(|variable| MIDNIGHT.contains(&format!("{variable}:")));
		let gradient = ["mention", "mention_hover", "reply", "reply_hover"].contains(token);
		if declared && !gradient {
			assert!(
				layer.colors().iter().any(|(name, _, _)| name == token),
				"{token} was not imported"
			);
		}
	}
	let shown = Palette::dark().layered([&layer]);
	assert_same_colours(&shown, &Palette::midnight());
	assert_eq!(shown.size("gap"), Some(12.));
	assert_eq!(shown.size("chatbar_height"), Some(47.));
	assert_eq!(shown.color("bg_4"), Some("#282828"));
	assert_eq!(shown.color("background_primary"), Some("#282828"));
	assert_eq!(shown.color("text_3"), Some("#d4be98"));
	assert_eq!(shown.color("accent_2"), Some("#d3869b"));
	assert_eq!(shown.color("brand"), Some("#d3869b"));
	assert_eq!(shown.color("mention"), Some("#d3869b1a"));
}

#[test]
fn every_variable_of_the_mockups_has_a_token_or_a_reason() {
	let root = mockup_root();
	let variables: Vec<&str> = root
		.split(';')
		.filter_map(|declaration| declaration.split_once(':'))
		.map(|(name, _)| name.rsplit("*/").next().unwrap_or("").trim())
		.map(|name| name.rsplit('{').next().unwrap_or("").trim())
		.filter(|name| name.starts_with("--"))
		.collect();
	assert!(variables.len() > 300, "only {} variables found", variables.len());
	for variable in variables {
		let colour = CSS_MAPPING
			.iter()
			.any(|(_, discord, midnight)| discord.contains(&variable) || midnight.contains(&variable));
		let size = CSS_SIZES.iter().any(|(_, name)| *name == variable);
		let role = MOCKUP_ROLES.iter().any(|(name, _)| *name == variable);
		let reason = UNTOKENED
			.iter()
			.any(|(name, _)| *name == variable || (name.ends_with('-') && variable.starts_with(name)));
		assert!(
			colour || size || role || reason,
			"{variable} has no token and no reason"
		);
	}
	for (variable, token) in MOCKUP_ROLES {
		assert!(
			color_token(token).is_some() || ["ui_font", "code_font"].contains(token),
			"{variable} names {token}, which is not a token"
		);
	}
	for (token, _) in CSS_SIZES {
		assert!(size_token(token).is_some(), "{token} is not a size token");
	}
}

#[test]
fn each_midnight_variable_lands_on_exactly_one_token() {
	let mut seen: Vec<&str> = Vec::new();
	for (_, _, midnight) in CSS_MAPPING {
		for variable in *midnight {
			assert!(!seen.contains(variable), "{variable} maps onto two tokens");
			seen.push(variable);
		}
	}
	for (_, variable) in CSS_SIZES {
		assert!(!seen.contains(variable), "{variable} maps onto two tokens");
		seen.push(variable);
	}
}

#[test]
fn the_mockups_variable_block_restyles_kyoko_exactly_as_midnight() {
	let layer = import_css(&mockup_root());
	let midnight = Palette::midnight();
	assert_same_colours(
		&Palette::builtin("light").expect("light").layered([&layer]),
		&midnight,
	);
	let shown = Palette::dark().layered([&layer]);
	assert_same_colours(&shown, &midnight);
	for (token, size) in [
		("guild_list_width", 66.),
		("channel_header_height", 50.),
		("chatbar_height", 47.),
		("channel_item_height", 36.),
		("member_list_panel_width", 264.),
		("radius_md", 12.),
		("space_xs", 8.),
	] {
		assert_eq!(shown.size(token), Some(size), "{token}");
	}
}

#[test]
fn the_midnight_theme_gives_every_token_a_value_from_its_own_palette() {
	let midnight = Palette::midnight();
	let on_light = Palette::builtin("light")
		.expect("light")
		.layered([&Layer::midnight()]);
	let on_amoled = Palette::builtin("amoled")
		.expect("amoled")
		.layered([&Layer::midnight()]);
	assert_same_colours(&on_light, &midnight);
	assert_same_colours(&on_amoled, &midnight);
	assert_eq!(midnight.colors().len(), COLORS.len());
	for (token, value) in [
		("bg_3", "#31302f"),
		("text_1", "#f1e9da"),
		("text_0", "#282828"),
		("accent_new", "#d3869b"),
		("online", "#a8b665"),
		("border", "#69656333"),
		("background_tertiary", "#31302f"),
		("interactive_active", "#f1e9da"),
		("danger", "#ea6962"),
		("mention_background", "#d3869b1a"),
		("background_feedback_critical", "#ea696233"),
		("overlay", "#000000b3"),
	] {
		assert_eq!(midnight.color(token), Some(value), "{token}");
	}
	assert_eq!(midnight.ui_fonts()[0], "Inter");
	assert_eq!(midnight.code_fonts()[0], "gg mono");
	assert!(!midnight.is_light());
}

#[test]
fn a_theme_that_changes_a_base_colour_carries_it_to_every_token_built_on_it() {
	let layer = Layer::from_json(
		"rose",
		r##"{ "purple_2": "#ff0088", "text_3": "#eeeeee", "link": "#00ff00", "gap": 8, "guildbar_avatar_size": 48 }"##,
	)
	.expect("theme");
	let shown = Palette::midnight().layered([&layer]);
	for token in [
		"accent_2",
		"accent_new",
		"streaming",
		"focus_ring",
		"mention_badge",
	] {
		assert_eq!(shown.color(token), Some("#ff0088"), "{token}");
	}
	assert_eq!(shown.color("mention"), Some("#ff00881a"));
	assert_eq!(shown.color("text_normal"), Some("#eeeeee"));
	assert_eq!(shown.color("link"), Some("#00ff00"));
	assert_eq!(shown.color("accent_1"), Palette::midnight().color("accent_1"));
	assert_eq!(shown.size("top_strip_height"), Some(8.));
	assert_eq!(shown.size("guild_list_width"), Some(74.));
	assert_eq!(shown.size("guild_sidebar_width"), Some(354.));
	assert_eq!(shown.size("channel_header_height"), Some(58.));
}

fn mockup_root() -> String {
	let path =
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/design/mockups/midnight-discord.html");
	let page = std::fs::read_to_string(path).expect("the mockup page");
	let start = page.find(":root {").expect("a :root block");
	let end = page[start..].find("\n}").expect("the end of :root") + start + 2;
	page[start..end].to_owned()
}

fn assert_same_colours(shown: &Palette, expected: &Palette) {
	let hex = |value: Option<&str>| value.and_then(parse_color).map(|color| color.hex());
	for (token, value) in expected.colors() {
		assert_eq!(hex(shown.color(token)), hex(Some(value)), "{token}");
	}
}

#[test]
fn every_mapped_variable_lands_on_a_real_colour_token() {
	for (token, discord, midnight) in CSS_MAPPING {
		assert!(color_token(token).is_some(), "{token} is not a colour token");
		assert!(
			!discord.is_empty() || !midnight.is_empty(),
			"{token} maps nothing"
		);
		for name in discord.iter().chain(midnight.iter()) {
			assert!(name.starts_with("--"), "{name}");
		}
	}
}

fn scratch(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join(format!("kyoko-bridge-themes-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).expect("scratch dir");
	dir
}

fn write(dir: &std::path::Path, file: &str, text: &str) {
	std::fs::write(dir.join(file), text).expect("theme file");
}

fn dark(themes: &Themes, settings: &ThemeSettings) -> Palette {
	palette(themes, settings, "dark")
}

fn palette(themes: &Themes, settings: &ThemeSettings, name: &str) -> Palette {
	themes
		.palettes(settings)
		.into_iter()
		.find(|palette| palette.name == name)
		.expect("the base palette")
}

fn enabled(ids: &[&str]) -> ThemeSettings {
	ThemeSettings {
		enabled: ids.iter().map(|id| (*id).to_owned()).collect(),
		links: Vec::new(),
	}
}

#[test]
fn a_vencord_theme_brings_only_its_colour_variables() {
	let layer = import_css(VENCORD_THEME);
	let shown = Palette::dark().layered([&layer]);
	let untouched = Palette::dark();

	assert_eq!(shown.color("background_primary"), Some("#0b1d2a"));
	assert_eq!(shown.color("background_secondary"), Some("#081621"));
	assert_eq!(shown.color("brand"), Some("#29a3a3"));
	assert_eq!(shown.color("text_normal"), Some("#e0f0ff"));
	assert_eq!(shown.color("background_floating"), Some("#101010"));
	assert_eq!(shown.color("user_panel_background"), Some("#050f17"));
	for skipped in [
		"text_muted",
		"background_tertiary",
		"interactive_normal",
		"link",
		"header_primary",
	] {
		assert_eq!(
			shown.color(skipped),
			untouched.color(skipped),
			"{skipped} changed"
		);
	}
	assert_eq!(shown.sizes(), untouched.sizes());
	assert_eq!(shown.ui_fonts(), untouched.ui_fonts());

	let light = Palette::builtin("light").expect("light").layered([&layer]);
	assert_eq!(light.color("background_primary"), Some("#f0f8ff"));
	assert_eq!(light.color("brand"), Some("#007777"));
	assert_eq!(light.color("user_panel_background"), light.color("bg_4"));
	assert_ne!(light.color("user_panel_background"), Some("#050f17"));
}

const DISCORD_THEME: &str = include_str!("fixtures/discord-variables.theme.css");

const ON_TWO_MIDNIGHT_TOKENS: &[&str] = &["--brand-experiment", "--brand-500", "--brand-experiment-500"];

fn hex(css: &str) -> String {
	parse_color(css).expect("a colour").hex()
}

#[test]
fn a_theme_of_discord_variables_recolours_the_midnight_shell() {
	let layer = import_css(DISCORD_THEME);
	let midnight = Palette::midnight();
	let shown = midnight.layered([&layer]);
	let brand = hex("hsl(190 70% 50%)");
	for (token, value) in [
		("bg_4", "#10202b".to_owned()),
		("bg_3", "#060d12".to_owned()),
		("text_1", "#ffffff".to_owned()),
		("text_2", "#f0f7fb".to_owned()),
		("text_3", "#d6e6ef".to_owned()),
		("text_4", "#98b1bf".to_owned()),
		("text_5", "#3c5260".to_owned()),
		("hover", hex("rgba(120, 170, 200, 0.12)")),
		("active", hex("rgba(120, 170, 200, 0.22)")),
		("active_2", hex("rgba(120, 170, 200, 0.3)")),
		("border", hex("rgba(120, 170, 200, 0.25)")),
		("accent_1", "#6fd3ef".to_owned()),
		("accent_2", brand.clone()),
		("accent_3", brand.clone()),
		("accent_4", hex("hsl(190 70% 42%)")),
		("online", "#4ccf8a".to_owned()),
	] {
		assert_eq!(shown.color(token), Some(value.as_str()), "{token}");
	}
	for (token, from) in [
		("text_0", "bg_4"),
		("border_light", "hover"),
		("border_hover", "active"),
		("accent_new", "accent_2"),
		("offline", "text_4"),
		("background_tertiary", "bg_3"),
		("interactive_hover", "text_3"),
		("header_primary", "text_2"),
	] {
		assert_eq!(shown.color(token), shown.color(from), "{token}");
	}
	let faded = |css: &str| parse_color(css).expect("a colour").faded(0.1).hex();
	assert_eq!(shown.color("mention"), Some(faded(&brand).as_str()));
	assert_eq!(shown.color("reply"), Some(faded("#d6e6ef").as_str()));
	assert_eq!(shown.color("streaming"), midnight.color("streaming"));
	assert_eq!(shown.color("red_2"), midnight.color("red_2"));
	assert_eq!(shown.color("background_floating"), Some("#0a151c"));

	assert_eq!(shown.color("background_primary"), Some("#10202b"));
	assert_eq!(shown.color("background_secondary"), Some("#0d1a23"));
	assert_eq!(shown.color("user_panel_background"), Some("#0a151c"));
	assert_eq!(shown.color("text_muted"), Some("#7d95a3"));
	assert_eq!(shown.color("header_secondary"), Some("#a9bfcc"));
	assert_eq!(shown.color("interactive_normal"), Some("#98b1bf"));
	assert_eq!(shown.color("brand"), Some(brand.as_str()));
	assert_eq!(shown.color("divider"), shown.color("border"));
	assert_eq!(shown.sizes(), midnight.sizes());
	assert_eq!(shown.ui_fonts(), midnight.ui_fonts());
}

#[test]
fn a_discord_theme_recolours_a_light_base_from_its_light_block_only() {
	let layer = import_css(DISCORD_THEME);
	let base = Palette::builtin("light").expect("light");
	let shown = base.layered([&layer]);
	for (token, value) in [
		("bg_4", "#fbfdfe"),
		("bg_3", "#dde8ee"),
		("text_2", "#0b1419"),
		("text_3", "#1d2b33"),
		("text_4", "#4a5f6b"),
		("accent_2", "#1c8fae"),
		("accent_3", "#1c8fae"),
		("background_primary", "#fbfdfe"),
		("background_secondary", "#eef4f7"),
		("text_muted", "#5d7280"),
		("user_panel_background", "#fbfdfe"),
		("text_0", "#fbfdfe"),
	] {
		assert_eq!(shown.color(token), Some(value), "{token}");
	}
	assert_eq!(shown.color("hover"), Some(hex("rgba(16, 32, 43, 0.06)").as_str()));
	assert_eq!(
		shown.color("active"),
		Some(hex("rgba(16, 32, 43, 0.12)").as_str())
	);
	assert_eq!(shown.color("border"), shown.color("active"));
	assert!(shown.is_light());
	for token in ["text_1", "text_5", "active_2", "accent_1", "online"] {
		assert_eq!(shown.color(token), base.color(token), "{token} took a dark value");
	}
}

#[test]
fn a_discord_variable_lands_on_one_midnight_token_and_what_is_built_on_it() {
	let is_midnight = |midnight: &[&str]| !midnight.is_empty();
	for (_, discord, midnight) in CSS_MAPPING {
		if !is_midnight(midnight) {
			continue;
		}
		for variable in *discord {
			let targets: Vec<&str> = CSS_MAPPING
				.iter()
				.filter(|(_, names, midnight)| is_midnight(midnight) && names.contains(variable))
				.map(|(token, _, _)| *token)
				.collect();
			if ON_TWO_MIDNIGHT_TOKENS.contains(variable) {
				assert_eq!(targets, ["accent_2", "accent_3"], "{variable}");
			} else {
				assert_eq!(targets.len(), 1, "{variable} lands on {targets:?}");
			}
			for (other, names, midnight) in CSS_MAPPING {
				if is_midnight(midnight) || !names.contains(variable) {
					continue;
				}
				assert!(
					DERIVED_COLORS
						.iter()
						.any(|(target, source, alpha)| target == other
							&& targets.contains(source)
							&& *alpha == 1.),
					"{variable} lands on {other}, which is not built on {targets:?}"
				);
			}
		}
	}
}

#[test]
fn a_theme_that_is_not_css_at_all_imports_nothing_and_never_fails() {
	for text in [
		"",
		"<!DOCTYPE html><html><body>404</body></html>",
		"{{{{ ;;;; }}}} --background-primary: #fff",
		":root { --background-primary: var(--a); --a: var(--background-primary); }",
		":root { --background-primary: #ffffff",
		"\u{0}\u{1}\u{2}",
	] {
		assert!(import_css(text).is_empty(), "{text:?} imported something");
	}
}

#[test]
fn theme_details_come_from_the_first_doc_comment_and_default_to_the_file_name() {
	let meta = parse_meta(VENCORD_THEME, "ocean");
	assert_eq!(meta.name, "Midnight Ocean");
	assert_eq!(meta.author, "someone");
	assert_eq!(meta.description, "Deep blue with a teal accent.");
	assert_eq!(meta.version, "2.1.0");
	assert_eq!(meta.source, "https://github.com/someone/ocean");
	assert_eq!(meta.website, "https://someone.github.io/ocean");

	let bare = parse_meta("/* @name not a doc comment */ :root {}", "nord");
	assert_eq!(bare.name, "nord");
	assert_eq!(bare.author, "");

	let later = parse_meta("/** @author first */\n/** @name second */", "fallback");
	assert_eq!(later.name, "fallback");
	assert_eq!(later.author, "first");
}

#[test]
fn theme_links_drop_blanks_and_duplicates_and_keep_the_light_and_dark_prefix() {
	let lines = link_lines(
		"https://a.example/one.css\n\n   \nhttps://a.example/one.css\n  @light https://a.example/two.css  \n@dark https://a.example/three.css\n@lightish https://a.example/four.css\n",
	);
	assert_eq!(
		lines,
		[
			"https://a.example/one.css",
			"@light https://a.example/two.css",
			"@dark https://a.example/three.css",
			"@lightish https://a.example/four.css",
		]
	);
	assert_eq!(theme_link(&lines[0]).scheme, Scheme::Any);
	assert_eq!(theme_link(&lines[1]).scheme, Scheme::Light);
	assert_eq!(theme_link(&lines[1]).url, "https://a.example/two.css");
	assert_eq!(theme_link(&lines[2]).scheme, Scheme::Dark);
	assert_eq!(theme_link(&lines[3]).scheme, Scheme::Any);
	assert_eq!(link_urls(&lines).len(), 4);

	let mut settings = ThemeSettings::default();
	assert!(set_theme_links(
		&mut settings,
		"https://a.example/one.css\nhttps://a.example/one.css"
	));
	assert_eq!(settings.links, ["https://a.example/one.css"]);
	assert!(!set_theme_links(&mut settings, "\nhttps://a.example/one.css\n"));
}

#[test]
fn enabled_themes_apply_in_order_over_the_chosen_base_and_turning_one_off_restores_it() {
	let dir = scratch("order");
	write(
		&dir,
		"red.css",
		"/** @name Red */ :root { --brand-experiment: #ff0000; --background-primary: #200000; }",
	);
	write(
		&dir,
		"blue.json",
		r##"{ "name": "Blue", "author": "Kyoko", "brand": "#0000ff", "radius_md": 12 }"##,
	);
	let themes = Themes::load(&[], dir.clone());
	let base = Palette::dark();

	let both = dark(&themes, &enabled(&["red.css", "blue.json"]));
	assert_eq!(both.color("brand"), Some("#0000ff"));
	assert_eq!(both.color("background_primary"), Some("#200000"));
	assert!(both.sizes().contains(&("radius_md", 12.)));

	let reversed = dark(&themes, &enabled(&["blue.json", "red.css"]));
	assert_eq!(reversed.color("brand"), Some("#ff0000"));

	let mut settings = enabled(&["red.css", "blue.json"]);
	assert!(toggle_theme(&mut settings, "blue.json", false));
	let red_only = dark(&themes, &settings);
	assert_eq!(red_only.color("brand"), Some("#ff0000"));
	assert!(red_only.sizes().contains(&("radius_md", 12.)));

	assert!(toggle_theme(&mut settings, "red.css", false));
	assert_eq!(dark(&themes, &settings), base);
	assert!(!toggle_theme(&mut settings, "red.css", false));

	let light = palette(&themes, &enabled(&["red.css"]), "light");
	assert_eq!(light.color("brand"), Some("#ff0000"));
	assert_eq!(
		light.color("text_normal"),
		Palette::builtin("light").expect("light").color("text_normal")
	);
	let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn online_themes_follow_local_ones_and_their_prefix_picks_light_or_dark() {
	let dir = scratch("online");
	write(&dir, "red.css", ":root { --brand-experiment: #ff0000; }");
	let mut themes = Themes::load(&[], dir.clone());
	let lines = vec![
		"@dark https://a.example/green.css".to_owned(),
		"https://a.example/broken.css".to_owned(),
		"https://a.example/purple.json".to_owned(),
	];
	let copies = [
		ThemeLinkCopy {
			url: "https://a.example/green.css".to_owned(),
			text: Some("/** @name Green */ :root { --brand-experiment: #00ff00; }".to_owned()),
			error: None,
			cached: false,
		},
		ThemeLinkCopy {
			url: "https://a.example/broken.css".to_owned(),
			text: None,
			error: Some("the server answered 404 Not Found".to_owned()),
			cached: false,
		},
		ThemeLinkCopy {
			url: "https://a.example/purple.json".to_owned(),
			text: Some(r##"{ "link": "#aa00ff" }"##.to_owned()),
			error: Some("the server took too long to answer".to_owned()),
			cached: true,
		},
	];
	themes.set_online(&lines, &copies);
	let settings = ThemeSettings {
		enabled: vec!["red.css".to_owned()],
		links: lines.clone(),
	};

	let shown = dark(&themes, &settings);
	assert_eq!(shown.color("brand"), Some("#00ff00"));
	assert_eq!(shown.color("link"), Some("#aa00ff"));
	let light = palette(&themes, &settings, "light");
	assert_eq!(light.color("brand"), Some("#ff0000"));
	assert_eq!(light.color("link"), Some("#aa00ff"));

	let entries = themes.entries(&settings, "dark");
	let online: Vec<_> = entries
		.iter()
		.filter(|entry| entry.source == ThemeSource::Online)
		.collect();
	assert_eq!(online.len(), 3);
	assert_eq!(online[0].meta.name, "Green");
	assert_eq!(online[0].scheme, Scheme::Dark);
	assert!(online[0].enabled);
	assert_eq!(online[1].error, "the server answered 404 Not Found");
	assert!(!online[1].enabled);
	assert_eq!(online[2].error, "the server took too long to answer");
	assert_eq!(online[2].note, "Using the copy saved last time.");
	assert!(online[2].enabled);
	let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_themes_folder_lists_every_theme_and_a_broken_file_never_breaks_the_rest() {
	let dir = scratch("folder");
	write(
		&dir,
		"good.css",
		"/**
 * @name Good
 * @version 1.0
 */ :root { --brand-experiment: #123456; }",
	);
	write(&dir, "broken.json", "{ not json");
	write(&dir, "huge.css", &"a".repeat(session::MAX_THEME_BYTES + 1));
	write(&dir, "notes.txt", "not a theme");
	std::fs::create_dir_all(dir.join("sunset")).expect("theme folder");
	write(
		&dir.join("sunset"),
		"theme.json",
		r##"{ "base": "light", "brand": "#ff8800" }"##,
	);
	write(
		&dir.join("sunset"),
		"theme.toml",
		"id = \"sunset\"\nname = \"Sunset\"\nversion = \"0.3.0\"\nauthor = \"Example\"\ndescription = \"Warm.\"\n",
	);
	let mut themes = Themes::load(&[], dir.clone());
	let settings = enabled(&["good.css", "broken.json", "huge.css", "sunset"]);
	let entries = themes.entries(&settings, "dark");
	let local: Vec<_> = entries
		.iter()
		.filter(|entry| entry.source == ThemeSource::Local)
		.map(|entry| (entry.id.as_str(), entry.enabled, entry.error.is_empty()))
		.collect();
	assert_eq!(
		local,
		[
			("broken.json", false, false),
			("good.css", true, true),
			("huge.css", false, false),
			("sunset", true, true),
		]
	);
	let sunset = entries.iter().find(|entry| entry.id == "sunset").expect("sunset");
	assert_eq!(sunset.meta.name, "Sunset");
	assert_eq!(sunset.meta.version, "0.3.0");
	assert_eq!(dark(&themes, &settings).color("brand"), Some("#ff8800"));

	assert!(!themes.rescan());
	write(&dir, "late.css", ":root { --text-link: #abcdef; }");
	assert!(themes.rescan());
	assert!(themes.has_local("late.css"));
	std::fs::remove_file(dir.join("late.css")).expect("remove");
	assert!(themes.rescan());
	assert!(!themes.has_local("late.css"));
	let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_bundled_themes_are_bases_with_their_details() {
	let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
	let themes = Themes::load(&[bundled], PathBuf::new());
	let names = themes.base_names();
	assert_eq!(names[..4], ["midnight", "dark", "light", "amoled"]);
	assert!(names.contains(&"deep-blue".to_owned()));
	let entries = themes.entries(&ThemeSettings::default(), "deep-blue");
	let deep_blue = entries
		.iter()
		.find(|entry| entry.id == "deep-blue")
		.expect("deep-blue");
	assert!(deep_blue.enabled);
	assert_eq!(deep_blue.meta.name, "Deep Blue");
	assert_eq!(deep_blue.source, ThemeSource::Builtin);
	assert!(!deep_blue.meta.author.is_empty());
	assert!(!deep_blue.meta.version.is_empty());
	assert_eq!(deep_blue.swatch.len(), 6);
	assert_eq!(
		themes
			.base("deep-blue")
			.and_then(|palette| palette.color("background_primary")),
		Some("#0f1020")
	);
}

#[test]
fn midnight_is_the_default_theme_and_credits_refact0r() {
	assert_eq!(DEFAULT_THEME, "midnight");
	assert_eq!(store::Settings::default().theme, DEFAULT_THEME);
	let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
	let themes = Themes::load(&[bundled], PathBuf::new());
	let entries = themes.entries(&ThemeSettings::default(), DEFAULT_THEME);
	let midnight = &entries[0];
	assert_eq!(midnight.id, "midnight");
	assert_eq!(midnight.meta.name, "Midnight");
	assert_eq!(midnight.meta.author, "refact0r");
	assert_eq!(midnight.meta.version, "2.1.1");
	assert!(midnight.enabled);
	assert_eq!(themes.base(DEFAULT_THEME), Some(&Palette::midnight()));
	assert_eq!(Palette::builtin(DEFAULT_THEME), Some(Palette::midnight()));
}
