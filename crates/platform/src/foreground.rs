use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub fn belongs_to_this_process() -> bool {
	let window = unsafe { GetForegroundWindow() };
	if window.is_invalid() {
		return false;
	}
	let mut process = 0u32;
	unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
	process == std::process::id()
}
