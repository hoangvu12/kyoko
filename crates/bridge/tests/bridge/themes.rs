use std::path::PathBuf;

use bridge::themes::{
	CSS_MAPPING, Palette, Scheme, ThemeSource, Themes, color_token, import_css, link_lines, link_urls,
	parse_meta, set_theme_links, theme_link, toggle_theme,
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
fn a_midnight_theme_maps_its_role_variables_onto_the_nearest_tokens() {
	let meta = parse_meta(MIDNIGHT, "midnight-vencord.theme");
	assert_eq!(meta.name, "midnight (vencord)");
	assert_eq!(meta.author, "refact0r");
	assert_eq!(meta.version, "2.1.1");

	let shown = Palette::dark().layered([&import_css(MIDNIGHT)]);
	for (token, value) in [
		("background_primary", "#282828"),
		("background_secondary", "#282828"),
		("user_panel_background", "#282828"),
		("background_tertiary", "#31302f"),
		("background_floating", "#31302f"),
		("input_background", "#31302f"),
		("code_background", "#31302f"),
		("button_secondary", "#3c3836"),
		("button_secondary_hover", "#433f3d"),
		("scrollbar_thumb", "#433f3d"),
		("modifier_hover", "#6965631a"),
		("modifier_selected", "#69656333"),
		("modifier_active", "#6965634c"),
		("divider", "#69656333"),
		("message_hover", "#0000001a"),
		("text_on_brand", "#282828"),
		("interactive_active", "#f1e9da"),
		("header_primary", "#e4d2b4"),
		("text_normal", "#d4be98"),
		("interactive_hover", "#d4be98"),
		("text_muted", "#a79a83"),
		("header_secondary", "#a79a83"),
		("interactive_normal", "#a79a83"),
		("interactive_muted", "#d4be9866"),
		("link", "#db9aab"),
		("mention_text", "#db9aab"),
		("unread_pill", "#d3869b"),
		("mention_badge", "#d3869b"),
		("brand", "#d3869b"),
		("brand_hover", "#cd748c"),
		("status_online", "#a8b665"),
		("status_idle", "#d8a656"),
		("status_dnd", "#ea6962"),
		("status_offline", "#a79a83"),
		("status_streaming", "#d3869b"),
		("danger", "#ea6962"),
		("danger_hover", "#e7534b"),
		("success", "#a8b665"),
		("warning", "#d8a656"),
	] {
		assert_eq!(shown.color(token), Some(value), "{token}");
	}
	assert_eq!(
		shown.color("mention_background"),
		Palette::dark().color("mention_background")
	);
	assert_eq!(shown.sizes(), Palette::dark().sizes());
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
	assert_eq!(
		light.color("user_panel_background"),
		Palette::builtin("light")
			.expect("light")
			.color("user_panel_background")
	);
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
	assert!(red_only.sizes().contains(&("radius_md", 8.)));

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
	assert_eq!(names[..3], ["dark", "light", "amoled"]);
	assert!(names.contains(&"midnight".to_owned()));
	let entries = themes.entries(&ThemeSettings::default(), "midnight");
	let midnight = entries
		.iter()
		.find(|entry| entry.id == "midnight")
		.expect("midnight");
	assert!(midnight.enabled);
	assert_eq!(midnight.source, ThemeSource::Builtin);
	assert!(!midnight.meta.author.is_empty());
	assert!(!midnight.meta.version.is_empty());
	assert_eq!(midnight.swatch.len(), 6);
}
