use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bridge::Row;
use bridge::components::{FieldKind, ModalField, ModalForm};
use media::{Gallery, Limits, MediaFetcher, MediaService};
use protocol::{ComponentType, Message, ModalApplication, Snowflake};
use serde_json::json;
use session::{Command, ModalValue};
use state::{ModalStatus, State};
use zgx::compose::prelude::*;
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{Prefs, Setup, TimelineView, rows};

const DOCS: &str = "https://example.com/docs/components";
const TITLE: &str = "Reaction tooltip wraps to three lines";

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn entry() -> bridge::MessageEntry {
	let message: Message = serde_json::from_value(json!({
		"id": "100",
		"channel_id": "7",
		"author": { "id": "9100000000000000001", "username": "mockbot", "bot": true },
		"content": "Pick an action",
		"timestamp": "2026-10-01T12:00:00+00:00",
		"components": [
			{ "type": 1, "components": [
				{ "type": 2, "style": 1, "label": "Primary", "custom_id": "primary" },
				{ "type": 2, "style": 5, "label": "Docs", "url": DOCS },
			]},
			{ "type": 42, "custom_id": "from-the-future" },
		],
	}))
	.expect("the message decodes");
	bridge::detached_entry(&State::new(), None, &Arc::new(message))
}

fn form() -> ModalForm {
	let field = |custom_id: &str, label: &str, kind: FieldKind| ModalField {
		custom_id: custom_id.to_owned(),
		label: label.to_owned(),
		description: None,
		required: true,
		kind,
		placeholder: None,
		min_length: None,
		max_length: Some(4000),
		value: None,
	};
	ModalForm {
		serial: 1,
		title: "Report a bug".to_owned(),
		application: ModalApplication {
			name: "Mockbot".to_owned(),
			..ModalApplication::default()
		},
		fields: vec![
			field("title", "Title", FieldKind::Short),
			field("details", "What happened?", FieldKind::Paragraph),
		],
		status: ModalStatus::Open,
		missing: Vec::new(),
	}
}

#[test]
fn a_link_button_asks_before_leaving_and_the_app_modal_sits_on_top_taking_input() {
	let tokio = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let handle = tokio.handle().clone();
	let row = Rc::new(Row::Message(Box::new(entry())));
	let sent: Rc<RefCell<Vec<Command>>> = Rc::default();
	let heard = sent.clone();
	let timeline: Rc<RefCell<Option<Rc<TimelineView>>>> = Rc::default();
	let kept = timeline.clone();
	let built: Rc<RefCell<Option<View>>> = Rc::default();
	let into = built.clone();
	let mut app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let fetcher = MediaFetcher::new().expect("a media fetcher");
			let service = MediaService::new(fetcher, scratch("components-media"), Limits::default());
			let view = TimelineView::new(
				runtime,
				Setup {
					gallery: Gallery::new(service, handle),
					cdn: media::cdn::CDN.to_owned(),
					mock_origin: None,
					code_font: "Consolas".to_owned(),
					prefs: Prefs::default(),
					store: None,
					developer_mode: false,
					pictures: None,
					display: bridge::Display::default(),
				},
			);
			view.on_command(move |command| heard.borrow_mut().push(command));
			let (env, hovered, bump) = (view.env.clone(), runtime.signal(false), runtime.signal(0u32));
			let watched = bump.clone();
			*into.borrow_mut() = Some(
				div()
					.w(640.)
					.h(480.)
					.child(switch(
						move || watched.get(),
						move |_: u32, _| rows::render(&row, &env, &hovered, &bump),
					))
					.child(view.env.links.view())
					.child(view.app_modal.view()),
			);
			*kept.borrow_mut() = Some(view);
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| built.borrow_mut().take().unwrap_or_else(div),
	);
	let primary = "component-100-primary";
	let docs = format!("component-100-{DOCS}");
	assert!(
		app.wait_until(|app| app.find(primary).is_some() && app.find(&docs).is_some()),
		"the buttons are laid out and the unknown component is skipped"
	);

	app.click(primary);
	assert!(
		matches!(
			sent.borrow().as_slice(),
			[Command::PressComponent { channel_id, message_id, custom_id, kind: ComponentType::Button, values: None }]
				if *channel_id == Snowflake(7) && *message_id == Snowflake(100) && custom_id == "primary"
		),
		"pressing a button sends the component interaction"
	);

	assert!(app.find("leave-kyoko").is_none());
	app.click(&docs);
	assert!(
		app.wait_until(|app| app.find("leave-kyoko").is_some()),
		"a link button goes through the Leave Kyoko? check"
	);
	assert_eq!(sent.borrow().len(), 1, "a link button never reaches Discord");
	app.key(zgx::input::Key::Escape);
	app.frames(2);

	let view = timeline.borrow().clone().expect("the timeline view");
	view.set_modal(Some(form()));
	let field = "app-modal-field-title";
	assert!(
		app.wait_until(|app| app.find(field).is_some() && app.bounds(field).width > 0.),
		"the app modal opens"
	);
	app.ui.dispatch(zgx::input::InputEvent::Text(TITLE.into()));
	app.settle();
	assert!(
		!sent
			.borrow()
			.iter()
			.any(|command| matches!(command, Command::CloseModal)),
		"typing does not reach anything under the modal"
	);
	view.submit_modal();
	let submitted = sent.borrow().iter().find_map(|command| match command {
		Command::SubmitModal { answers } => Some(answers.clone()),
		_ => None,
	});
	let answers = submitted.expect("Submit sends the modal");
	let title = answers
		.iter()
		.find(|answer| answer.custom_id == "title")
		.map(|answer| answer.value.clone());
	assert_eq!(title, Some(ModalValue::Text(TITLE.to_owned())));
}
