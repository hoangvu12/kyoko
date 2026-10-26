use serde_json::{Value, json};

pub const BUTTONS_PROMPT: &str = "Pick an action";
pub const FRUIT_PROMPT: &str = "What's your favourite fruit?";
pub const FRUITS_PROMPT: &str = "Which fruits do you like?";
pub const RELEASE_TITLE: &str = "Midnight 2.1.1 released";
pub const DOCS_URL: &str = "https://example.com/docs/components";
pub const CHANGELOG_URL: &str = "https://example.com/midnight/changelog";
pub const PRESSED: &str = "You pressed";
pub const BUG_MODAL: &str = "bug_report";
pub const BUG_TITLE: &str = "Report a bug";
pub const BUG_FILED: &str = "Thanks, filed:";
pub const FRUITS: [(&str, &str, &str); 4] = [
	("Apple", "\u{1f34e}", "Crisp and sweet"),
	("Banana", "\u{1f34c}", "Soft, good in smoothies"),
	("Grapes", "\u{1f347}", "Small and shareable"),
	("Orange", "\u{1f34a}", "Juicy citrus"),
];
pub const SEVERITIES: [&str; 3] = ["Low", "Medium", "High"];

fn button(style: i64, label: &str, custom_id: &str) -> Value {
	json!({ "type": 2, "style": style, "label": label, "custom_id": custom_id })
}

fn row(components: Vec<Value>) -> Value {
	json!({ "type": 1, "components": components })
}

pub fn buttons() -> Vec<Value> {
	vec![
		row(vec![
			button(1, "Primary", "primary"),
			button(2, "Secondary", "secondary"),
			button(3, "Success", "success"),
			button(4, "Danger", "danger"),
		]),
		row(vec![
			json!({ "type": 2, "style": 5, "label": "Docs", "url": DOCS_URL }),
		]),
		row(vec![
			json!({
				"type": 2,
				"style": 2,
				"label": "Celebrate",
				"custom_id": "celebrate",
				"emoji": { "name": "\u{1f389}" },
			}),
			json!({ "type": 2, "style": 1, "label": "Disabled", "custom_id": "disabled", "disabled": true }),
			button(1, "Refresh", "refresh"),
		]),
	]
}

pub fn fruit_select(custom_id: &str, multi: bool) -> Vec<Value> {
	let options: Vec<Value> = FRUITS
		.iter()
		.map(|(label, emoji, description)| {
			json!({
				"label": label,
				"value": label.to_lowercase(),
				"description": description,
				"emoji": { "name": emoji },
			})
		})
		.collect();
	let mut select = json!({
		"type": 3,
		"custom_id": custom_id,
		"placeholder": "Make a selection",
		"options": options,
	});
	if multi {
		select["min_values"] = json!(1);
		select["max_values"] = json!(FRUITS.len());
	}
	vec![row(vec![select])]
}

pub fn release(thumbnail: &str, gallery: [&str; 3]) -> Vec<Value> {
	let media = |url: &str| json!({ "url": url, "width": 640, "height": 480, "content_type": "image/png" });
	vec![json!({
		"type": 17,
		"accent_color": 0xd47d9b,
		"components": [
			{
				"type": 10,
				"content": format!("## {RELEASE_TITLE}\nFixes the **picker shadow** and moves reaction chips onto `--reaction-*` tokens. [Release notes]({CHANGELOG_URL})"),
			},
			{
				"type": 9,
				"components": [{
					"type": 10,
					"content": "**refact0r** \u{b7} theme author\nMaintains midnight and its Vencord flavour.\nFeedback goes in #theme-support.",
				}],
				"accessory": { "type": 11, "media": media(thumbnail), "description": "refact0r" },
			},
			{ "type": 14, "divider": true, "spacing": 2 },
			{
				"type": 12,
				"items": gallery.iter().map(|url| json!({ "media": media(url) })).collect::<Vec<_>>(),
			},
			{
				"type": 9,
				"components": [{ "type": 10, "content": "Read the full changelog on GitHub." }],
				"accessory": { "type": 2, "style": 5, "label": "Changelog", "url": CHANGELOG_URL },
			},
		],
	})]
}

fn label(text: &str, component: Value) -> Value {
	json!({ "type": 18, "label": text, "component": component })
}

pub fn bug_modal() -> Vec<Value> {
	vec![
		label(
			"Title",
			json!({ "type": 4, "custom_id": "title", "style": 1, "max_length": 100, "required": true }),
		),
		label(
			"What happened?",
			json!({
				"type": 4,
				"custom_id": "details",
				"style": 2,
				"max_length": 4000,
				"required": true,
				"placeholder": "Steps to reproduce, what you expected",
			}),
		),
		label(
			"Severity",
			json!({
				"type": 3,
				"custom_id": "severity",
				"required": false,
				"placeholder": "Make a selection",
				"options": SEVERITIES
					.iter()
					.map(|name| json!({ "label": name, "value": name.to_lowercase() }))
					.collect::<Vec<_>>(),
			}),
		),
	]
}

pub fn submitted_text(components: &Value, custom_id: &str) -> Option<String> {
	let list = components.as_array()?;
	list.iter().find_map(|component| {
		if component["custom_id"].as_str() == Some(custom_id) {
			return component["value"].as_str().map(str::to_owned);
		}
		let wrapped = component["component"]
			.is_object()
			.then(|| json!([component["component"]]));
		submitted_text(&component["components"], custom_id)
			.or_else(|| submitted_text(wrapped.as_ref()?, custom_id))
	})
}
