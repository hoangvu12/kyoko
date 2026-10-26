use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};

pub fn shift_held() -> bool {
	let state = unsafe { GetAsyncKeyState(i32::from(VK_SHIFT.0)) };
	state < 0
}
