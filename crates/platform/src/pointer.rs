use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::{
	GA_ROOT, GetAncestor, GetCursorPos, GetWindowThreadProcessId, WindowFromPoint,
};

pub fn over_own_window() -> Option<(f32, f32)> {
	let mut point = POINT::default();
	unsafe { GetCursorPos(&mut point) }.ok()?;
	let window = unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) };
	if window.is_invalid() {
		return None;
	}
	let mut process = 0u32;
	unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
	if process != std::process::id() {
		return None;
	}
	if !unsafe { ScreenToClient(window, &mut point) }.as_bool() {
		return None;
	}
	Some((point.x as f32, point.y as f32))
}
