use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const NUDGE_EVERY: Duration = Duration::from_millis(50);
const NUDGE_FOR: Duration = Duration::from_secs(10);

static LOOP_THREAD: OnceLock<u32> = OnceLock::new();
static LOOP_ENDED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub struct Frame {
	#[cfg_attr(not(windows), allow(dead_code))]
	window: usize,
}

impl Frame {
	pub fn attach() -> Option<Frame> {
		let window = native::find_window()?;
		let _ = LOOP_THREAD.set(native::thread_of(window));
		Some(Frame { window })
	}

	pub fn is_maximized(self) -> bool {
		native::is_maximized(self.window)
	}
}

pub fn wake_until_loop_ends() {
	let Some(&thread) = LOOP_THREAD.get() else {
		return;
	};
	let spawned = std::thread::Builder::new()
		.name("kyoko-exit-waker".into())
		.spawn(move || {
			let started = Instant::now();
			while started.elapsed() < NUDGE_FOR && !LOOP_ENDED.load(Ordering::Acquire) {
				native::wake(thread);
				std::thread::sleep(NUDGE_EVERY);
			}
		});
	if let Err(error) = spawned {
		tracing::warn!(%error, "could not start the exit waker");
	}
}

pub fn loop_ended() {
	LOOP_ENDED.store(true, Ordering::Release);
}

#[cfg(windows)]
mod native {
	use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
	use windows::Win32::UI::WindowsAndMessaging::{
		EnumWindows, GetClassNameW, GetClientRect, GetWindowThreadProcessId, IsWindowVisible, IsZoomed,
		PostThreadMessageW, WM_NULL,
	};
	use windows::core::BOOL;

	const WINDOW_CLASS: &str = "Window Class";

	struct Search {
		process: u32,
		best: Option<(usize, i64)>,
	}

	pub fn find_window() -> Option<usize> {
		let mut search = Search {
			process: std::process::id(),
			best: None,
		};
		unsafe {
			let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
		}
		search.best.map(|(window, _)| window)
	}

	unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
		let search = unsafe { &mut *(data.0 as *mut Search) };
		let mut process = 0;
		unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
		if process != search.process || !unsafe { IsWindowVisible(window) }.as_bool() {
			return true.into();
		}
		let mut class = [0u16; 64];
		let length = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
		if String::from_utf16_lossy(&class[..length]) != WINDOW_CLASS {
			return true.into();
		}
		let mut rect = RECT::default();
		if unsafe { GetClientRect(window, &mut rect) }.is_err() {
			return true.into();
		}
		let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
		if search.best.is_none_or(|(_, best)| area > best) {
			search.best = Some((window.0 as usize, area));
		}
		true.into()
	}

	pub fn thread_of(window: usize) -> u32 {
		unsafe { GetWindowThreadProcessId(HWND(window as *mut _), None) }
	}

	pub fn is_maximized(window: usize) -> bool {
		unsafe { IsZoomed(HWND(window as *mut _)) }.as_bool()
	}

	pub fn wake(thread: u32) {
		unsafe {
			let _ = PostThreadMessageW(thread, WM_NULL, WPARAM(0), LPARAM(0));
		}
	}
}

#[cfg(not(windows))]
mod native {
	pub fn find_window() -> Option<usize> {
		None
	}

	pub fn thread_of(_: usize) -> u32 {
		0
	}

	pub fn is_maximized(_: usize) -> bool {
		false
	}

	pub fn wake(_: u32) {}
}
