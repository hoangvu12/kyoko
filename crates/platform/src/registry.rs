use std::ffi::c_void;

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
	HKEY, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyW, RegDeleteTreeW, RegGetValueW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

use crate::{PlatformError, Result};

pub(crate) struct Key(HKEY);

fn check(status: WIN32_ERROR, what: &str) -> Result<()> {
	if status.is_ok() {
		Ok(())
	} else {
		Err(PlatformError::Os(format!("{what} failed with code {}", status.0)))
	}
}

impl Key {
	pub(crate) fn create(root: HKEY, path: &str) -> Result<Key> {
		let mut key = HKEY::default();
		let status = unsafe { RegCreateKeyW(root, &HSTRING::from(path), &mut key) };
		check(status, "RegCreateKeyW")?;
		Ok(Key(key))
	}

	pub(crate) fn set_string(&self, name: &str, value: &str) -> Result<()> {
		let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
		let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_le_bytes()).collect();
		let name = HSTRING::from(name);
		let name = if name.is_empty() {
			PCWSTR::null()
		} else {
			PCWSTR(name.as_ptr())
		};
		let status = unsafe { RegSetValueExW(self.0, name, None, REG_SZ, Some(&bytes)) };
		check(status, "RegSetValueExW")
	}
}

impl Drop for Key {
	fn drop(&mut self) {
		let _ = unsafe { RegCloseKey(self.0) };
	}
}

pub(crate) fn read_string(root: HKEY, path: &str, name: &str) -> Option<String> {
	let path = HSTRING::from(path);
	let name = HSTRING::from(name);
	let name = if name.is_empty() {
		PCWSTR::null()
	} else {
		PCWSTR(name.as_ptr())
	};
	let mut size = 0u32;
	let status = unsafe { RegGetValueW(root, &path, name, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
	if status.is_err() || size == 0 {
		return None;
	}
	let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
	let status = unsafe {
		RegGetValueW(
			root,
			&path,
			name,
			RRF_RT_REG_SZ,
			None,
			Some(buffer.as_mut_ptr() as *mut c_void),
			Some(&mut size),
		)
	};
	if status.is_err() {
		return None;
	}
	let length = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
	Some(String::from_utf16_lossy(&buffer[..length]))
}

pub(crate) fn delete_tree(root: HKEY, path: &str) -> Result<()> {
	let status = unsafe { RegDeleteTreeW(root, &HSTRING::from(path)) };
	if status == ERROR_FILE_NOT_FOUND {
		return Ok(());
	}
	check(status, "RegDeleteTreeW")
}
