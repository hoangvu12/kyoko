use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use zgx::input::InputEvent;
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{DropTarget, DropZones, FileDrag, accept_files};
use crate::incoming::{MAX_READ_BYTES, read_upload};

const WINDOW: (f32, f32) = (1280., 800.);
const THREAD_LEFT: f32 = 860.;
const CHAT: (f32, f32) = (420., 400.);
const THREAD: (f32, f32) = (1000., 400.);
const DISCORD_LIMIT: u64 = 10 * 1024 * 1024;
const NITRO_LIMIT: u64 = 1024 * 1024 * 1024;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89";

type Heard = Rc<RefCell<Vec<FileDrag>>>;

fn scratch(name: &str) -> PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	std::fs::create_dir_all(&root).expect("scratch folder");
	root
}

fn window(zones: Rc<Cell<DropZones>>, heard: &Heard) -> TestApp {
	let heard = heard.clone();
	let app = TestApp::wrapped(
		|runtime| {
			ScriptHost::inline(
				runtime,
				"app",
				"return ui.div { id = \"window\", w = 1280, h = 800 }",
			)
		},
		move |view| {
			accept_files(
				view,
				move || zones.get(),
				move |drag| heard.borrow_mut().push(drag),
			)
		},
	);
	app.ui.scene.borrow_mut().resize(WINDOW.0, WINDOW.1);
	app.frames(2);
	app
}

fn hover(app: &mut TestApp, (x, y): (f32, f32)) {
	app.ui.dispatch(InputEvent::FileHover {
		x,
		y,
		path: PathBuf::from("cat.png"),
	});
	app.frames(1);
}

fn drop_at(app: &mut TestApp, (x, y): (f32, f32), paths: &[PathBuf]) {
	app.ui.dispatch(InputEvent::FilesDrop {
		x,
		y,
		paths: Arc::from(paths.to_vec()),
	});
	app.frames(1);
}

fn taken(heard: &Heard) -> Vec<FileDrag> {
	std::mem::take(&mut *heard.borrow_mut())
}

fn sparse(path: &Path, size: u64) {
	let file = std::fs::File::create(path).expect("a large file");
	file.set_len(size).expect("its length");
}

#[test]
fn files_dragged_over_the_chat_or_the_thread_panel_go_to_that_composer_and_are_read_within_the_caps() {
	let folder = scratch("drops");
	let picture = folder.join("cat.png");
	std::fs::write(&picture, PNG).expect("a picture");
	let clip = folder.join("clip.MOV");
	std::fs::write(&clip, b"not really a movie").expect("a clip");
	let huge = folder.join("huge.mkv");
	sparse(&huge, MAX_READ_BYTES + 1);
	let zones = Rc::new(Cell::new(DropZones {
		thread_left: Some(THREAD_LEFT),
		scale: 1.,
		..DropZones::default()
	}));
	let heard = Heard::default();
	let mut app = window(zones.clone(), &heard);
	hover(&mut app, CHAT);
	hover(&mut app, THREAD);
	app.ui.dispatch(InputEvent::FileHoverCancelled);
	app.frames(1);
	assert_eq!(
		taken(&heard),
		[
			FileDrag::Over(DropTarget::Main),
			FileDrag::Over(DropTarget::Thread),
			FileDrag::Left
		]
	);
	let dropped = vec![picture.clone(), clip.clone(), huge.clone(), folder.clone()];
	drop_at(&mut app, THREAD, &dropped);
	assert_eq!(
		taken(&heard),
		[
			FileDrag::Left,
			FileDrag::Dropped(DropTarget::Thread, dropped.clone())
		]
	);
	let read: Vec<Result<(String, String), String>> = dropped
		.iter()
		.map(|path| read_upload(path, NITRO_LIMIT).map(|file| (file.name, file.content_type)))
		.collect();
	assert_eq!(read[0], Ok(("cat.png".to_owned(), "image/png".to_owned())));
	assert_eq!(read[1], Ok(("clip.MOV".to_owned(), "video/quicktime".to_owned())));
	assert!(
		read[2]
			.as_ref()
			.is_err_and(|refusal| refusal.starts_with("huge.mkv is too big")),
		"{:?}",
		read[2]
	);
	assert!(
		read[3]
			.as_ref()
			.is_err_and(|refusal| refusal.ends_with("is a folder"))
	);
	let refused = read_upload(&clip, 4).expect_err("Discord's own limit comes first");
	assert!(refused.starts_with("clip.MOV is too big"), "{refused}");
	assert!(read_upload(&picture, DISCORD_LIMIT).is_ok());
	drop_at(&mut app, CHAT, &dropped[..1]);
	assert_eq!(
		taken(&heard).pop(),
		Some(FileDrag::Dropped(DropTarget::Main, dropped[..1].to_vec()))
	);
	let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_forum_takes_drops_into_its_post_and_the_call_view_into_its_chat_unless_over_the_thread() {
	let zones = Rc::new(Cell::new(DropZones {
		forum: true,
		scale: 1.,
		..DropZones::default()
	}));
	let heard = Heard::default();
	let mut app = window(zones.clone(), &heard);
	let file = vec![PathBuf::from("notes.txt")];
	drop_at(&mut app, CHAT, &file);
	assert_eq!(
		taken(&heard).pop(),
		Some(FileDrag::Dropped(DropTarget::ForumPost, file.clone()))
	);
	zones.set(DropZones {
		call: true,
		thread_left: Some(THREAD_LEFT),
		scale: 1.,
		..DropZones::default()
	});
	hover(&mut app, CHAT);
	drop_at(&mut app, CHAT, &file);
	drop_at(&mut app, THREAD, &file);
	assert_eq!(
		taken(&heard),
		[
			FileDrag::Over(DropTarget::VoiceChat),
			FileDrag::Left,
			FileDrag::Dropped(DropTarget::VoiceChat, file.clone()),
			FileDrag::Left,
			FileDrag::Dropped(DropTarget::Thread, file.clone()),
		]
	);
}
