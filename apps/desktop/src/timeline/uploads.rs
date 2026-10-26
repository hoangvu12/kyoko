use std::rc::Rc;

use bridge::MessageEntry;
use protocol::Nonce;
use session::Command;
use state::UploadProgress;
use zgx::compose::prelude::*;

use super::icons::icon;
use super::rows::{RowEnv, TypeSize, tok};

const CARD_WIDTH: f32 = 400.;
const CARD_ICON: f32 = 24.;
const CANCEL_ICON: f32 = 20.;
const BAR_HEIGHT: f32 = 4.;
pub const CARD_HEIGHT: f32 = 66.;
pub const FAILED_HEIGHT: f32 = 20.;

pub fn upload(entry: &MessageEntry, env: &Rc<RowEnv>) -> Option<View> {
	let upload = entry.upload.as_ref()?;
	let nonce = entry.message.nonce.clone()?;
	let channel_id = entry.message.channel_id;
	if entry.failed {
		return Some(failed(channel_id, nonce, env));
	}
	Some(progress(upload, channel_id, nonce, env))
}

pub fn height(entry: &MessageEntry) -> f32 {
	match &entry.upload {
		Some(_) if entry.failed => FAILED_HEIGHT,
		Some(_) => CARD_HEIGHT,
		None => 0.,
	}
}

fn label(upload: &UploadProgress) -> String {
	let name = match upload.files.as_slice() {
		[file] => file.name.clone(),
		files => format!("{} files", files.len()),
	};
	format!(
		"{name} · {} of {}",
		bridge::file_size(upload.sent.min(upload.total())),
		bridge::file_size(upload.total())
	)
}

fn fraction(upload: &UploadProgress) -> f32 {
	let total = upload.total();
	if total == 0 {
		return 1.;
	}
	(upload.sent.min(total) as f64 / total as f64) as f32
}

fn video(upload: &UploadProgress) -> bool {
	upload.files.iter().all(|file| {
		let name = file.name.to_ascii_lowercase();
		[".mp4", ".webm", ".mov"].iter().any(|kind| name.ends_with(kind))
	})
}

fn progress(
	upload: &UploadProgress,
	channel_id: protocol::Snowflake,
	nonce: Nonce,
	env: &Rc<RowEnv>,
) -> View {
	let cancel = env.clone();
	let id = nonce.0.clone();
	row()
		.id(format!("upload-progress-{id}"))
		.mt(4.)
		.w(CARD_WIDTH)
		.max_w_percent(100.)
		.items_center()
		.gap(tok("space_12"))
		.p(tok("space_12"))
		.rounded(tok("radius_md"))
		.bg(tok("bg_3"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.child(icon(
			if video(upload) { "play" } else { "file" },
			CARD_ICON,
			"text_4",
		))
		.child(
			column()
				.grow()
				.min_w(0.)
				.gap(tok("space_6"))
				.child(
					text(label(upload))
						.truncate()
						.type_size("font_sm")
						.font_weight(500)
						.text_color(tok("text_3")),
				)
				.child(
					div()
						.w_full()
						.h(BAR_HEIGHT)
						.rounded(BAR_HEIGHT)
						.overflow_hidden()
						.bg(tok("bg_2"))
						.child(
							div()
								.h_full()
								.w_percent(fraction(upload) * 100.)
								.rounded(BAR_HEIGHT)
								.bg(tok("accent_2")),
						),
				),
		)
		.child(
			div()
				.id(format!("upload-cancel-{id}"))
				.size(CANCEL_ICON, CANCEL_ICON)
				.items_center()
				.justify_center()
				.shrink_0()
				.cursor(Cursor::Pointer)
				.on_click(move || {
					cancel.send(Command::DiscardMessage {
						channel_id,
						nonce: nonce.clone(),
					})
				})
				.child(icon("close", CANCEL_ICON, "text_4")),
		)
}

fn failed(channel_id: protocol::Snowflake, nonce: Nonce, env: &Rc<RowEnv>) -> View {
	let (retry, remove) = (env.clone(), env.clone());
	let (retried, removed) = (nonce.clone(), nonce.clone());
	let link = |label: &'static str| {
		text(label)
			.type_size("font_tiny")
			.font_weight(500)
			.text_color(tok("link"))
			.cursor(Cursor::Pointer)
	};
	row()
		.id(format!("upload-failed-{}", nonce.0))
		.items_center()
		.gap(tok("space_8"))
		.child(
			text("Upload failed")
				.type_size("font_tiny")
				.font_weight(500)
				.text_color(tok("red_2")),
		)
		.child(link("Retry").on_click(move || {
			retry.send(Command::RetryMessage {
				channel_id,
				nonce: retried.clone(),
			})
		}))
		.child(link("Remove").on_click(move || {
			remove.send(Command::DiscardMessage {
				channel_id,
				nonce: removed.clone(),
			})
		}))
}
