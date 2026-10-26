use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{Composer, FileEdit, Inbox, Mark, Notice, OpenChannel, Outcome, Suggestion, Visual};
use chrono::Utc;
use protocol::{Nonce, Snowflake};
use rest::UploadFile;
use state::{Changes, State};
use tokio::runtime::Handle;
use zgx::image::ImageData;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record, snowflake, text, texts};
use crate::capture;
use crate::composer_layer::{PlusMenu, Request};

const DRAFT_DELAY: Duration = Duration::from_millis(500);
const TICK: Duration = Duration::from_millis(250);
const SUGGESTION_POINTS: f64 = 24.;
const THUMBNAIL_PIXELS: u32 = 400;
const WAITING: &str = "Waiting for connection...";
type Handler = fn(&Link, Pane);

const CAPTURE_DRAFT: &str = "Checking how the composer looks with a draft in it";
const CAPTURE_EMOJI_QUERY: &str = "wow :sm";

pub(super) use crate::composer_layer::Pane;

const CAPTURE_UPLOADS: usize = 3;
const CAPTURE_VIDEO_BYTES: usize = 4 * 1024 * 1024;
const CAPTURE_PICTURE: (u32, u32) = (320, 240);
const TRAY_INSET: f32 = 17.;
const CARD: f32 = 216.;
const CARD_GAP: f32 = 8.;

fn over_limit(limit: usize) -> String {
	let filler_words = "thanks for testing the pickers\n";
	let wanted = limit + 12;
	let mut draft = String::with_capacity(wanted);
	while draft.chars().count() + capture::MARKDOWN_DRAFT.chars().count() + 1 < wanted {
		draft.push_str(filler_words);
	}
	draft.truncate(wanted - capture::MARKDOWN_DRAFT.chars().count() - 1);
	draft.push('\n');
	draft.push_str(capture::MARKDOWN_DRAFT);
	draft
}

fn capture_picture() -> Loaded {
	let (width, height) = CAPTURE_PICTURE;
	let picture = image::RgbaImage::from_fn(width, height, |x, y| {
		let across = (x * 255 / width) as u8;
		let down = (y * 255 / height) as u8;
		image::Rgba([120 - across / 4, 80 + down / 3, 200, 255])
	});
	let mut encoded = Cursor::new(Vec::new());
	if picture.write_to(&mut encoded, image::ImageFormat::Png).is_err() {
		return Loaded::Failed("The capture picture could not be made".to_owned());
	}
	let bytes = encoded.into_inner();
	Loaded::File {
		thumbnail: thumbnail(&bytes, "image/png"),
		file: UploadFile::new("screenshot.png", bytes, "image/png"),
	}
}

fn capture_uploads() -> Vec<Loaded> {
	vec![
		capture_picture(),
		Loaded::File {
			thumbnail: None,
			file: UploadFile::new("clip.mp4", vec![0u8; 2048], "video/mp4"),
		},
		Loaded::File {
			thumbnail: None,
			file: UploadFile::new("notes.txt", b"midnight 2.1.2 notes".to_vec(), "text/plain"),
		},
	]
}

pub(super) struct Names {
	placeholder: &'static str,
	replace: &'static str,
	focus: &'static str,
	reply: &'static str,
	editing: &'static str,
	files: &'static str,
	suggestions: &'static str,
	heading: &'static str,
	selected: &'static str,
	typing: &'static str,
	notice: &'static str,
	drop_hover: &'static str,
	limit: &'static str,
	open: &'static str,
	changed: &'static str,
	key: &'static str,
	choose: &'static str,
	add_files: &'static str,
	remove_file: &'static str,
	start_reply: &'static str,
	start_edit: &'static str,
	retry: &'static str,
	discard: &'static str,
	confirm: &'static str,
	cancel_reply: &'static str,
	toggle_ping: &'static str,
	cancel_edit: &'static str,
	dismiss: &'static str,
	thumbnails: &'static str,
	plus: &'static str,
	spoiler: &'static str,
	edit_file: &'static str,
	pick: &'static str,
}

pub(super) const MAIN: Names = Names {
	placeholder: "composer.placeholder",
	replace: "composer.replace",
	focus: "composer.focus",
	reply: "composer.reply",
	editing: "composer.editing",
	files: "composer.files",
	suggestions: "composer.suggestions",
	heading: "composer.heading",
	selected: "composer.selected",
	typing: "composer.typing",
	notice: "composer.notice",
	drop_hover: "composer.drop_hover",
	limit: "composer.limit",
	open: "composer.open",
	changed: "composer.changed",
	key: "composer.key",
	choose: "composer.choose",
	add_files: "composer.add_files",
	remove_file: "composer.remove_file",
	start_reply: "composer.start_reply",
	start_edit: "composer.start_edit",
	retry: "composer.retry",
	discard: "composer.discard",
	confirm: "composer.confirm",
	cancel_reply: "composer.cancel_reply",
	toggle_ping: "composer.toggle_ping",
	cancel_edit: "composer.cancel_edit",
	dismiss: "composer.dismiss",
	thumbnails: "composer",
	plus: "composer.plus",
	spoiler: "composer.spoiler",
	edit_file: "composer.edit_file",
	pick: "composer.pick",
};

pub(super) const THREAD: Names = Names {
	placeholder: "thread_composer.placeholder",
	replace: "thread_composer.replace",
	focus: "thread_composer.focus",
	reply: "thread_composer.reply",
	editing: "thread_composer.editing",
	files: "thread_composer.files",
	suggestions: "thread_composer.suggestions",
	heading: "thread_composer.heading",
	selected: "thread_composer.selected",
	typing: "thread_composer.typing",
	notice: "thread_composer.notice",
	drop_hover: "thread_composer.drop_hover",
	limit: "thread_composer.limit",
	open: "thread_composer.open",
	changed: "thread_composer.changed",
	key: "thread_composer.key",
	choose: "thread_composer.choose",
	add_files: "thread_composer.add_files",
	remove_file: "thread_composer.remove_file",
	start_reply: "thread_composer.start_reply",
	start_edit: "thread_composer.start_edit",
	retry: "thread_composer.retry",
	discard: "thread_composer.discard",
	confirm: "thread_composer.confirm",
	cancel_reply: "thread_composer.cancel_reply",
	toggle_ping: "thread_composer.toggle_ping",
	cancel_edit: "thread_composer.cancel_edit",
	dismiss: "thread_composer.dismiss",
	thumbnails: "thread_composer",
	plus: "thread_composer.plus",
	spoiler: "thread_composer.spoiler",
	edit_file: "thread_composer.edit_file",
	pick: "thread_composer.pick",
};

enum Loaded {
	File {
		file: UploadFile,
		thumbnail: Option<ImageData>,
	},
	Direct {
		files: Vec<UploadFile>,
		refused: Option<String>,
	},
	Failed(String),
}

pub(super) struct ComposerView {
	names: &'static Names,
	composer: RefCell<Composer>,
	runtime: Handle,
	loads: Inbox<Loaded>,
	serial: Cell<u64>,
	focus_serial: Cell<u64>,
	replaced: RefCell<Option<String>>,
	text: RefCell<String>,
	caret: Cell<usize>,
	draft_due: Cell<Option<Instant>>,
	images: RefCell<Vec<String>>,
	thumbnails: RefCell<Vec<(u64, String)>>,
	lock_until: Cell<Option<chrono::DateTime<Utc>>>,
	placeholder: Shared,
	replace: Shared,
	focus: Shared,
	reply: Shared,
	editing: Shared,
	files: Shared,
	suggestions: Shared,
	heading: Shared,
	selected: Shared,
	typing: Shared,
	notice: Shared,
	drop_hover: Shared,
	limit: Shared,
	open: Shared,
	pick: Shared,
	pick_serial: Cell<u64>,
}

pub(super) fn content_type(name: &str) -> &'static str {
	let extension = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
	match extension.as_str() {
		"png" => "image/png",
		"jpg" | "jpeg" => "image/jpeg",
		"gif" => "image/gif",
		"webp" => "image/webp",
		"bmp" => "image/bmp",
		"mp4" => "video/mp4",
		"webm" => "video/webm",
		"mov" => "video/quicktime",
		"mp3" => "audio/mpeg",
		"ogg" => "audio/ogg",
		"wav" => "audio/wav",
		"txt" | "log" | "md" => "text/plain",
		"json" => "application/json",
		"pdf" => "application/pdf",
		"zip" => "application/zip",
		_ => "application/octet-stream",
	}
}

fn thumbnail(bytes: &[u8], kind: &str) -> Option<ImageData> {
	if !kind.starts_with("image/") {
		return None;
	}
	let image = image::load_from_memory(bytes).ok()?;
	let small = image.thumbnail(THUMBNAIL_PIXELS, THUMBNAIL_PIXELS).to_rgba8();
	ImageData::new(small.width(), small.height(), small.into_raw()).ok()
}

fn read_upload(path: &Path, limit: u64) -> Result<UploadFile, String> {
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| "file".to_owned());
	match std::fs::metadata(path) {
		Ok(metadata) if metadata.is_dir() => return Err(format!("{name} is a folder")),
		Ok(metadata) => {
			if let Some(refusal) = bridge::too_big(&name, metadata.len(), limit) {
				return Err(refusal);
			}
		}
		Err(_) => return Err(format!("{name} could not be read")),
	}
	let bytes = std::fs::read(path).map_err(|_| format!("{name} could not be read"))?;
	let kind = content_type(&name);
	Ok(UploadFile::new(name, bytes, kind))
}

fn read_file(path: PathBuf, limit: u64) -> Loaded {
	match read_upload(&path, limit) {
		Ok(file) => Loaded::File {
			thumbnail: thumbnail(&file.bytes, &file.content_type),
			file,
		},
		Err(reason) => Loaded::Failed(reason),
	}
}

fn read_direct(paths: &[PathBuf], limit: u64) -> Loaded {
	let mut files = Vec::with_capacity(paths.len());
	let mut refused = None;
	for path in paths {
		match read_upload(path, limit) {
			Ok(file) => files.push(file),
			Err(reason) => refused = Some(reason),
		}
	}
	Loaded::Direct { files, refused }
}

fn file_kind(file: &UploadFile) -> &'static str {
	if file.content_type.starts_with("image/") {
		"image"
	} else if file.content_type.starts_with("video/") {
		"video"
	} else {
		"file"
	}
}

fn background_color(mark: Mark) -> Option<&'static str> {
	match mark {
		Mark::Mention => Some("$kyoko.mention_background"),
		Mark::Code => Some("$kyoko.bg_4"),
		Mark::Spoiler => Some("$kyoko.bg_2"),
		Mark::Syntax | Mark::Label => None,
	}
}

fn backgrounds(typed: &str) -> Value {
	let spans: Vec<Value> = bridge::text_marks(typed)
		.into_iter()
		.filter(|marked| marked.range.start < marked.range.end)
		.filter_map(|marked| {
			let color = background_color(marked.mark)?;
			Some(Value::List(Rc::new([
				((marked.range.start + 1) as f64).into(),
				(marked.range.end as f64).into(),
				color.into(),
			])))
		})
		.collect();
	Value::List(spans.into())
}

fn install_marks(host: &ScriptHost) {
	host.function("composer.backgrounds", |argument| {
		backgrounds(text(&argument).unwrap_or_default())
	});
}

fn pasted_image(image: platform::clipboard::RgbaImage) -> Loaded {
	let (Ok(width), Ok(height)) = (u32::try_from(image.width), u32::try_from(image.height)) else {
		return Loaded::Failed("The pasted image is too large".to_owned());
	};
	let Some(buffer) = image::RgbaImage::from_raw(width, height, image.bytes) else {
		return Loaded::Failed("The pasted image could not be read".to_owned());
	};
	let mut encoded = Cursor::new(Vec::new());
	if buffer.write_to(&mut encoded, image::ImageFormat::Png).is_err() {
		return Loaded::Failed("The pasted image could not be read".to_owned());
	}
	let bytes = encoded.into_inner();
	Loaded::File {
		thumbnail: thumbnail(&bytes, "image/png"),
		file: UploadFile::new(
			format!("image-{}.png", Utc::now().format("%Y%m%d-%H%M%S")),
			bytes,
			"image/png",
		),
	}
}

fn suggestion_kind(visual: &Visual) -> &'static str {
	match visual {
		Visual::Avatar(_) => "avatar",
		Visual::Glyph(_) => "glyph",
		Visual::Emoji { .. } => "emoji",
		Visual::Icon(_) => "icon",
	}
}

fn caret_of(value: &Value) -> usize {
	match field(value, "caret") {
		Some(Value::Num(caret)) if caret.is_finite() && *caret >= 0. => *caret as usize,
		_ => usize::MAX,
	}
}

fn flag(value: &Value, key: &str) -> bool {
	matches!(field(value, key), Some(Value::Bool(true)))
}

impl ComposerView {
	pub(super) fn install(host: &ScriptHost, runtime: Handle, names: &'static Names) -> ComposerView {
		ComposerView {
			names,
			composer: RefCell::new(Composer::default()),
			runtime,
			loads: Inbox::new(),
			serial: Cell::new(0),
			focus_serial: Cell::new(0),
			replaced: RefCell::new(None),
			text: RefCell::new(String::new()),
			caret: Cell::new(0),
			draft_due: Cell::new(None),
			images: RefCell::new(Vec::new()),
			thumbnails: RefCell::new(Vec::new()),
			lock_until: Cell::new(None),
			placeholder: host.shared(names.placeholder, ""),
			replace: host.shared(
				names.replace,
				record([("serial", 0.0.into()), ("text", "".into()), ("caret", 0.0.into())]),
			),
			focus: host.shared(names.focus, 0.0),
			reply: host.shared(names.reply, record([("active", false.into())])),
			editing: host.shared(names.editing, false),
			files: host.shared(names.files, Value::List(Rc::new([]))),
			suggestions: host.shared(names.suggestions, Value::List(Rc::new([]))),
			heading: host.shared(
				names.heading,
				record([("title", "".into()), ("query", "".into())]),
			),
			selected: host.shared(names.selected, 1.0),
			typing: host.shared(names.typing, ""),
			notice: host.shared(names.notice, record([("kind", "".into())])),
			drop_hover: host.shared(names.drop_hover, false),
			limit: host.shared(names.limit, 2000.0),
			open: host.shared(names.open, false),
			pick: host.shared(names.pick, 0.0),
			pick_serial: Cell::new(0),
		}
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.images.borrow().clone()
	}

	pub(super) fn runtime(&self) -> &Handle {
		&self.runtime
	}
}

impl Link {
	pub(super) fn composer_acted_recently(&self) -> bool {
		let now = Utc::now();
		[Pane::Main, Pane::Thread]
			.into_iter()
			.any(|pane| self.pane_composer(pane).composer.borrow().acted_recently(now))
	}

	pub(super) fn pane_composer(&self, pane: Pane) -> &ComposerView {
		match pane {
			Pane::Main => &self.composer,
			Pane::Thread => self.thread_composer(),
		}
	}

	pub(super) fn pane_open(&self, pane: Pane) -> Option<OpenChannel> {
		match pane {
			Pane::Main => self.timeline.borrow().open(),
			Pane::Thread => self.thread_open(),
		}
	}

	pub(super) fn install_composer(host: &ScriptHost, link: &Rc<Link>) {
		for pane in [Pane::Main, Pane::Thread] {
			Link::install_pane_composer(host, link, pane);
		}
		install_marks(host);
		let actions = Rc::downgrade(link);
		link.composer_layer.on_request(move |request| {
			if let Some(link) = actions.upgrade() {
				link.composer_request(request);
			}
		});
	}

	fn composer_request(&self, request: Request) {
		match request {
			Request::Upload(pane) => {
				let view = self.pane_composer(pane);
				let serial = view.pick_serial.get() + 1;
				view.pick_serial.set(serial);
				view.pick.set((serial as f64).into());
			}
			Request::CreateThread => {
				if let Some(open) = self.pane_open(Pane::Main) {
					self.open_create_thread(open.channel_id, None);
				}
			}
			Request::SaveFile(pane, id, edit) => {
				if self.pane_composer(pane).composer.borrow_mut().edit_file(id, edit) {
					self.publish_composer_all(pane);
				}
			}
		}
	}

	fn composer_plus(&self, pane: Pane, at: (f32, f32)) {
		let thread = pane == Pane::Main
			&& self.state().is_some_and(|shared| {
				self.pane_open(pane)
					.is_some_and(|open| bridge::can_create_threads(&shared.read(), open.channel_id))
			});
		self.composer_layer.open_menu(PlusMenu { pane, at, thread });
	}

	fn composer_edit_file(&self, pane: Pane, id: u64) {
		let edit = self
			.pane_composer(pane)
			.composer
			.borrow()
			.file(id)
			.map(|attachment| FileEdit {
				name: attachment.file.name.clone(),
				description: attachment.file.description.clone().unwrap_or_default(),
				spoiler: attachment.file.spoiler,
			});
		if let Some(edit) = edit {
			self.composer_layer.edit_file(pane, id, edit);
		}
	}

	fn install_pane_composer(host: &ScriptHost, link: &Rc<Link>, pane: Pane) {
		let names = link.pane_composer(pane).names;
		let actions = Rc::downgrade(link);
		host.function(names.changed, move |argument| {
			if let Some(link) = actions.upgrade() {
				let typed = field(&argument, "text")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.composer_changed(pane, typed, caret_of(&argument));
			}
			Value::Nil
		});
		let actions = Rc::downgrade(link);
		host.function(names.key, move |argument| {
			let consumed = actions
				.upgrade()
				.is_some_and(|link| link.composer_key(pane, &argument));
			Value::Bool(consumed)
		});
		let actions = Rc::downgrade(link);
		host.action(names.choose, move |argument| {
			if let Some(link) = actions.upgrade() {
				let index = match field(&argument, "index") {
					Some(Value::Num(index)) if *index >= 1. => Some(*index as usize - 1),
					_ => None,
				};
				let typed = field(&argument, "text")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.composer_choose(pane, &typed, index);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.add_files, move |argument| {
			if let Some(link) = actions.upgrade() {
				let paths = texts(&argument).into_iter().map(PathBuf::from).collect();
				link.composer_add_paths(pane, paths);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.remove_file, move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(id) = text(&argument).and_then(|id| id.parse().ok())
			{
				link.composer_remove_file(pane, id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.plus, move |argument| {
			if let (Some(link), Some(x), Some(y)) = (
				actions.upgrade(),
				super::number(&argument, "x"),
				super::number(&argument, "y"),
			) {
				link.composer_plus(pane, (x, y));
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.spoiler, move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(id) = text(&argument).and_then(|id| id.parse().ok())
				&& link.pane_composer(pane).composer.borrow_mut().toggle_spoiler(id)
			{
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.edit_file, move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(id) = text(&argument).and_then(|id| id.parse().ok())
			{
				link.composer_edit_file(pane, id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.start_reply, move |argument| {
			if let (Some(link), Some(message_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.pane_reply(pane, message_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.start_edit, move |argument| {
			if let (Some(link), Some(message_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.pane_edit(pane, message_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.retry, move |argument| {
			if let Some(link) = actions.upgrade() {
				let nonce = Nonce(text(&argument).unwrap_or_default().to_owned());
				let command = link.pane_composer(pane).composer.borrow().retry(nonce);
				link.composer_send(command.into_iter().collect());
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.discard, move |argument| {
			if let Some(link) = actions.upgrade() {
				let nonce = Nonce(text(&argument).unwrap_or_default().to_owned());
				let command = link.pane_composer(pane).composer.borrow().discard(nonce);
				link.composer_send(command.into_iter().collect());
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.confirm, move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(shared) = link.state()
			{
				let outcome = link
					.pane_composer(pane)
					.composer
					.borrow_mut()
					.confirm_lock(&shared.read(), Utc::now());
				link.composer_outcome(pane, outcome);
			}
		});
		let simple: [(&str, Handler); 4] = [
			(names.cancel_reply, |link, pane| {
				link.pane_composer(pane).composer.borrow_mut().cancel_reply();
			}),
			(names.toggle_ping, |link, pane| {
				link.pane_composer(pane).composer.borrow_mut().toggle_ping()
			}),
			(names.cancel_edit, |link, pane| {
				link.composer_cancel_edit(pane);
			}),
			(names.dismiss, |link, pane| {
				link.pane_composer(pane).composer.borrow_mut().dismiss();
			}),
		];
		for (name, handler) in simple {
			let actions = Rc::downgrade(link);
			host.action(name, move |_| {
				if let Some(link) = actions.upgrade() {
					handler(&link, pane);
					link.publish_composer_all(pane);
				}
			});
		}
	}

	pub(super) async fn follow_composer_files(self: Rc<Self>, pane: Pane) {
		let loads = self.pane_composer(pane).loads.clone();
		while let Some(batch) = loads.next().await {
			for loaded in batch {
				self.composer_loaded(pane, loaded);
			}
			self.publish_composer_all(pane);
		}
	}

	pub(super) async fn tick_composer(self: Rc<Self>) {
		loop {
			zgx::timer::sleep(TICK).await;
			let now = Instant::now();
			for pane in [Pane::Main, Pane::Thread] {
				let composer = self.pane_composer(pane);
				if composer.draft_due.get().is_some_and(|due| due <= now) {
					self.flush_draft(pane);
				}
				if composer.lock_until.get().is_some_and(|until| until <= Utc::now()) {
					composer.lock_until.set(None);
					self.publish_composer_notice(pane);
				}
			}
		}
	}

	pub(super) fn files_dropped(&self, paths: &[PathBuf]) {
		let direct = platform::keyboard::shift_held();
		tracing::debug!(files = paths.len(), direct, "files dropped on the window");
		self.composer.drop_hover.set(false.into());
		self.composer_layer.show_drop(None);
		if direct {
			self.composer_upload_now(Pane::Main, paths.to_vec());
		} else {
			self.composer_add_paths(Pane::Main, paths.to_vec());
		}
	}

	pub(super) fn files_hovering(&self, hovering: bool) {
		tracing::debug!(hovering, "files over the window");
		self.composer.drop_hover.set(hovering.into());
		let target = hovering.then(|| self.drop_target()).flatten();
		self.composer_layer.show_drop(target);
	}

	fn drop_target(&self) -> Option<String> {
		let shared = self.state()?;
		let state = shared.read();
		self.composer.composer.borrow().channel()?;
		let placeholder = self.composer_placeholder(Pane::Main, &state);
		Some(
			placeholder
				.strip_prefix("Message ")
				.unwrap_or(&placeholder)
				.to_owned(),
		)
	}

	fn composer_upload_now(&self, pane: Pane, paths: Vec<PathBuf>) {
		let Some(shared) = self.state() else {
			return;
		};
		let view = self.pane_composer(pane);
		let Some(channel_id) = view.composer.borrow().channel() else {
			return;
		};
		let limit = shared.read().upload_limit(channel_id);
		let loads = view.loads.clone();
		view.runtime
			.spawn_blocking(move || loads.push(read_direct(&paths, limit)));
	}

	fn flush_draft(&self, pane: Pane) {
		let view = self.pane_composer(pane);
		view.draft_due.set(None);
		let composer = view.composer.borrow();
		if composer.editing().is_some() {
			return;
		}
		if let Some(channel_id) = composer.channel() {
			self.send(session::Command::SetDraft {
				channel_id,
				content: view.text.borrow().clone(),
			});
		}
	}

	pub(super) fn composer_apply(&self, state: &State, changes: &Changes) {
		for pane in [Pane::Main, Pane::Thread] {
			self.pane_apply(pane, state, changes);
		}
	}

	fn pane_apply(&self, pane: Pane, state: &State, changes: &Changes) {
		let view = self.pane_composer(pane);
		let open = self.pane_open(pane);
		let target = open.map(|open| open.channel_id);
		let switched = view.composer.borrow().channel() != target;
		if switched {
			if pane == Pane::Main {
				self.close_expressions();
			}
			if view.draft_due.get().is_some() {
				self.flush_draft(pane);
			}
			let outcome = view
				.composer
				.borrow_mut()
				.open(state, open.and_then(|open| open.guild_id), target);
			self.replace_text(pane, outcome.replace);
			self.publish_composer(pane, state);
			return;
		}
		let Some(channel_id) = target else {
			return;
		};
		if changes.drafts.contains(channel_id) {
			let draft = view.composer.borrow_mut().draft_arrived(state);
			if let Some(draft) = draft
				&& *view.text.borrow() != draft.0
			{
				self.replace_text(pane, Some(draft));
			}
		}
		if changes.typing.contains(channel_id) || changes.members.contains(channel_id) {
			self.publish_composer_typing(pane, state);
		}
		if changes.emoji_ranking {
			let request = view.composer.borrow_mut().rank_request(state);
			self.composer_send(request.into_iter().collect());
		}
		if changes.commands
			|| changes.emoji_ranking
			|| !changes.members.is_empty()
			|| changes.messages.contains(channel_id)
			|| !changes.channel_lists.is_empty()
		{
			let refreshed = view.composer.borrow_mut().refresh_suggestions(state);
			if refreshed {
				self.publish_composer_suggestions(pane, state);
			}
		}
		if changes.messages.contains(channel_id)
			|| changes.locks
			|| changes.rejected
			|| changes.private_channels
			|| changes.relationships
			|| !changes.channel_lists.is_empty()
			|| !changes.members.is_empty()
		{
			self.publish_composer_notice_with(pane, state);
		}
		if changes.current_user {
			view.limit.set((state.message_limit() as f64).into());
		}
		if !changes.channel_lists.is_empty() || changes.private_channels || changes.session {
			view.placeholder
				.set(self.composer_placeholder(pane, state).as_str().into());
		}
	}

	fn replace_text(&self, pane: Pane, replace: Option<(String, usize)>) {
		let Some((replacement, caret)) = replace else {
			return;
		};
		let view = self.pane_composer(pane);
		let serial = view.serial.get() + 1;
		view.serial.set(serial);
		*view.text.borrow_mut() = replacement.clone();
		view.caret.set(caret);
		*view.replaced.borrow_mut() = Some(replacement.clone());
		view.replace.set(record([
			("serial", (serial as f64).into()),
			("text", replacement.as_str().into()),
			("caret", (caret as f64).into()),
		]));
	}

	fn request_focus(&self, pane: Pane) {
		let view = self.pane_composer(pane);
		let serial = view.focus_serial.get() + 1;
		view.focus_serial.set(serial);
		view.focus.set((serial as f64).into());
	}

	pub(super) fn composer_insert(&self, cell: &bridge::EmojiCell) {
		let typed = self.composer.text.borrow().clone();
		let caret = self.composer.caret.get();
		let inserted = self
			.composer
			.composer
			.borrow_mut()
			.pick_emoji(&typed, caret, cell);
		if inserted.is_some() {
			self.composer.draft_due.set(Some(Instant::now() + DRAFT_DELAY));
			self.replace_text(Pane::Main, inserted);
			self.request_focus(Pane::Main);
		}
	}

	pub(super) fn composer_expression(&self, pick: impl FnOnce(&mut Composer, &State) -> Outcome) {
		let Some(shared) = self.state() else {
			return;
		};
		let outcome = pick(&mut self.composer.composer.borrow_mut(), &shared.read());
		self.composer_send(outcome.commands);
		self.publish_composer_all(Pane::Main);
	}

	pub(super) fn composer_channel(&self) -> Option<(Option<Snowflake>, Snowflake)> {
		let composer = self.composer.composer.borrow();
		Some((composer.guild(), composer.channel()?))
	}

	fn composer_send(&self, commands: Vec<session::Command>) {
		for command in commands {
			self.send(command);
		}
	}

	fn composer_outcome(&self, pane: Pane, outcome: Outcome) {
		let cleared = outcome.replace.as_ref().is_some_and(|(text, _)| text.is_empty());
		if cleared {
			self.pane_composer(pane).draft_due.set(None);
		}
		self.replace_text(pane, outcome.replace);
		self.composer_send(outcome.commands);
		self.publish_composer_all(pane);
	}

	fn composer_changed(&self, pane: Pane, typed: String, caret: usize) {
		let Some(shared) = self.state() else {
			return;
		};
		let view = self.pane_composer(pane);
		let echoed = view.replaced.borrow().as_deref() == Some(typed.as_str());
		let caret = if echoed { view.caret.get() } else { caret };
		*view.text.borrow_mut() = typed.clone();
		view.caret.set(caret.min(typed.len()));
		let commands = {
			let state = shared.read();
			let mut composer = view.composer.borrow_mut();
			if echoed {
				view.replaced.borrow_mut().take();
				composer.restored(&state, &typed, caret);
				Vec::new()
			} else {
				view.draft_due.set(Some(Instant::now() + DRAFT_DELAY));
				composer.changed(&state, &typed, caret)
			}
		};
		self.composer_send(commands);
		self.publish_composer_suggestions(pane, &shared.read());
	}

	fn composer_key(&self, pane: Pane, event: &Value) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let view = self.pane_composer(pane);
		let key = field(event, "key").and_then(text).unwrap_or_default();
		let typed = field(event, "text").and_then(text).unwrap_or_default().to_owned();
		let shift = flag(event, "shift");
		let suggesting = !view.composer.borrow().suggestions().is_empty();
		match key {
			"ArrowDown" | "ArrowUp" if suggesting => {
				let delta = if key == "ArrowDown" { 1 } else { -1 };
				view.composer.borrow_mut().move_selection(delta);
				self.publish_composer_selected(pane);
				true
			}
			"Tab" | "Enter" if suggesting && !shift => {
				self.composer_choose(pane, &typed, None);
				true
			}
			"Escape" => {
				let edited = self.composer_cancel_edit(pane);
				let dismissed = edited || view.composer.borrow_mut().dismiss();
				self.publish_composer_all(pane);
				dismissed
			}
			"Enter" if !shift => {
				let outcome = view
					.composer
					.borrow_mut()
					.submit(&shared.read(), &typed, Utc::now());
				self.composer_outcome(pane, outcome);
				true
			}
			"ArrowUp" if typed.is_empty() && view.composer.borrow().files().is_empty() => {
				let edit = view.composer.borrow_mut().edit_last(&shared.read());
				let editing = edit.is_some();
				self.replace_text(pane, edit);
				self.publish_composer_all(pane);
				editing
			}
			"v" | "V" if flag(event, "cmd") => self.composer_paste(pane),
			_ => false,
		}
	}

	fn composer_choose(&self, pane: Pane, typed: &str, index: Option<usize>) {
		let chosen = self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.choose(typed, index);
		self.replace_text(pane, chosen);
		if let Some(shared) = self.state() {
			self.publish_composer_suggestions(pane, &shared.read());
		}
	}

	fn composer_cancel_edit(&self, pane: Pane) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let restored = self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.cancel_edit(&shared.read());
		let cancelled = restored.is_some();
		self.replace_text(pane, restored);
		cancelled
	}

	pub(super) fn composer_reply(&self, message_id: Snowflake) {
		self.pane_reply(Pane::Main, message_id);
	}

	pub(super) fn composer_edit(&self, message_id: Snowflake) {
		self.pane_edit(Pane::Main, message_id);
	}

	pub(super) fn pane_reply(&self, pane: Pane, message_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		if self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.start_reply(&shared.read(), message_id)
		{
			self.request_focus(pane);
			self.publish_composer_all(pane);
		}
	}

	pub(super) fn pane_edit(&self, pane: Pane, message_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let edit = self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.start_edit(&shared.read(), message_id);
		if edit.is_some() {
			self.replace_text(pane, edit);
			self.request_focus(pane);
			self.publish_composer_all(pane);
		}
	}

	fn composer_paste(&self, pane: Pane) -> bool {
		let files = platform::clipboard::read_files().unwrap_or_default();
		if !files.is_empty() {
			self.composer_add_paths(pane, files);
			return true;
		}
		let image = match platform::clipboard::read_image() {
			Ok(Some(image)) => image,
			Ok(None) => return false,
			Err(error) => {
				tracing::debug!(%error, "the clipboard image could not be read");
				return false;
			}
		};
		let view = self.pane_composer(pane);
		let loads = view.loads.clone();
		view.runtime
			.spawn_blocking(move || loads.push(pasted_image(image)));
		true
	}

	fn composer_add_paths(&self, pane: Pane, paths: Vec<PathBuf>) {
		let Some(shared) = self.state() else {
			return;
		};
		let view = self.pane_composer(pane);
		let Some(channel_id) = view.composer.borrow().channel() else {
			return;
		};
		let limit = shared.read().upload_limit(channel_id);
		for path in paths {
			let loads = view.loads.clone();
			view.runtime
				.spawn_blocking(move || loads.push(read_file(path, limit)));
		}
	}

	fn composer_loaded(&self, pane: Pane, loaded: Loaded) {
		let Some(shared) = self.state() else {
			return;
		};
		let view = self.pane_composer(pane);
		let mut composer = view.composer.borrow_mut();
		match loaded {
			Loaded::File { file, thumbnail } => match composer.attach(&shared.read(), file) {
				Ok(id) => {
					if let Some(thumbnail) = thumbnail {
						let name = format!("{}/{id}", view.names.thumbnails);
						self.host.image(&name, Some(Arc::new(thumbnail)));
						view.thumbnails.borrow_mut().push((id, name));
					}
				}
				Err(reason) => composer.refuse(reason),
			},
			Loaded::Direct { files, refused } => {
				let outcome = composer.upload_now(&shared.read(), files, Utc::now());
				if let Some(reason) = refused {
					composer.refuse(reason);
				}
				drop(composer);
				self.composer_send(outcome.commands);
				return;
			}
			Loaded::Failed(reason) => composer.refuse(reason),
		}
		drop(composer);
		self.request_focus(pane);
	}

	fn composer_remove_file(&self, pane: Pane, id: u64) {
		let view = self.pane_composer(pane);
		if view.composer.borrow_mut().remove_file(id) {
			view.thumbnails.borrow_mut().retain(|(file, name)| {
				let keep = *file != id;
				if !keep {
					self.host.image(name, None);
				}
				keep
			});
			self.publish_composer_all(pane);
		}
	}

	fn forget_thumbnails(&self, pane: Pane) {
		let view = self.pane_composer(pane);
		let composer = view.composer.borrow();
		view.thumbnails.borrow_mut().retain(|(id, name)| {
			let keep = composer.files().iter().any(|attachment| attachment.id == *id);
			if !keep {
				self.host.image(name, None);
			}
			keep
		});
	}

	fn publish_composer_all(&self, pane: Pane) {
		if let Some(shared) = self.state() {
			self.publish_composer(pane, &shared.read());
		}
	}

	fn publish_composer(&self, pane: Pane, state: &State) {
		self.forget_thumbnails(pane);
		let view = self.pane_composer(pane);
		let composer = view.composer.borrow();
		let channel_id = composer.channel();
		view.open.set(channel_id.is_some().into());
		view.placeholder
			.set(self.composer_placeholder(pane, state).as_str().into());
		view.limit.set((state.message_limit() as f64).into());
		view.editing.set(composer.editing().is_some().into());
		view.reply.set(match composer.reply() {
			Some(reply) => record([
				("active", true.into()),
				("name", reply.name.as_str().into()),
				("ping", reply.ping.into()),
				("own", reply.own.into()),
			]),
			None => record([("active", false.into())]),
		});
		let thumbnails = view.thumbnails.borrow();
		let files: Vec<Value> = composer
			.files()
			.iter()
			.map(|attachment| {
				let image = thumbnails
					.iter()
					.find(|(id, _)| *id == attachment.id)
					.map(|(_, name)| format!("host:{name}"))
					.unwrap_or_default();
				record([
					("id", attachment.id.to_string().as_str().into()),
					("name", attachment.file.name.as_str().into()),
					("kind", file_kind(&attachment.file).into()),
					("spoiler", attachment.file.spoiler.into()),
					(
						"size",
						bridge::file_size(attachment.file.bytes.len() as u64)
							.as_str()
							.into(),
					),
					("image", image.as_str().into()),
				])
			})
			.collect();
		view.files.set(Value::List(files.into()));
		drop(thumbnails);
		drop(composer);
		self.publish_composer_suggestions(pane, state);
		self.publish_composer_typing(pane, state);
		self.publish_composer_notice_with(pane, state);
	}

	fn composer_placeholder(&self, pane: Pane, state: &State) -> String {
		if !state.session().connected && self.pane_open(pane).is_some() {
			return WAITING.to_owned();
		}
		match self.pane_open(pane) {
			Some(open) if open.guild_id.is_none() => format!("Message @{}", self.channel_title(state, open)),
			Some(open) => bridge::placeholder(state, open.channel_id),
			None => String::new(),
		}
	}

	fn publish_composer_typing(&self, pane: Pane, state: &State) {
		let view = self.pane_composer(pane);
		let line = view.composer.borrow().typing(state, Instant::now());
		view.typing.set(line.as_str().into());
	}

	fn publish_composer_selected(&self, pane: Pane) {
		let view = self.pane_composer(pane);
		let selected = view.composer.borrow().selected() + 1;
		view.selected.set((selected as f64).into());
	}

	fn suggestion_image(&self, state: &State, suggestion: &Suggestion) -> String {
		let scale = self.scale_factor();
		let request = match suggestion.visual {
			Visual::Avatar(user_id) => state.user(user_id).and_then(|user| {
				bridge::avatar_request(&self.images.cdn, user, SUGGESTION_POINTS, scale, false)
			}),
			Visual::Emoji { id, animated } => Some(bridge::emoji_request(
				&self.images.cdn,
				id,
				animated,
				SUGGESTION_POINTS,
				scale,
			)),
			_ => None,
		};
		request
			.map(|request| self.images.gallery.request(request).to_string())
			.unwrap_or_default()
	}

	fn publish_composer_suggestions(&self, pane: Pane, state: &State) {
		let view = self.pane_composer(pane);
		let composer = view.composer.borrow();
		let mut images = Vec::new();
		let items: Vec<Value> = composer
			.suggestions()
			.iter()
			.enumerate()
			.map(|(index, suggestion)| {
				let image = self.suggestion_image(state, suggestion);
				if !image.is_empty() {
					images.push(image.clone());
				}
				let (glyph, icon) = match &suggestion.visual {
					Visual::Glyph(glyph) => (glyph.as_str(), ""),
					Visual::Icon(icon) => ("", *icon),
					_ => ("", ""),
				};
				let key = format!("{index}:{}:{}", suggestion.label, suggestion.insert);
				record([
					("key", key.as_str().into()),
					("label", suggestion.label.as_str().into()),
					("detail", suggestion.detail.as_deref().unwrap_or_default().into()),
					("kind", suggestion_kind(&suggestion.visual).into()),
					(
						"image",
						if image.is_empty() {
							String::new()
						} else {
							format!("host:{image}")
						}
						.as_str()
						.into(),
					),
					("glyph", glyph.into()),
					("svg", crate::emoji::luau_code(glyph).as_str().into()),
					("icon", icon.into()),
					("lock", suggestion.lock.unwrap_or_default().into()),
				])
			})
			.collect();
		let selected = composer.selected() + 1;
		let (title, query) = composer.heading().unwrap_or_default();
		drop(composer);
		view.heading.set(record([
			("title", title.into()),
			("query", query.as_str().into()),
		]));
		view.suggestions.set(Value::List(items.into()));
		view.selected.set((selected as f64).into());
		let changed = *view.images.borrow() != images;
		if changed {
			*view.images.borrow_mut() = images;
			self.refresh_visible();
		}
	}

	fn publish_composer_notice(&self, pane: Pane) {
		if let Some(shared) = self.state() {
			self.publish_composer_notice_with(pane, &shared.read());
		}
	}

	fn publish_composer_notice_with(&self, pane: Pane, state: &State) {
		let now = Utc::now();
		let view = self.pane_composer(pane);
		let mut composer = view.composer.borrow_mut();
		view.lock_until
			.set(composer.lock(state, now).and_then(|lock| lock.until));
		let notice = match composer.notice(state, now) {
			Some(Notice::Lock { text, confirmable }) => record([
				("kind", "lock".into()),
				("text", text.as_str().into()),
				("confirmable", confirmable.into()),
				("nonce", "".into()),
			]),
			Some(Notice::Failed { nonce, text }) => record([
				("kind", "failed".into()),
				("text", text.as_str().into()),
				("confirmable", false.into()),
				("nonce", nonce.0.as_str().into()),
			]),
			Some(Notice::Refused(text)) => record([
				("kind", "refused".into()),
				("text", text.as_str().into()),
				("confirmable", false.into()),
				("nonce", "".into()),
			]),
			None => record([("kind", "".into())]),
		};
		view.notice.set(notice);
	}

	pub(super) fn drive_composer(&self, screen: &capture::Screen) {
		match screen {
			capture::Screen::Composer => {
				self.replace_text(Pane::Main, Some((CAPTURE_DRAFT.to_owned(), CAPTURE_DRAFT.len())));
				self.request_focus(Pane::Main);
			}
			capture::Screen::ComposerReply => self.reply_to_newest_other(),
			_ => {}
		}
	}

	fn reply_to_newest_other(&self) {
		let Some(shared) = self.state() else {
			return;
		};
		let target = {
			let state = shared.read();
			let me = state.current_user_id();
			self.composer.composer.borrow().channel().and_then(|channel_id| {
				state
					.messages(channel_id)?
					.ordered
					.iter()
					.rev()
					.find(|message| Some(message.author.id) != me)
					.map(|message| message.id)
			})
		};
		if let Some(message_id) = target {
			self.composer_reply(message_id);
		}
	}

	pub(super) async fn drive_composer_shot(&self, shot: capture::ComposerShot) {
		match shot {
			capture::ComposerShot::OverLimit => {
				self.reply_to_newest_other();
				let limit = self.state().map_or(0, |shared| shared.read().message_limit());
				let draft = over_limit(limit);
				self.replace_text(Pane::Main, Some((draft.clone(), draft.len())));
				self.request_focus(Pane::Main);
				self.until_capture(|link| *link.composer.text.borrow() == draft)
					.await;
			}
			capture::ComposerShot::Uploads => self.drive_uploads().await,
			capture::ComposerShot::Drop => self.files_hovering(true),
			capture::ComposerShot::UploadFailed => self.drive_failed_uploads().await,
			capture::ComposerShot::PlusMenu => {
				self.until_capture(|link| link.composer_slot().is_some()).await;
				if let Some(slot) = self.composer_slot() {
					self.composer_plus(Pane::Main, (slot.x, slot.y));
				}
				self.until_capture(|link| link.composer_layer.menu_shown()).await;
			}
			capture::ComposerShot::EmojiAutocomplete => {
				self.replace_text(
					Pane::Main,
					Some((CAPTURE_EMOJI_QUERY.to_owned(), CAPTURE_EMOJI_QUERY.len())),
				);
				self.request_focus(Pane::Main);
				self.until_capture(|link| {
					let composer = link.composer.composer.borrow();
					let suggestions = composer.suggestions();
					suggestions.iter().any(|suggestion| suggestion.lock.is_some())
						&& suggestions.iter().any(|suggestion| {
							matches!(suggestion.visual, Visual::Emoji { .. }) && suggestion.lock.is_none()
						})
				})
				.await;
			}
		}
	}

	async fn drive_uploads(&self) {
		for loaded in capture_uploads() {
			self.composer.loads.push(loaded);
		}
		self.until_capture(|link| link.composer.composer.borrow().files().len() == CAPTURE_UPLOADS)
			.await;
		let first = self
			.composer
			.composer
			.borrow()
			.files()
			.first()
			.map(|attachment| attachment.id);
		if let Some(id) = first {
			self.composer.composer.borrow_mut().toggle_spoiler(id);
			self.publish_composer_all(Pane::Main);
		}
		zgx::timer::sleep(capture::SETTLE).await;
		let (Some(slot), Some(mut ui)) = (
			self.composer_slot(),
			self.ui.borrow().as_ref().and_then(zgx::widgets::WeakUi::upgrade),
		) else {
			return;
		};
		let x = slot.x + TRAY_INSET + CARD + CARD_GAP + CARD / 2.;
		let y = slot.y - TRAY_INSET + 1. - CARD / 2.;
		ui.dispatch(zgx::input::InputEvent::PointerMove { x, y });
	}

	async fn drive_failed_uploads(&self) {
		let Some(channel_id) = self.composer.composer.borrow().channel() else {
			return;
		};
		let send = |content: &str, file: UploadFile| {
			self.send(session::Command::SendMessage {
				channel_id,
				content: content.to_owned(),
				reply_to: None,
				ping_reply: true,
				files: vec![file],
			});
		};
		send(
			capture::UPLOADING,
			UploadFile::new(
				mockcord::STALLED_UPLOAD,
				vec![0u8; CAPTURE_VIDEO_BYTES],
				"video/mp4",
			),
		);
		send(
			capture::UPLOAD_FAILED,
			UploadFile::new(
				mockcord::BROKEN_UPLOAD,
				vec![0u8; CAPTURE_VIDEO_BYTES],
				"video/mp4",
			),
		);
		if let Some(shared) = self.state() {
			let limit = shared.read().upload_limit(channel_id);
			if let Some(refusal) = bridge::too_big("cat.mov", limit * 2, limit) {
				self.composer.composer.borrow_mut().refuse(refusal);
			}
			self.publish_composer(Pane::Main, &shared.read());
		}
		self.until_capture(|link| {
			let timeline = link.timeline.borrow();
			let failed: Vec<bool> = timeline
				.messages()
				.filter(|entry| entry.upload.is_some())
				.map(|entry| entry.failed)
				.collect();
			failed.contains(&true) && failed.contains(&false)
		})
		.await;
	}

	pub(super) fn composer_settled(&self, screen: &capture::Screen) -> bool {
		match screen {
			capture::Screen::Composer => *self.composer.text.borrow() == CAPTURE_DRAFT,
			capture::Screen::ComposerReply => self.composer.composer.borrow().reply().is_some(),
			_ => true,
		}
	}
}
