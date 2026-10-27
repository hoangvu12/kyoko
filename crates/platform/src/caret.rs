use std::time::Duration;

use windows::Win32::UI::WindowsAndMessaging::GetCaretBlinkTime;

const NEVER: u32 = u32::MAX;
const WINDOWS_DEFAULT: Duration = Duration::from_millis(530);

pub fn blink_time() -> Option<Duration> {
	match unsafe { GetCaretBlinkTime() } {
		NEVER => None,
		0 => Some(WINDOWS_DEFAULT),
		millis => Some(Duration::from_millis(millis.into())),
	}
}
