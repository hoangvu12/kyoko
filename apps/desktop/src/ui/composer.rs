use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{
	ActiveCommand, ChoiceList, CommandApp, CommandPicker, CommandResult, CommandResults, Composer, FileEdit,
	Inbox, Mark, Notice, OpenChannel, OptionChoices, OptionFile, Outcome, Suggestion, Visual,
};
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
const APP_ICON_POINTS: f64 = 32.;
const HEADER_ICON_POINTS: f64 = 16.;
const COMMAND_ICON_POINTS: f64 = 24.;
const RESULT_POINTS: f64 = 120.;
const MIN_RATIO: f64 = 0.5;
const MAX_RATIO: f64 = 2.;
const THUMBNAIL_PIXELS: u32 = 400;
const WAITING: &str = "Waiting for connection...";
type Handler = fn(&Link, Pane);

const CAPTURE_DRAFT: &str = "Checking how the composer looks with a draft in it";
const CAPTURE_EMOJI_QUERY: &str = "wow :sm";
const CAPTURE_OPTION_TEXT: &str = "hello";
const CAPTURE_ROLL_SIDES: i64 = 20;
const CAPTURE_OPTIONAL_PICK: &str = "query";
type ReplyStep = (u64, &'static str, fn(&bridge::MessageEntry) -> bool);
const CAPTURE_FILE_NAME: &str = "cat.png";
const CAPTURE_GIF_QUERY: &str = "cat";
const CAPTURE_STICKER_QUERY: &str = "wave";
const CAPTURE_THREAD: [&str; 2] = ["picker polish", "notes from testing"];

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

fn capture_cat() -> Loaded {
	let (width, height) = CAPTURE_PICTURE;
	let picture = image::RgbaImage::from_fn(width, height, |x, y| {
		let along = ((x + y) * 255 / (width + height)) as u8;
		image::Rgba([240, 200 - along / 2, 60 - along / 5, 255])
	});
	let mut encoded = Cursor::new(Vec::new());
	if picture.write_to(&mut encoded, image::ImageFormat::Png).is_err() {
		return Loaded::Failed("The capture picture could not be made".to_owned());
	}
	let bytes = encoded.into_inner();
	Loaded::Option {
		thumbnail: thumbnail(&bytes, "image/png"),
		file: UploadFile::new(CAPTURE_FILE_NAME, bytes, "image/png"),
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
	picker: &'static str,
	command: &'static str,
	command_section: &'static str,
	option_changed: &'static str,
	option_key: &'static str,
	option_focus: &'static str,
	toggle_optional: &'static str,
	add_optional: &'static str,
	cancel_command: &'static str,
	choose_option: &'static str,
	choose_result: &'static str,
	retry_choices: &'static str,
	option_files: &'static str,
	remove_option_file: &'static str,
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
	picker: "composer.picker_state",
	command: "composer.command",
	command_section: "composer.command_section",
	option_changed: "composer.option_changed",
	option_key: "composer.option_key",
	option_focus: "composer.option_focus",
	toggle_optional: "composer.toggle_optional",
	add_optional: "composer.add_optional",
	cancel_command: "composer.cancel_command",
	choose_option: "composer.choose_option",
	choose_result: "composer.choose_result",
	retry_choices: "composer.retry_choices",
	option_files: "composer.option_files",
	remove_option_file: "composer.remove_option_file",
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
	picker: "thread_composer.picker_state",
	command: "thread_composer.command",
	command_section: "thread_composer.command_section",
	option_changed: "thread_composer.option_changed",
	option_key: "thread_composer.option_key",
	option_focus: "thread_composer.option_focus",
	toggle_optional: "thread_composer.toggle_optional",
	add_optional: "thread_composer.add_optional",
	cancel_command: "thread_composer.cancel_command",
	choose_option: "thread_composer.choose_option",
	choose_result: "thread_composer.choose_result",
	retry_choices: "thread_composer.retry_choices",
	option_files: "thread_composer.option_files",
	remove_option_file: "thread_composer.remove_option_file",
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
	Option {
		file: UploadFile,
		thumbnail: Option<ImageData>,
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
	picker: Shared,
	command: Shared,
	command_focus: Cell<u64>,
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

fn position(value: &Value) -> Option<usize> {
	match value {
		Value::Num(index) if index.is_finite() && *index >= 1. => Some(*index as usize - 1),
		_ => None,
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
			picker: host.shared(names.picker, record([("open", false.into())])),
			command: host.shared(names.command, record([("active", false.into())])),
			command_focus: Cell::new(0),
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
			Request::SendPoll(pane, poll) => {
				if let Some(open) = self.pane_open(pane) {
					self.send(session::Command::SendPoll {
						channel_id: open.channel_id,
						poll,
					});
				}
			}
			Request::PollEmoji => {
				if let Some(shared) = self.state() {
					let model = bridge::EmojiPicker::of(&shared.read(), self.open_guild());
					self.composer_layer.pick_poll_emoji(model);
				}
			}
			Request::ImagesChanged => self.refresh_visible(),
			Request::UseApps => self.use_apps(),
		}
	}

	fn composer_plus(&self, pane: Pane, at: (f32, f32)) {
		let thread = pane == Pane::Main
			&& self.state().is_some_and(|shared| {
				self.pane_open(pane)
					.is_some_and(|open| bridge::can_create_threads(&shared.read(), open.channel_id))
			});
		let poll = self.state().is_some_and(|shared| {
			self.pane_open(pane)
				.is_some_and(|open| bridge::can_create_polls(&shared.read(), open.channel_id))
		});
		self.composer_layer.open_menu(PlusMenu {
			pane,
			at,
			thread,
			poll,
			apps: pane == Pane::Main,
		});
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
		Link::install_command_actions(host, link, pane);
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

	fn install_command_actions(host: &ScriptHost, link: &Rc<Link>, pane: Pane) {
		let names = link.pane_composer(pane).names;
		let actions = Rc::downgrade(link);
		host.action(names.command_section, move |argument| {
			if let (Some(link), Some(section)) = (actions.upgrade(), position(&argument))
				&& link
					.pane_composer(pane)
					.composer
					.borrow_mut()
					.choose_section(section)
			{
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.option_changed, move |argument| {
			if let (Some(link), Some(index)) =
				(actions.upgrade(), field(&argument, "index").and_then(position))
			{
				let value = field(&argument, "value").and_then(text).unwrap_or_default();
				if link
					.pane_composer(pane)
					.composer
					.borrow_mut()
					.edit_option(index, value)
				{
					link.ask_autocomplete(pane, false);
					link.publish_composer_all(pane);
				}
			}
		});
		let actions = Rc::downgrade(link);
		host.function(names.option_key, move |argument| {
			let consumed = actions
				.upgrade()
				.is_some_and(|link| link.option_key(pane, &argument));
			Value::Bool(consumed)
		});
		let actions = Rc::downgrade(link);
		host.action(names.option_focus, move |argument| {
			if let (Some(link), Some(index)) = (actions.upgrade(), position(&argument))
				&& link.pane_composer(pane).composer.borrow_mut().focus_option(index)
			{
				link.focus_command(pane);
				link.ask_autocomplete(pane, false);
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.toggle_optional, move |_| {
			if let Some(link) = actions.upgrade()
				&& link.pane_composer(pane).composer.borrow_mut().toggle_optional()
			{
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.add_optional, move |argument| {
			if let Some(link) = actions.upgrade()
				&& link
					.pane_composer(pane)
					.composer
					.borrow_mut()
					.add_optional(position(&argument))
			{
				link.focus_command(pane);
				link.ask_autocomplete(pane, false);
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.cancel_command, move |_| {
			if let Some(link) = actions.upgrade()
				&& link.pane_composer(pane).composer.borrow_mut().cancel_command()
			{
				link.replace_text(pane, Some((String::new(), 0)));
				link.request_focus(pane);
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.choose_option, move |argument| {
			if let Some(link) = actions.upgrade() {
				link.choose_option(pane, position(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.choose_result, move |argument| {
			if let Some(link) = actions.upgrade() {
				link.choose_result(pane, position(&argument));
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.retry_choices, move |_| {
			if let Some(link) = actions.upgrade() {
				link.ask_autocomplete(pane, true);
				link.publish_composer_all(pane);
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.option_files, move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(path) = texts(&argument).into_iter().next()
			{
				link.composer_option_path(pane, PathBuf::from(path));
			}
		});
		let actions = Rc::downgrade(link);
		host.action(names.remove_option_file, move |argument| {
			if let (Some(link), Some(index)) = (actions.upgrade(), position(&argument)) {
				let removed = link
					.pane_composer(pane)
					.composer
					.borrow_mut()
					.clear_option_file(index);
				if removed.is_some() {
					link.focus_command(pane);
					link.publish_composer_all(pane);
				}
			}
		});
	}

	fn ask_autocomplete(&self, pane: Pane, retry: bool) {
		let request = self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.autocomplete_request(retry);
		self.composer_send(request.into_iter().collect());
		self.ask_strip(pane);
	}

	fn ask_strip(&self, pane: Pane) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let (commands, due) = {
			let mut composer = self.pane_composer(pane).composer.borrow_mut();
			let commands = composer.strip_requests(&shared.read(), Instant::now());
			(commands, composer.strip_due())
		};
		if let Some(due) = due {
			self.wake_gif_search(due);
		}
		let asked = !commands.is_empty();
		self.composer_send(commands);
		asked
	}

	pub(super) fn poll_strips(&self) {
		for pane in [Pane::Main, Pane::Thread] {
			if self.ask_strip(pane) {
				self.publish_composer_all(pane);
			}
		}
	}

	fn choose_result(&self, pane: Pane, index: Option<usize>) {
		let Some(shared) = self.state() else {
			return;
		};
		let outcome =
			self.pane_composer(pane)
				.composer
				.borrow_mut()
				.choose_result(&shared.read(), index, Utc::now());
		self.composer_outcome(pane, outcome);
		self.request_focus(pane);
	}

	fn gif_usage(&self, pane: Pane, commands: &mut Vec<session::Command>) {
		if !self.settings.settings().advanced.share_gif_usage {
			return;
		}
		let Some(query) = self
			.pane_composer(pane)
			.composer
			.borrow()
			.result_query()
			.map(str::to_owned)
		else {
			return;
		};
		let sent = commands.iter().find_map(|command| match command {
			session::Command::SendGif { gif, .. } => Some(gif.id.clone()),
			_ => None,
		});
		if let Some(id) = sent {
			commands.insert(0, session::Command::SelectGif { id, query });
		}
	}

	fn choose_option(&self, pane: Pane, index: Option<usize>) {
		let Some(shared) = self.state() else {
			return;
		};
		let picked = self
			.pane_composer(pane)
			.composer
			.borrow_mut()
			.pick_choice(&shared.read(), index);
		if picked {
			self.focus_command(pane);
			self.ask_autocomplete(pane, false);
			self.publish_composer(pane, &shared.read());
		}
	}

	fn composer_option_path(&self, pane: Pane, path: PathBuf) {
		let Some(shared) = self.state() else {
			return;
		};
		let view = self.pane_composer(pane);
		let Some(channel_id) = view.composer.borrow().channel() else {
			return;
		};
		let limit = shared.read().upload_limit(channel_id);
		let loads = view.loads.clone();
		view.runtime.spawn_blocking(move || {
			loads.push(match read_upload(&path, limit) {
				Ok(file) => Loaded::Option {
					thumbnail: thumbnail(&file.bytes, &file.content_type),
					file,
				},
				Err(reason) => Loaded::Failed(reason),
			})
		});
	}

	fn focus_command(&self, pane: Pane) {
		let view = self.pane_composer(pane);
		view.command_focus.set(view.command_focus.get() + 1);
	}

	fn option_key(&self, pane: Pane, event: &Value) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let view = self.pane_composer(pane);
		let key = field(event, "key").and_then(text).unwrap_or_default();
		let shift = flag(event, "shift");
		let empty = field(event, "value").and_then(text).is_none_or(str::is_empty);
		let index = field(event, "index").and_then(position);
		let (optional_open, focused, required, has_file) = {
			let composer = view.composer.borrow();
			let Some(command) = composer.command() else {
				return false;
			};
			let field = command.fields.get(index.unwrap_or(command.focused));
			(
				command.optional_open,
				index.unwrap_or(command.focused),
				field.is_none_or(|field| field.option.is_required()),
				field.is_some_and(|field| field.file.is_some()),
			)
		};
		let choosing = view
			.composer
			.borrow()
			.option_choices(&shared.read())
			.is_some_and(|choices| choices.count() > 0);
		let showing = view
			.composer
			.borrow()
			.results(&shared.read())
			.is_some_and(|results| !results.items.is_empty());
		let mut composer = view.composer.borrow_mut();
		let handled = match key {
			"ArrowDown" | "ArrowUp" if showing && !optional_open => {
				composer.move_result(&shared.read(), if key == "ArrowDown" { 1 } else { -1 })
			}
			"ArrowDown" | "ArrowUp" if optional_open => {
				composer.move_selection(if key == "ArrowDown" { 1 } else { -1 });
				true
			}
			"ArrowDown" | "ArrowUp" if choosing => {
				composer.move_choice(&shared.read(), if key == "ArrowDown" { 1 } else { -1 })
			}
			"Enter" | "Tab" if choosing && !shift => {
				drop(composer);
				self.choose_option(pane, None);
				return true;
			}
			"Backspace" if has_file => composer.clear_option_file(focused).is_some(),
			"Enter" if optional_open && !shift => composer.add_optional(None),
			"Enter" if !shift => {
				let outcome = composer.submit(&shared.read(), "", Utc::now());
				drop(composer);
				self.composer_outcome(pane, outcome);
				self.focus_command(pane);
				self.publish_composer_all(pane);
				return true;
			}
			"Tab" => {
				let moved = composer.step_option(if shift { -1 } else { 1 });
				moved || (!shift && composer.toggle_optional())
			}
			"Escape" => composer.dismiss(),
			"Backspace" if empty && !required => composer.remove_option(focused),
			"Backspace" if empty && focused > 0 => composer.step_option(-1),
			"Backspace" if empty => {
				let restored = composer.leave_command();
				drop(composer);
				let left = restored.is_some();
				self.replace_text(pane, restored);
				self.request_focus(pane);
				self.publish_composer_all(pane);
				return left;
			}
			_ => return false,
		};
		drop(composer);
		if handled {
			self.focus_command(pane);
			self.ask_autocomplete(pane, false);
			self.publish_composer_all(pane);
		}
		handled || key == "Tab"
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
		if self.composer.composer.borrow().wants_file() {
			if let Some(path) = paths.first() {
				self.composer_option_path(Pane::Main, path.clone());
			}
			return;
		}
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
				self.close_apps();
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
			let commanding = changes.commands && view.composer.borrow().command().is_some();
			if refreshed || commanding {
				self.publish_composer_suggestions(pane, state);
			}
		}
		let stripping = view
			.composer
			.borrow()
			.command()
			.and_then(|command| command.entry.built_in())
			.is_some_and(bridge::BuiltIn::shows_results);
		if stripping && (!changes.loaded.is_empty() || changes.guild_list) {
			self.publish_composer_suggestions(pane, state);
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

	pub(super) fn composer_open_app(&self, app_id: Snowflake) {
		let opened = self.composer.composer.borrow_mut().open_app(app_id);
		if opened.is_some() {
			self.replace_text(Pane::Main, opened);
			self.request_focus(Pane::Main);
		}
	}

	pub(super) fn composer_pick_command(&self, entry: bridge::CommandEntry) {
		if !self.composer.composer.borrow_mut().pick_command(entry) {
			return;
		}
		self.composer.draft_due.set(None);
		self.focus_command(Pane::Main);
		self.ask_autocomplete(Pane::Main, false);
		self.replace_text(Pane::Main, Some((String::new(), 0)));
		if let Some(shared) = self.state() {
			self.publish_composer_suggestions(Pane::Main, &shared.read());
		}
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
		let Outcome {
			mut commands,
			replace,
			moderate,
			thread_message,
		} = outcome;
		let cleared = replace.as_ref().is_some_and(|(text, _)| text.is_empty());
		if cleared {
			self.pane_composer(pane).draft_due.set(None);
		}
		if let Some((parent, content)) = thread_message {
			self.await_first_message(parent, content);
		}
		self.gif_usage(pane, &mut commands);
		self.replace_text(pane, replace);
		self.composer_send(commands);
		if let Some(ask) = moderate {
			self.moderation.ask(&ask);
		}
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
		let picked = !echoed && view.composer.borrow_mut().exact_command(&shared.read(), &typed);
		if picked {
			self.replace_text(pane, Some((String::new(), 0)));
			self.focus_command(pane);
			self.ask_autocomplete(pane, false);
		}
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
		let suggesting = {
			let composer = view.composer.borrow();
			!composer.suggestions().is_empty()
				|| composer.picker().is_some_and(|picker| !picker.rows.is_empty())
		};
		match key {
			"ArrowDown" | "ArrowUp" if suggesting => {
				let delta = if key == "ArrowDown" { 1 } else { -1 };
				view.composer.borrow_mut().move_selection(delta);
				self.publish_composer_selected(pane);
				self.publish_composer_command(pane, &shared.read());
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
		if self.pane_composer(pane).composer.borrow().command().is_some() {
			self.focus_command(pane);
			self.ask_autocomplete(pane, false);
		}
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
			Loaded::Option { file, thumbnail } => {
				match composer.attach_option(&shared.read(), file) {
					Ok(id) => {
						if let Some(thumbnail) = thumbnail {
							let name = format!("{}/{id}", view.names.thumbnails);
							self.host.image(&name, Some(Arc::new(thumbnail)));
							view.thumbnails.borrow_mut().push((id, name));
						}
					}
					Err(reason) => composer.refuse(reason),
				}
				drop(composer);
				self.focus_command(pane);
				self.ask_autocomplete(pane, false);
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
		let options = composer.option_file_ids();
		view.thumbnails.borrow_mut().retain(|(id, name)| {
			let keep = options.contains(id) || composer.files().iter().any(|attachment| attachment.id == *id);
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
		images.extend(self.publish_composer_command(pane, state));
		let changed = *view.images.borrow() != images;
		if changed {
			*view.images.borrow_mut() = images;
			self.refresh_visible();
		}
	}

	fn app_visual(&self, app: Option<&CommandApp>, points: f64, images: &mut Vec<String>) -> Value {
		let Some(app) = app else {
			return record([("kind", "frequent".into())]);
		};
		if app.built_in() {
			return record([("kind", "built_in".into())]);
		}
		let image = match (app.id, app.icon.as_deref()) {
			(Some(id), Some(hash)) => {
				let request =
					bridge::app_icon_request(&self.images.cdn, id, hash, points, self.scale_factor());
				let name = self.images.gallery.request(request).to_string();
				images.push(name.clone());
				format!("host:{name}")
			}
			_ => String::new(),
		};
		let letter: String = app.name.chars().take(1).flat_map(char::to_uppercase).collect();
		record([
			("kind", "app".into()),
			("letter", letter.as_str().into()),
			("hue", (app.hue as f64).into()),
			("image", image.as_str().into()),
		])
	}

	fn picker_record(&self, picker: &CommandPicker, images: &mut Vec<String>) -> Value {
		let mut items = Vec::new();
		for (number, section) in picker.sections.iter().enumerate() {
			items.push(record([
				("key", format!("section:{number}").as_str().into()),
				("kind", "header".into()),
				("title", section.title.as_str().into()),
				(
					"app",
					self.app_visual(section.app.as_ref(), HEADER_ICON_POINTS, images),
				),
			]));
			for index in section.first..section.first + section.count {
				let entry = &picker.rows[index];
				let chips: Vec<Value> = entry
					.chips()
					.map(|option| {
						record([
							("key", option.name.as_str().into()),
							("name", option.name.as_str().into()),
						])
					})
					.collect();
				items.push(record([
					("key", format!("row:{number}:{}", entry.key).as_str().into()),
					("kind", "row".into()),
					("index", ((index + 1) as f64).into()),
					("name", format!("/{}", entry.name()).as_str().into()),
					("description", entry.description.as_str().into()),
					("app_name", entry.app.name.as_str().into()),
					("frequent", section.app.is_none().into()),
					("app", self.app_visual(Some(&entry.app), APP_ICON_POINTS, images)),
					("chips", Value::List(chips.into())),
				]));
			}
		}
		let rail: Vec<Value> = picker
			.sections
			.iter()
			.enumerate()
			.map(|(number, section)| {
				record([
					("key", format!("rail:{number}:{}", section.title).as_str().into()),
					("index", ((number + 1) as f64).into()),
					("title", section.title.as_str().into()),
					(
						"app",
						self.app_visual(section.app.as_ref(), APP_ICON_POINTS, images),
					),
				])
			})
			.collect();
		record([
			("open", true.into()),
			("items", Value::List(items.into())),
			("rail", Value::List(rail.into())),
			("selected", ((picker.selected + 1) as f64).into()),
			(
				"section",
				(picker.selected_section().map_or(0, |section| section + 1) as f64).into(),
			),
		])
	}

	fn file_record(&self, view: &ComposerView, file: Option<&OptionFile>) -> Value {
		let Some(file) = file else {
			return record([("name", "".into()), ("image", "".into())]);
		};
		let image = view
			.thumbnails
			.borrow()
			.iter()
			.find(|(id, _)| *id == file.id)
			.map(|(_, name)| format!("host:{name}"))
			.unwrap_or_default();
		record([
			("name", file.file.name.as_str().into()),
			("image", image.as_str().into()),
		])
	}

	fn result_image(&self, result: &CommandResult, ratio: f64, images: &mut Vec<String>) -> String {
		let scale = self.scale_factor();
		let request = match result {
			CommandResult::Gif(gif) => {
				bridge::gif_still_request(gif, (RESULT_POINTS * ratio, RESULT_POINTS), scale)
			}
			CommandResult::Sticker(cell) => bridge::sticker_request(&cell.item(), RESULT_POINTS, scale),
		};
		let Some(mut request) = request else {
			return String::new();
		};
		request.url = bridge::local_media(&request.url, self.images.mock_origin.as_deref());
		let name = self.images.gallery.request(request).to_string();
		images.push(name.clone());
		format!("host:{name}")
	}

	fn results_record(&self, results: Option<CommandResults>, images: &mut Vec<String>) -> Value {
		let Some(results) = results else {
			return record([("open", false.into())]);
		};
		let items: Vec<Value> = results
			.items
			.iter()
			.enumerate()
			.map(|(index, result)| {
				let (kind, key, ratio) = match result {
					CommandResult::Gif(gif) => {
						let ratio = if gif.width > 0 && gif.height > 0 {
							(f64::from(gif.width) / f64::from(gif.height)).clamp(MIN_RATIO, MAX_RATIO)
						} else {
							1.
						};
						("gif", gif.id.clone(), ratio)
					}
					CommandResult::Sticker(cell) => ("sticker", cell.sticker.id.to_string(), 1.),
				};
				let image = self.result_image(result, ratio, images);
				record([
					("key", format!("{kind}:{key}").as_str().into()),
					("index", ((index + 1) as f64).into()),
					("kind", kind.into()),
					("ratio", ratio.into()),
					("image", image.as_str().into()),
				])
			})
			.collect();
		record([
			("open", true.into()),
			("selected", ((results.selected + 1) as f64).into()),
			("status", results.status.unwrap_or_default().into()),
			("items", Value::List(items.into())),
		])
	}

	fn choices_record(choices: Option<OptionChoices>) -> Value {
		let Some(choices) = choices else {
			return record([("open", false.into())]);
		};
		let (kind, items) = match &choices.list {
			ChoiceList::Choices(list) => (
				"choices",
				list.iter()
					.enumerate()
					.map(|(index, choice)| {
						record([
							("key", format!("{index}:{}", choice.name).as_str().into()),
							("index", ((index + 1) as f64).into()),
							("name", choice.name.as_str().into()),
						])
					})
					.collect(),
			),
			ChoiceList::Loading => ("loading", Vec::new()),
			ChoiceList::Failed => ("failed", Vec::new()),
		};
		record([
			("open", true.into()),
			("kind", kind.into()),
			("title", choices.title.as_str().into()),
			("items", Value::List(items.into())),
			("selected", ((choices.selected + 1) as f64).into()),
		])
	}

	fn command_record(
		&self,
		view: &ComposerView,
		command: &ActiveCommand,
		(choices, results): (Option<OptionChoices>, Option<CommandResults>),
		images: &mut Vec<String>,
	) -> Value {
		let fields: Vec<Value> = command
			.fields
			.iter()
			.enumerate()
			.map(|(index, field)| {
				record([
					("key", field.option.name.as_str().into()),
					("index", ((index + 1) as f64).into()),
					("name", field.option.name.as_str().into()),
					("value", field.value.as_str().into()),
					("focused", (index == command.focused).into()),
					("invalid", field.error.is_some().into()),
					("takes_file", field.takes_file().into()),
					("file", self.file_record(view, field.file.as_ref())),
				])
			})
			.collect();
		let optional: Vec<Value> = command
			.optional()
			.into_iter()
			.map(|option| {
				record([
					("key", option.name.as_str().into()),
					("name", option.name.as_str().into()),
					(
						"description",
						option.description.as_deref().unwrap_or_default().into(),
					),
				])
			})
			.collect();
		let hint = command.hint();
		record([
			("active", true.into()),
			("serial", (view.command_focus.get() as f64).into()),
			(
				"app",
				self.app_visual(Some(&command.entry.app), COMMAND_ICON_POINTS, images),
			),
			("name", format!("/{}", command.entry.name()).as_str().into()),
			("focused", ((command.focused + 1) as f64).into()),
			("fields", Value::List(fields.into())),
			(
				"hint",
				record([
					("name", hint.name.as_str().into()),
					("description", hint.description.as_str().into()),
					("error", hint.error.into()),
				]),
			),
			("optional_count", (optional.len() as f64).into()),
			("optional_open", command.optional_open.into()),
			(
				"optional_selected",
				((command.optional_selected + 1) as f64).into(),
			),
			("optional", Value::List(optional.into())),
			("choices", Self::choices_record(choices)),
			("results", self.results_record(results, images)),
			("wants_file", command.wants_file().into()),
		])
	}

	fn publish_composer_command(&self, pane: Pane, state: &State) -> Vec<String> {
		let view = self.pane_composer(pane);
		let composer = view.composer.borrow();
		let mut images = Vec::new();
		let picker = match composer.picker().filter(|picker| !picker.rows.is_empty()) {
			Some(picker) => self.picker_record(picker, &mut images),
			None => record([("open", false.into())]),
		};
		let command = match composer.command() {
			Some(command) => {
				let choices = command.choices(state.autocomplete());
				let results = composer.results(state);
				self.command_record(view, command, (choices, results), &mut images)
			}
			None => record([("active", false.into())]),
		};
		drop(composer);
		view.picker.set(picker);
		view.command.set(command);
		images
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
			capture::ComposerShot::CommandPicker
			| capture::ComposerShot::CommandOptions
			| capture::ComposerShot::CommandOptional
			| capture::ComposerShot::CommandRemind
			| capture::ComposerShot::CommandInvalid
			| capture::ComposerShot::CommandChoices
			| capture::ComposerShot::CommandChoicesLoading
			| capture::ComposerShot::CommandChoicesFailed
			| capture::ComposerShot::CommandFile
			| capture::ComposerShot::CommandFilePicked
			| capture::ComposerShot::CommandOnlyOptional => self.drive_command(shot).await,
			capture::ComposerShot::CommandReplies => self.drive_command_replies().await,
			capture::ComposerShot::CommandBuiltIn
			| capture::ComposerShot::BuiltInGif
			| capture::ComposerShot::BuiltInSticker
			| capture::ComposerShot::BuiltInThread => self.drive_built_in(shot).await,
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

	async fn drive_command(&self, shot: capture::ComposerShot) {
		let Some(channel_id) = self.composer.composer.borrow().channel() else {
			return;
		};
		self.send(session::Command::LoadApplicationCommands { channel_id });
		self.replace_text(Pane::Main, Some(("/".to_owned(), 1)));
		self.request_focus(Pane::Main);
		let frequent = shot == capture::ComposerShot::CommandPicker;
		self.until_capture(move |link| {
			link.composer.composer.borrow().picker().is_some_and(|picker| {
				picker.rows.iter().any(|row| row.app.name == "Remindy")
					&& (!frequent
						|| picker
							.sections
							.first()
							.is_some_and(|section| section.app.is_none()))
			})
		})
		.await;
		let (name, value, optional) = match shot {
			capture::ComposerShot::CommandOptions => ("roll", "", false),
			capture::ComposerShot::CommandOptional => ("roll", "20", true),
			capture::ComposerShot::CommandRemind => ("remind", "", false),
			capture::ComposerShot::CommandInvalid => ("roll", "1000", false),
			capture::ComposerShot::CommandChoices => ("echo", CAPTURE_OPTION_TEXT, false),
			capture::ComposerShot::CommandChoicesLoading | capture::ComposerShot::CommandChoicesFailed => {
				("translate", CAPTURE_OPTION_TEXT, false)
			}
			capture::ComposerShot::CommandFile | capture::ComposerShot::CommandFilePicked => {
				("resize", "", false)
			}
			capture::ComposerShot::CommandOnlyOptional => ("lookup", "", false),
			_ => return,
		};
		{
			let mut composer = self.composer.composer.borrow_mut();
			let index = composer
				.picker()
				.and_then(|picker| picker.rows.iter().position(|row| row.name() == name));
			composer.choose("", index);
			composer.edit_option(0, value);
			if optional {
				composer.toggle_optional();
			}
			if !value.is_empty() && name != "roll" {
				composer.step_option(1);
			}
		}
		self.focus_command(Pane::Main);
		self.ask_autocomplete(Pane::Main, false);
		self.replace_text(Pane::Main, Some((String::new(), 0)));
		self.publish_composer_all(Pane::Main);
		if shot == capture::ComposerShot::CommandFilePicked {
			self.composer.loads.push(capture_cat());
		}
		self.until_capture(move |link| link.command_shot_ready(shot))
			.await;
		if shot == capture::ComposerShot::CommandOnlyOptional {
			self.pick_optional_by_hand().await;
		}
	}

	async fn pick_optional_by_hand(&self) {
		zgx::timer::sleep(capture::SETTLE).await;
		self.click_text(CAPTURE_OPTIONAL_PICK);
		self.until_capture(|link| {
			link.composer
				.composer
				.borrow()
				.command()
				.is_some_and(|command| command.fields.len() == 1)
		})
		.await;
	}

	fn click_text(&self, wanted: &str) {
		let Some(bounds) = self.rightmost_text(wanted) else {
			tracing::warn!(wanted, "the text to click is not on screen");
			return;
		};
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(zgx::widgets::WeakUi::upgrade) else {
			return;
		};
		let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
		let button = zgx::input::PointerButton::Primary;
		ui.dispatch(zgx::input::InputEvent::PointerMove { x, y });
		ui.dispatch(zgx::input::InputEvent::PointerDown { x, y, button });
		ui.dispatch(zgx::input::InputEvent::PointerUp { x, y, button });
	}

	async fn drive_built_in(&self, shot: capture::ComposerShot) {
		let Some(channel_id) = self.composer.composer.borrow().channel() else {
			return;
		};
		self.send(session::Command::LoadApplicationCommands { channel_id });
		self.replace_text(Pane::Main, Some(("/".to_owned(), 1)));
		self.request_focus(Pane::Main);
		self.until_capture(|link| {
			link.composer
				.composer
				.borrow()
				.picker()
				.is_some_and(|picker| picker.rows.iter().any(|row| row.app.name == "Remindy"))
		})
		.await;
		let (name, values): (&str, &[&str]) = match shot {
			capture::ComposerShot::BuiltInGif => ("gif", &[CAPTURE_GIF_QUERY]),
			capture::ComposerShot::BuiltInSticker => ("sticker", &[CAPTURE_STICKER_QUERY]),
			capture::ComposerShot::BuiltInThread => ("thread", &CAPTURE_THREAD),
			_ => {
				zgx::timer::sleep(capture::SETTLE).await;
				let found = self.composer.composer.borrow().picker().and_then(|picker| {
					let gif = picker
						.rows
						.iter()
						.position(|row| row.built_in() == Some(bridge::BuiltIn::Gif))?;
					let section = picker
						.sections
						.iter()
						.position(|section| (section.first..section.first + section.count).contains(&gif))?;
					Some((section, gif - picker.sections[section].first))
				});
				let Some((section, offset)) = found else {
					return;
				};
				self.composer.composer.borrow_mut().choose_section(section);
				self.publish_composer_all(Pane::Main);
				zgx::timer::sleep(capture::SETTLE).await;
				self.composer.composer.borrow_mut().move_selection(offset as i32);
				self.publish_composer_all(Pane::Main);
				return;
			}
		};
		{
			let mut composer = self.composer.composer.borrow_mut();
			let index = composer.picker().and_then(|picker| {
				picker
					.rows
					.iter()
					.position(|row| row.built_in().is_some() && row.name() == name)
			});
			composer.choose("", index);
			for (field, value) in values.iter().enumerate() {
				composer.edit_option(field, value);
			}
			composer.focus_option(0);
		}
		self.focus_command(Pane::Main);
		self.ask_autocomplete(Pane::Main, false);
		self.replace_text(Pane::Main, Some((String::new(), 0)));
		self.publish_composer_all(Pane::Main);
		if shot == capture::ComposerShot::BuiltInThread {
			return;
		}
		self.until_capture(|link| {
			link.state().is_some_and(|shared| {
				link.composer
					.composer
					.borrow()
					.results(&shared.read())
					.is_some_and(|results| results.items.len() > 1)
			})
		})
		.await;
		self.publish_composer_all(Pane::Main);
	}

	async fn drive_command_replies(&self) {
		let Some(channel_id) = self.composer.composer.borrow().channel() else {
			return;
		};
		self.send(session::Command::LoadApplicationCommands { channel_id });
		self.until_capture(move |link| {
			link.state()
				.is_some_and(|shared| shared.read().command_index(channel_id).is_some())
		})
		.await;
		let steps: [ReplyStep; 5] = [
			(mockcord::THINK_COMMAND, "think", |entry| {
				entry.reply_state == Some(bridge::ReplyState::Thinking)
			}),
			(mockcord::ROLL_COMMAND, "roll", |entry| {
				entry.reply_state.is_none() && !entry.content.is_empty()
			}),
			(mockcord::WHISPER_COMMAND, "whisper", |entry| entry.ephemeral),
			(mockcord::PING_COMMAND, "ping", |entry| {
				entry.reply_state == Some(bridge::ReplyState::Sending)
			}),
			(mockcord::BROKEN_COMMAND, "broken", |entry| {
				entry
					.reply_state
					.as_ref()
					.is_some_and(|state| state.failure().is_some())
			}),
		];
		for (command, name, shown) in steps {
			let options = if command == mockcord::ROLL_COMMAND {
				vec![protocol::InteractionOption {
					kind: protocol::CommandOptionType::Integer,
					name: "sides".to_owned(),
					value: Some(serde_json::json!(CAPTURE_ROLL_SIDES)),
					options: Vec::new(),
					focused: None,
				}]
			} else {
				Vec::new()
			};
			self.send(session::Command::RunApplicationCommand {
				channel_id,
				command_id: protocol::Snowflake(command),
				options,
				files: Vec::new(),
			});
			self.until_capture(move |link| {
				link.timeline.borrow().messages().any(|entry| {
					entry
						.interaction
						.as_ref()
						.is_some_and(|line| line.command == name)
						&& shown(entry)
				})
			})
			.await;
		}
	}

	fn command_shot_ready(&self, shot: capture::ComposerShot) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let composer = self.composer.composer.borrow();
		let Some(command) = composer.command() else {
			return false;
		};
		let list = composer
			.option_choices(&shared.read())
			.map(|choices| choices.list);
		match shot {
			capture::ComposerShot::CommandChoices => matches!(list, Some(ChoiceList::Choices(_))),
			capture::ComposerShot::CommandChoicesLoading => {
				list == Some(ChoiceList::Loading)
					&& shared
						.read()
						.autocomplete()
						.is_some_and(|autocomplete| autocomplete.status == state::AutocompleteStatus::Loading)
			}
			capture::ComposerShot::CommandChoicesFailed => list == Some(ChoiceList::Failed),
			capture::ComposerShot::CommandFile => command.wants_file(),
			capture::ComposerShot::CommandFilePicked => {
				!command.file_ids().is_empty()
					&& self
						.composer
						.thumbnails
						.borrow()
						.iter()
						.any(|(id, _)| command.file_ids().contains(id))
			}
			_ => true,
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
