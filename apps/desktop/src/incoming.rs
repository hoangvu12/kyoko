use std::io::Cursor;
use std::path::{Path, PathBuf};

use chrono::Utc;
use platform::clipboard::RgbaImage;
use rest::UploadFile;

pub const MAX_READ_BYTES: u64 = 500 * 1024 * 1024;

pub trait Clipboard {
	fn files(&self) -> Vec<PathBuf>;
	fn image(&self) -> Option<RgbaImage>;
}

pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
	fn files(&self) -> Vec<PathBuf> {
		platform::clipboard::read_files()
			.inspect_err(|error| tracing::debug!(%error, "the clipboard files could not be read"))
			.unwrap_or_default()
			.into_iter()
			.filter(|path| path.exists())
			.collect()
	}

	fn image(&self) -> Option<RgbaImage> {
		platform::clipboard::read_image()
			.inspect_err(|error| tracing::debug!(%error, "the clipboard image could not be read"))
			.ok()
			.flatten()
	}
}

pub enum Pasted {
	Files(Vec<PathBuf>),
	Image(RgbaImage),
	Nothing,
}

pub fn pasted(clipboard: &dyn Clipboard) -> Pasted {
	let files = clipboard.files();
	if !files.is_empty() {
		return Pasted::Files(files);
	}
	clipboard.image().map_or(Pasted::Nothing, Pasted::Image)
}

fn file_name(path: &Path) -> String {
	path.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| "file".to_owned())
}

pub fn read_upload(path: &Path, limit: u64) -> Result<UploadFile, String> {
	let name = file_name(path);
	match std::fs::metadata(path) {
		Ok(metadata) if metadata.is_dir() => return Err(format!("{name} is a folder")),
		Ok(metadata) => {
			if let Some(refusal) = bridge::too_big(&name, metadata.len(), limit.min(MAX_READ_BYTES)) {
				return Err(refusal);
			}
		}
		Err(_) => return Err(format!("{name} could not be read")),
	}
	let bytes = std::fs::read(path).map_err(|_| format!("{name} could not be read"))?;
	Ok(UploadFile::named(name, bytes))
}

pub fn too_many(free: usize, wanted: usize) -> Option<String> {
	(wanted > free).then(|| format!("You can attach up to {} files", state::MAX_ATTACHMENTS))
}

pub fn pasted_image(image: RgbaImage) -> Result<UploadFile, String> {
	let (Ok(width), Ok(height)) = (u32::try_from(image.width), u32::try_from(image.height)) else {
		return Err("The pasted image is too large".to_owned());
	};
	let Some(buffer) = image::RgbaImage::from_raw(width, height, image.bytes) else {
		return Err("The pasted image could not be read".to_owned());
	};
	let mut encoded = Cursor::new(Vec::new());
	if buffer.write_to(&mut encoded, image::ImageFormat::Png).is_err() {
		return Err("The pasted image could not be read".to_owned());
	}
	Ok(UploadFile::named(
		format!("image-{}.png", Utc::now().format("%Y%m%d-%H%M%S")),
		encoded.into_inner(),
	))
}
