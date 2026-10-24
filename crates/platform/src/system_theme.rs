use std::ffi::c_void;

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTheme {
	Dark,
	Light,
}

pub fn current() -> SystemTheme {
	let mut uses_light = 0u32;
	let mut size = size_of::<u32>() as u32;
	let status = unsafe {
		RegGetValueW(
			HKEY_CURRENT_USER,
			w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
			w!("AppsUseLightTheme"),
			RRF_RT_REG_DWORD,
			None,
			Some(&mut uses_light as *mut u32 as *mut c_void),
			Some(&mut size),
		)
	};
	if status.is_ok() && uses_light != 0 {
		SystemTheme::Light
	} else {
		SystemTheme::Dark
	}
}
