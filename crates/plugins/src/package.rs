use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::manifest::{MANIFEST_FILE, Manifest};

const FILE_LIMIT: u64 = 2 * 1024 * 1024;
const FILES_PER_PACKAGE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
	Bundled,
	User,
	Development,
}

impl Origin {
	pub fn name(self) -> &'static str {
		match self {
			Origin::Bundled => "bundled",
			Origin::User => "user",
			Origin::Development => "development",
		}
	}
}

pub type Embedded = &'static [(&'static str, &'static str)];

#[derive(Clone, Debug)]
enum Files {
	Dir(PathBuf),
	Embedded(Embedded),
}

#[derive(Clone, Debug)]
pub struct Package {
	pub manifest: Manifest,
	pub origin: Origin,
	files: Files,
}

pub type Stamp = Vec<(String, Option<SystemTime>, u64)>;

impl Package {
	pub fn embedded(files: Embedded) -> Result<Package, String> {
		let manifest = files
			.iter()
			.find(|(name, _)| *name == MANIFEST_FILE)
			.map(|(_, text)| *text)
			.ok_or_else(|| "the bundled plugin has no plugin.toml".to_owned())?;
		Ok(Package {
			manifest: Manifest::parse(manifest).map_err(|error| error.to_string())?,
			origin: Origin::Bundled,
			files: Files::Embedded(files),
		})
	}

	pub fn open(dir: &Path, origin: Origin) -> Result<Package, String> {
		let manifest =
			read_capped(&dir.join(MANIFEST_FILE)).ok_or_else(|| "plugin.toml is missing".to_owned())?;
		Ok(Package {
			manifest: Manifest::parse(&manifest).map_err(|error| error.to_string())?,
			origin,
			files: Files::Dir(dir.to_path_buf()),
		})
	}

	pub fn id(&self) -> &str {
		&self.manifest.id
	}

	pub fn dir(&self) -> Option<&Path> {
		match &self.files {
			Files::Dir(dir) => Some(dir),
			Files::Embedded(_) => None,
		}
	}

	pub fn read(&self, name: &str) -> Option<String> {
		if !is_module_file(name) {
			return None;
		}
		match &self.files {
			Files::Dir(dir) => read_capped(&dir.join(name)),
			Files::Embedded(files) => files
				.iter()
				.find(|(file, _)| *file == name)
				.map(|(_, text)| (*text).to_owned()),
		}
	}

	pub fn logic_source(&self) -> Option<(String, String)> {
		let name = self.logic_file().to_owned();
		self.read(&name).map(|source| (name, source))
	}

	fn ui_file(&self) -> &str {
		self.manifest.ui.as_deref().unwrap_or(crate::manifest::UI_FILE)
	}

	fn logic_file(&self) -> &str {
		self.manifest
			.logic
			.as_deref()
			.unwrap_or(crate::manifest::LOGIC_FILE)
	}

	pub fn ui_entry(&self) -> Option<PathBuf> {
		let dir = self.dir()?;
		let path = dir.join(self.ui_file());
		path.is_file().then_some(path)
	}

	pub fn unpack_ui(&self, into: &Path) -> Option<PathBuf> {
		let Files::Embedded(files) = &self.files else {
			return self.ui_entry();
		};
		let entry = self.ui_file();
		if !files.iter().any(|(name, _)| *name == entry) {
			return None;
		}
		if let Err(error) = fs::create_dir_all(into) {
			tracing::warn!(plugin = %self.id(), %error, "the plugin's interface could not be unpacked");
			return None;
		}
		let unpacked = files
			.iter()
			.filter(|(name, _)| *name != MANIFEST_FILE && *name != self.logic_file() && is_plain_name(name))
			.all(|(name, text)| {
				let path = into.join(name);
				if fs::read_to_string(&path).ok().as_deref() == Some(*text) {
					return true;
				}
				fs::write(&path, text)
					.inspect_err(
						|error| tracing::warn!(plugin = %self.id(), %error, "the plugin's interface could not be unpacked"),
					)
					.is_ok()
			});
		unpacked.then(|| into.join(entry))
	}

	pub fn stamp(&self) -> Stamp {
		let Files::Dir(dir) = &self.files else {
			return Vec::new();
		};
		stamp_of(dir)
	}
}

pub fn stamp_of(dir: &Path) -> Stamp {
	let mut stamp: Stamp = fs::read_dir(dir)
		.into_iter()
		.flatten()
		.flatten()
		.take(FILES_PER_PACKAGE)
		.filter_map(|entry| {
			let metadata = entry.metadata().ok()?;
			metadata.is_file().then(|| {
				(
					entry.file_name().to_string_lossy().into_owned(),
					metadata.modified().ok(),
					metadata.len(),
				)
			})
		})
		.collect();
	stamp.sort();
	stamp
}

pub fn package_dirs(root: &Path) -> Vec<PathBuf> {
	let mut dirs: Vec<PathBuf> = fs::read_dir(root)
		.into_iter()
		.flatten()
		.flatten()
		.map(|entry| entry.path())
		.filter(|path| path.join(MANIFEST_FILE).is_file())
		.collect();
	dirs.sort();
	dirs
}

pub fn is_module_file(name: &str) -> bool {
	(name.ends_with(".luau") || name == MANIFEST_FILE) && is_plain_name(name)
}

fn is_plain_name(name: &str) -> bool {
	!name.is_empty()
		&& !name.starts_with('.')
		&& name
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
		&& !name.contains("..")
}

fn read_capped(path: &Path) -> Option<String> {
	let metadata = fs::metadata(path).ok()?;
	if !metadata.is_file() || metadata.len() > FILE_LIMIT {
		return None;
	}
	fs::read_to_string(path).ok()
}
