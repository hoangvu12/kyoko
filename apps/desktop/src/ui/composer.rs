use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{Composer, Inbox, Notice, Outcome, Suggestion, Visual};
use chrono::Utc;
use protocol::{Nonce, Snowflake};
use rest::UploadFile;
use state::{Changes, State};
use tokio::runtime::Handle;
use zgx::image::ImageData;
use zgx_script::{ScriptHost, Shared, Value};

use super::{Link, field, record, snowflake, text, texts};
use crate::capture;

const DRAFT_DELAY: Duration = Duration::from_millis(500);
const TICK: Duration = Duration::from_millis(250);
const SUGGESTION_POINTS: f64 = 24.;
const THUMBNAIL_PIXELS: u32 = 96;
type Handler = fn(&Link);

const CAPTURE_DRAFT: &str = "Checking how the composer looks with a draft in it";

enum Loaded {
	File {
		file: UploadFile,
		thumbnail: Option<ImageData>,
	},
	Failed(String),
}

pub(super) struct ComposerView {
	composer: RefCell<Composer>,
	runtime: Handle,
	loads: Inbox<Loaded>,
	serial: Cell<u64>,
	focus_serial: Cell<u64>,
	replaced: RefCell<Option<String>>,
	text: RefCell<String>,
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
	selected: Shared,
	typing: Shared,
	notice: Shared,
	drop_hover: Shared,
	limit: Shared,
	open: Shared,
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

fn read_file(path: PathBuf, limit: u64) -> Loaded {
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| "file".to_owned());
	let too_large = || {
		Loaded::Failed(format!(
			"{name} is too large. The limit here is {} MB.",
			(limit as f64 / (1024.0 * 1024.0)).round()
		))
	};
	match std::fs::metadata(&path) {
		Ok(metadata) if metadata.is_dir() => return Loaded::Failed(format!("{name} is a folder")),
		Ok(metadata) if metadata.len() > limit => return too_large(),
		Ok(_) => {}
		Err(_) => return Loaded::Failed(format!("{name} could not be read")),
	}
	match std::fs::read(&path) {
		Ok(bytes) => {
			let kind = content_type(&name);
			Loaded::File {
				thumbnail: thumbnail(&bytes, kind),
				file: UploadFile {
					name,
					bytes: bytes.into(),
					content_type: kind.to_owned(),
				},
			}
		}
		Err(_) => Loaded::Failed(format!("{name} could not be read")),
	}
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
		file: UploadFile {
			name: format!("image-{}.png", Utc::now().format("%Y%m%d-%H%M%S")),
			bytes: bytes.into(),
			content_type: "image/png".to_owned(),
		},
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
	pub(super) fn install(host: &ScriptHost, runtime: Handle) -> ComposerView {
		ComposerView {
			composer: RefCell::new(Composer::default()),
			runtime,
			loads: Inbox::new(),
			serial: Cell::new(0),
			focus_serial: Cell::new(0),
			replaced: RefCell::new(None),
			text: RefCell::new(String::new()),
			draft_due: Cell::new(None),
			images: RefCell::new(Vec::new()),
			thumbnails: RefCell::new(Vec::new()),
			lock_until: Cell::new(None),
			placeholder: host.shared("composer.placeholder", ""),
			replace: host.shared(
				"composer.replace",
				record([("serial", 0.0.into()), ("text", "".into()), ("caret", 0.0.into())]),
			),
			focus: host.shared("composer.focus", 0.0),
			reply: host.shared("composer.reply", record([("active", false.into())])),
			editing: host.shared("composer.editing", false),
			files: host.shared("composer.files", Value::List(Rc::new([]))),
			suggestions: host.shared("composer.suggestions", Value::List(Rc::new([]))),
			selected: host.shared("composer.selected", 1.0),
			typing: host.shared("composer.typing", ""),
			notice: host.shared("composer.notice", record([("kind", "".into())])),
			drop_hover: host.shared("composer.drop_hover", false),
			limit: host.shared("composer.limit", 2000.0),
			open: host.shared("composer.open", false),
		}
	}

	pub(super) fn images(&self) -> Vec<String> {
		self.images.borrow().clone()
	}
}

impl Link {
	pub(super) fn install_composer(host: &ScriptHost, link: &Rc<Link>) {
		let actions = Rc::downgrade(link);
		host.function("composer.changed", move |argument| {
			if let Some(link) = actions.upgrade() {
				let typed = field(&argument, "text")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.composer_changed(typed, caret_of(&argument));
			}
			Value::Nil
		});
		let actions = Rc::downgrade(link);
		host.function("composer.key", move |argument| {
			let consumed = actions.upgrade().is_some_and(|link| link.composer_key(&argument));
			Value::Bool(consumed)
		});
		let actions = Rc::downgrade(link);
		host.action("composer.choose", move |argument| {
			if let Some(link) = actions.upgrade() {
				let index = match field(&argument, "index") {
					Some(Value::Num(index)) if *index >= 1. => Some(*index as usize - 1),
					_ => None,
				};
				let typed = field(&argument, "text")
					.and_then(text)
					.unwrap_or_default()
					.to_owned();
				link.composer_choose(&typed, index);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.add_files", move |argument| {
			if let Some(link) = actions.upgrade() {
				let paths = texts(&argument).into_iter().map(PathBuf::from).collect();
				link.composer_add_paths(paths);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.remove_file", move |argument| {
			if let Some(link) = actions.upgrade()
				&& let Some(id) = text(&argument).and_then(|id| id.parse().ok())
			{
				link.composer_remove_file(id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.start_reply", move |argument| {
			if let (Some(link), Some(message_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.composer_reply(message_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.start_edit", move |argument| {
			if let (Some(link), Some(message_id)) = (actions.upgrade(), snowflake(&argument)) {
				link.composer_edit(message_id);
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.retry", move |argument| {
			if let Some(link) = actions.upgrade() {
				let nonce = Nonce(text(&argument).unwrap_or_default().to_owned());
				let command = link.composer.composer.borrow().retry(nonce);
				link.composer_send(command.into_iter().collect());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.discard", move |argument| {
			if let Some(link) = actions.upgrade() {
				let nonce = Nonce(text(&argument).unwrap_or_default().to_owned());
				let command = link.composer.composer.borrow().discard(nonce);
				link.composer_send(command.into_iter().collect());
			}
		});
		let actions = Rc::downgrade(link);
		host.action("composer.confirm", move |_| {
			if let Some(link) = actions.upgrade()
				&& let Some(shared) = link.state()
			{
				let outcome = link
					.composer
					.composer
					.borrow_mut()
					.confirm_lock(&shared.read(), Utc::now());
				link.composer_outcome(outcome);
			}
		});
		let simple: [(&str, Handler); 4] = [
			("composer.cancel_reply", |link| {
				link.composer.composer.borrow_mut().cancel_reply();
			}),
			("composer.toggle_ping", |link| {
				link.composer.composer.borrow_mut().toggle_ping()
			}),
			("composer.cancel_edit", |link| {
				link.composer_cancel_edit();
			}),
			("composer.dismiss", |link| {
				link.composer.composer.borrow_mut().dismiss();
			}),
		];
		for (name, handler) in simple {
			let actions = Rc::downgrade(link);
			host.action(name, move |_| {
				if let Some(link) = actions.upgrade() {
					handler(&link);
					link.publish_composer_all();
				}
			});
		}
	}

	pub(super) async fn follow_composer_files(self: Rc<Self>) {
		while let Some(batch) = self.composer.loads.next().await {
			for loaded in batch {
				self.composer_loaded(loaded);
			}
			self.publish_composer_all();
		}
	}

	pub(super) async fn tick_composer(self: Rc<Self>) {
		loop {
			zgx::timer::sleep(TICK).await;
			let now = Instant::now();
			if self.composer.draft_due.get().is_some_and(|due| due <= now) {
				self.flush_draft();
			}
			if self
				.composer
				.lock_until
				.get()
				.is_some_and(|until| until <= Utc::now())
			{
				self.composer.lock_until.set(None);
				self.publish_composer_notice();
			}
		}
	}

	pub(super) fn files_dropped(&self, paths: &[PathBuf]) {
		self.composer.drop_hover.set(false.into());
		self.composer_add_paths(paths.to_vec());
	}

	pub(super) fn files_hovering(&self, hovering: bool) {
		self.composer.drop_hover.set(hovering.into());
	}

	fn flush_draft(&self) {
		self.composer.draft_due.set(None);
		let composer = self.composer.composer.borrow();
		if composer.editing().is_some() {
			return;
		}
		if let Some(channel_id) = composer.channel() {
			self.send(session::Command::SetDraft {
				channel_id,
				content: self.composer.text.borrow().clone(),
			});
		}
	}

	pub(super) fn composer_apply(&self, state: &State, changes: &Changes) {
		let open = self.timeline.borrow().open();
		let target = open.map(|open| open.channel_id);
		let switched = self.composer.composer.borrow().channel() != target;
		if switched {
			if self.composer.draft_due.get().is_some() {
				self.flush_draft();
			}
			let outcome =
				self.composer
					.composer
					.borrow_mut()
					.open(state, open.and_then(|open| open.guild_id), target);
			self.replace_text(outcome.replace);
			self.publish_composer(state);
			return;
		}
		let Some(channel_id) = target else {
			return;
		};
		if changes.drafts.contains(channel_id) {
			let draft = self.composer.composer.borrow_mut().draft_arrived(state);
			if let Some(draft) = draft
				&& *self.composer.text.borrow() != draft.0
			{
				self.replace_text(Some(draft));
			}
		}
		if changes.typing.contains(channel_id) || changes.members.contains(channel_id) {
			self.publish_composer_typing(state);
		}
		if changes.commands
			|| !changes.members.is_empty()
			|| changes.messages.contains(channel_id)
			|| !changes.channel_lists.is_empty()
		{
			let refreshed = self.composer.composer.borrow_mut().refresh_suggestions(state);
			if refreshed {
				self.publish_composer_suggestions(state);
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
			self.publish_composer_notice_with(state);
		}
		if changes.current_user {
			self.composer.limit.set((state.message_limit() as f64).into());
		}
		if !changes.channel_lists.is_empty() || changes.private_channels {
			self.composer
				.placeholder
				.set(bridge::placeholder(state, channel_id).as_str().into());
		}
	}

	fn replace_text(&self, replace: Option<(String, usize)>) {
		let Some((replacement, caret)) = replace else {
			return;
		};
		let serial = self.composer.serial.get() + 1;
		self.composer.serial.set(serial);
		*self.composer.text.borrow_mut() = replacement.clone();
		*self.composer.replaced.borrow_mut() = Some(replacement.clone());
		self.composer.replace.set(record([
			("serial", (serial as f64).into()),
			("text", replacement.as_str().into()),
			("caret", (caret as f64).into()),
		]));
	}

	fn request_focus(&self) {
		let serial = self.composer.focus_serial.get() + 1;
		self.composer.focus_serial.set(serial);
		self.composer.focus.set((serial as f64).into());
	}

	fn composer_send(&self, commands: Vec<session::Command>) {
		for command in commands {
			self.send(command);
		}
	}

	fn composer_outcome(&self, outcome: Outcome) {
		let cleared = outcome.replace.as_ref().is_some_and(|(text, _)| text.is_empty());
		if cleared {
			self.composer.draft_due.set(None);
		}
		self.replace_text(outcome.replace);
		self.composer_send(outcome.commands);
		self.publish_composer_all();
	}

	fn composer_changed(&self, typed: String, caret: usize) {
		let Some(shared) = self.state() else {
			return;
		};
		let echoed = self.composer.replaced.borrow().as_deref() == Some(typed.as_str());
		*self.composer.text.borrow_mut() = typed.clone();
		let commands = {
			let state = shared.read();
			let mut composer = self.composer.composer.borrow_mut();
			if echoed {
				self.composer.replaced.borrow_mut().take();
				composer.restored(&state, &typed, caret);
				Vec::new()
			} else {
				self.composer.draft_due.set(Some(Instant::now() + DRAFT_DELAY));
				composer.changed(&state, &typed, caret)
			}
		};
		self.composer_send(commands);
		self.publish_composer_suggestions(&shared.read());
	}

	fn composer_key(&self, event: &Value) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let key = field(event, "key").and_then(text).unwrap_or_default();
		let typed = field(event, "text").and_then(text).unwrap_or_default().to_owned();
		let shift = flag(event, "shift");
		let suggesting = !self.composer.composer.borrow().suggestions().is_empty();
		match key {
			"ArrowDown" | "ArrowUp" if suggesting => {
				let delta = if key == "ArrowDown" { 1 } else { -1 };
				self.composer.composer.borrow_mut().move_selection(delta);
				self.publish_composer_selected();
				true
			}
			"Tab" | "Enter" if suggesting && !shift => {
				self.composer_choose(&typed, None);
				true
			}
			"Escape" => {
				let edited = self.composer_cancel_edit();
				let dismissed = edited || self.composer.composer.borrow_mut().dismiss();
				self.publish_composer_all();
				dismissed
			}
			"Enter" if !shift => {
				let outcome = self
					.composer
					.composer
					.borrow_mut()
					.submit(&shared.read(), &typed, Utc::now());
				self.composer_outcome(outcome);
				true
			}
			"ArrowUp" if typed.is_empty() && self.composer.composer.borrow().files().is_empty() => {
				let edit = self.composer.composer.borrow_mut().edit_last(&shared.read());
				let editing = edit.is_some();
				self.replace_text(edit);
				self.publish_composer_all();
				editing
			}
			"v" | "V" if flag(event, "cmd") => self.composer_paste(),
			_ => false,
		}
	}

	fn composer_choose(&self, typed: &str, index: Option<usize>) {
		let chosen = self.composer.composer.borrow_mut().choose(typed, index);
		self.replace_text(chosen);
		if let Some(shared) = self.state() {
			self.publish_composer_suggestions(&shared.read());
		}
	}

	fn composer_cancel_edit(&self) -> bool {
		let Some(shared) = self.state() else {
			return false;
		};
		let restored = self.composer.composer.borrow_mut().cancel_edit(&shared.read());
		let cancelled = restored.is_some();
		self.replace_text(restored);
		cancelled
	}

	fn composer_reply(&self, message_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		if self
			.composer
			.composer
			.borrow_mut()
			.start_reply(&shared.read(), message_id)
		{
			self.request_focus();
			self.publish_composer_all();
		}
	}

	fn composer_edit(&self, message_id: Snowflake) {
		let Some(shared) = self.state() else {
			return;
		};
		let edit = self
			.composer
			.composer
			.borrow_mut()
			.start_edit(&shared.read(), message_id);
		if edit.is_some() {
			self.replace_text(edit);
			self.request_focus();
			self.publish_composer_all();
		}
	}

	fn composer_paste(&self) -> bool {
		let files = platform::clipboard::read_files().unwrap_or_default();
		if !files.is_empty() {
			self.composer_add_paths(files);
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
		let loads = self.composer.loads.clone();
		self.composer
			.runtime
			.spawn_blocking(move || loads.push(pasted_image(image)));
		true
	}

	fn composer_add_paths(&self, paths: Vec<PathBuf>) {
		let Some(shared) = self.state() else {
			return;
		};
		let Some(channel_id) = self.composer.composer.borrow().channel() else {
			return;
		};
		let limit = shared.read().upload_limit(channel_id);
		for path in paths {
			let loads = self.composer.loads.clone();
			self.composer
				.runtime
				.spawn_blocking(move || loads.push(read_file(path, limit)));
		}
	}

	fn composer_loaded(&self, loaded: Loaded) {
		let Some(shared) = self.state() else {
			return;
		};
		let mut composer = self.composer.composer.borrow_mut();
		match loaded {
			Loaded::File { file, thumbnail } => match composer.attach(&shared.read(), file) {
				Ok(id) => {
					if let Some(thumbnail) = thumbnail {
						let name = format!("composer/{id}");
						self.host.image(&name, Some(Arc::new(thumbnail)));
						self.composer.thumbnails.borrow_mut().push((id, name));
					}
				}
				Err(reason) => composer.refuse(reason),
			},
			Loaded::Failed(reason) => composer.refuse(reason),
		}
		drop(composer);
		self.request_focus();
	}

	fn composer_remove_file(&self, id: u64) {
		if self.composer.composer.borrow_mut().remove_file(id) {
			self.composer.thumbnails.borrow_mut().retain(|(file, name)| {
				let keep = *file != id;
				if !keep {
					self.host.image(name, None);
				}
				keep
			});
			self.publish_composer_all();
		}
	}

	fn forget_thumbnails(&self) {
		let composer = self.composer.composer.borrow();
		self.composer.thumbnails.borrow_mut().retain(|(id, name)| {
			let keep = composer.files().iter().any(|attachment| attachment.id == *id);
			if !keep {
				self.host.image(name, None);
			}
			keep
		});
	}

	fn publish_composer_all(&self) {
		if let Some(shared) = self.state() {
			self.publish_composer(&shared.read());
		}
	}

	fn publish_composer(&self, state: &State) {
		self.forget_thumbnails();
		let composer = self.composer.composer.borrow();
		let channel_id = composer.channel();
		self.composer.open.set(channel_id.is_some().into());
		self.composer.placeholder.set(
			channel_id
				.map(|channel_id| bridge::placeholder(state, channel_id))
				.unwrap_or_default()
				.as_str()
				.into(),
		);
		self.composer.limit.set((state.message_limit() as f64).into());
		self.composer.editing.set(composer.editing().is_some().into());
		self.composer.reply.set(match composer.reply() {
			Some(reply) => record([
				("active", true.into()),
				("name", reply.name.as_str().into()),
				("ping", reply.ping.into()),
				("own", reply.own.into()),
			]),
			None => record([("active", false.into())]),
		});
		let thumbnails = self.composer.thumbnails.borrow();
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
		self.composer.files.set(Value::List(files.into()));
		drop(thumbnails);
		drop(composer);
		self.publish_composer_suggestions(state);
		self.publish_composer_typing(state);
		self.publish_composer_notice_with(state);
	}

	fn publish_composer_typing(&self, state: &State) {
		let line = self.composer.composer.borrow().typing(state, Instant::now());
		self.composer.typing.set(line.as_str().into());
	}

	fn publish_composer_selected(&self) {
		let selected = self.composer.composer.borrow().selected() + 1;
		self.composer.selected.set((selected as f64).into());
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

	fn publish_composer_suggestions(&self, state: &State) {
		let composer = self.composer.composer.borrow();
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
					("icon", icon.into()),
				])
			})
			.collect();
		let selected = composer.selected() + 1;
		drop(composer);
		self.composer.suggestions.set(Value::List(items.into()));
		self.composer.selected.set((selected as f64).into());
		let changed = *self.composer.images.borrow() != images;
		if changed {
			*self.composer.images.borrow_mut() = images;
			self.refresh_visible();
		}
	}

	fn publish_composer_notice(&self) {
		if let Some(shared) = self.state() {
			self.publish_composer_notice_with(&shared.read());
		}
	}

	fn publish_composer_notice_with(&self, state: &State) {
		let now = Utc::now();
		let mut composer = self.composer.composer.borrow_mut();
		self.composer
			.lock_until
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
		self.composer.notice.set(notice);
	}

	pub(super) fn drive_composer(&self, screen: &capture::Screen) {
		match screen {
			capture::Screen::Composer => {
				self.replace_text(Some((CAPTURE_DRAFT.to_owned(), CAPTURE_DRAFT.len())));
				self.request_focus();
			}
			capture::Screen::ComposerReply => {
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
			_ => {}
		}
	}

	pub(super) fn composer_settled(&self, screen: &capture::Screen) -> bool {
		match screen {
			capture::Screen::Composer => *self.composer.text.borrow() == CAPTURE_DRAFT,
			capture::Screen::ComposerReply => self.composer.composer.borrow().reply().is_some(),
			_ => true,
		}
	}
}
