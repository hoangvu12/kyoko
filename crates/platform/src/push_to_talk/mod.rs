mod matcher;

use std::cell::RefCell;
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
	NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
	CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HC_ACTION,
	HHOOK, HWND_MESSAGE, KBDLLHOOKSTRUCT, KillTimer, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT,
	PM_NOREMOVE, PeekMessageW, PostThreadMessageW, RegisterClassW, SetTimer, SetWindowsHookExW,
	UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_KEYDOWN, WM_KEYUP,
	WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_WTSSESSION_CHANGE, WM_XBUTTONDOWN, WM_XBUTTONUP,
	WNDCLASSW, WTS_SESSION_LOCK,
};
use windows::core::w;

pub use matcher::{
	Binding, Health, Input, InvalidBinding, LOST_AFTER_MS, MOUSE_4, MOUSE_5, Matcher, Modifiers,
	REFRESH_AFTER_MS, Verdict, verdict, virtual_key,
};

use crate::{PlatformError, Result};

const CHECK_EVERY_MS: u32 = 250;
const XBUTTON1: u32 = 1;
const XBUTTON2: u32 = 2;

pub struct TalkHook {
	thread_id: u32,
	thread: Option<JoinHandle<()>>,
}

impl TalkHook {
	pub fn start(binding: Binding, on_change: impl Fn(bool) + Send + 'static) -> Result<TalkHook> {
		let (started, ready) = mpsc::channel();
		let thread = std::thread::Builder::new()
			.name("kyoko-push-to-talk".to_owned())
			.spawn(move || run(binding, Box::new(on_change), &started))?;
		match ready.recv() {
			Ok(Ok(thread_id)) => Ok(TalkHook {
				thread_id,
				thread: Some(thread),
			}),
			Ok(Err(error)) => {
				let _ = thread.join();
				Err(error)
			}
			Err(_) => {
				let _ = thread.join();
				Err(PlatformError::Os("the push to talk thread stopped".to_owned()))
			}
		}
	}
}

impl Drop for TalkHook {
	fn drop(&mut self) {
		if let Err(error) = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) } {
			tracing::warn!(%error, "the push to talk thread was not told to stop");
			return;
		}
		if let Some(thread) = self.thread.take() {
			let _ = thread.join();
		}
	}
}

struct Hooked {
	matcher: Matcher,
	on_change: Box<dyn Fn(bool) + Send>,
	keyboard: HHOOK,
	mouse: HHOOK,
	last_event: u32,
	installed_at: u32,
	window: Option<HWND>,
}

impl Hooked {
	fn report(&self, change: Option<bool>) {
		if let Some(held) = change {
			(self.on_change)(held);
		}
	}

	fn check(&mut self) {
		let now = unsafe { GetTickCount() };
		let health = Health {
			active: self.matcher.active(),
			physically_held: self.matcher.binding().keys().into_iter().all(pressed),
			input_after_last_event_ms: last_input().map_or(0, |input| {
				let after = input.wrapping_sub(self.last_event);
				if after > u32::MAX / 2 { 0 } else { after }
			}),
			installed_ms: now.wrapping_sub(self.installed_at),
		};
		match verdict(health) {
			Verdict::Fine => {}
			Verdict::Stuck => {
				tracing::info!("push to talk missed a release, reinstalling the hooks");
				self.reinstall();
			}
			Verdict::Lost => {
				tracing::debug!("the push to talk hooks went quiet, reinstalling them");
				self.reinstall();
			}
			Verdict::Refresh => self.reinstall(),
		}
	}

	fn reinstall(&mut self) {
		match install() {
			Ok((keyboard, mouse)) => {
				unhook(self.keyboard, self.mouse);
				self.keyboard = keyboard;
				self.mouse = mouse;
				let now = unsafe { GetTickCount() };
				self.installed_at = now;
				self.last_event = now;
			}
			Err(error) => tracing::warn!(%error, "the push to talk hooks could not be reinstalled"),
		}
		let change = self.matcher.resync(pressed);
		self.report(change);
	}
}

impl Drop for Hooked {
	fn drop(&mut self) {
		if let Some(window) = self.window.take() {
			unsafe {
				let _ = WTSUnRegisterSessionNotification(window);
				let _ = DestroyWindow(window);
			}
		}
		unhook(self.keyboard, self.mouse);
		let change = self.matcher.release();
		self.report(change);
	}
}

thread_local! {
	static HOOKED: RefCell<Option<Hooked>> = const { RefCell::new(None) };
}

fn with_hooked(work: impl FnOnce(&mut Hooked)) {
	HOOKED.with(|hooked| {
		if let Ok(mut hooked) = hooked.try_borrow_mut()
			&& let Some(hooked) = hooked.as_mut()
		{
			work(hooked);
		}
	});
}

fn run(binding: Binding, on_change: Box<dyn Fn(bool) + Send>, started: &mpsc::Sender<Result<u32>>) {
	let mut message = MSG::default();
	unsafe {
		let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
	}
	let (keyboard, mouse) = match install() {
		Ok(hooks) => hooks,
		Err(error) => {
			let _ = started.send(Err(error));
			return;
		}
	};
	let now = unsafe { GetTickCount() };
	HOOKED.with(|hooked| {
		*hooked.borrow_mut() = Some(Hooked {
			matcher: Matcher::new(binding),
			on_change,
			keyboard,
			mouse,
			last_event: now,
			installed_at: now,
			window: session_window(),
		});
	});
	let timer = unsafe { SetTimer(None, 0, CHECK_EVERY_MS, None) };
	let _ = started.send(Ok(unsafe { GetCurrentThreadId() }));
	while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
		if message.message == WM_TIMER && message.hwnd.is_invalid() {
			with_hooked(Hooked::check);
		} else {
			unsafe { DispatchMessageW(&message) };
		}
	}
	unsafe {
		let _ = KillTimer(None, timer);
	}
	HOOKED.with(|hooked| hooked.borrow_mut().take());
}

fn install() -> Result<(HHOOK, HHOOK)> {
	let module = unsafe { GetModuleHandleW(None) }.map_err(os)?;
	let instance = Some(HINSTANCE(module.0));
	let keyboard =
		unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), instance, 0) }.map_err(os)?;
	match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), instance, 0) } {
		Ok(mouse) => Ok((keyboard, mouse)),
		Err(error) => {
			unsafe {
				let _ = UnhookWindowsHookEx(keyboard);
			}
			Err(os(error))
		}
	}
}

fn unhook(keyboard: HHOOK, mouse: HHOOK) {
	unsafe {
		let _ = UnhookWindowsHookEx(keyboard);
		let _ = UnhookWindowsHookEx(mouse);
	}
}

fn session_window() -> Option<HWND> {
	let class = w!("KyokoPushToTalk");
	let window = unsafe {
		let module = GetModuleHandleW(None).ok()?;
		let instance = HINSTANCE(module.0);
		RegisterClassW(&WNDCLASSW {
			lpfnWndProc: Some(session_proc),
			hInstance: instance,
			lpszClassName: class,
			..Default::default()
		});
		CreateWindowExW(
			WINDOW_EX_STYLE::default(),
			class,
			w!(""),
			WINDOW_STYLE::default(),
			0,
			0,
			0,
			0,
			Some(HWND_MESSAGE),
			None,
			Some(instance),
			None,
		)
	};
	let window = match window {
		Ok(window) => window,
		Err(error) => {
			tracing::warn!(%error, "push to talk will not hear the session lock");
			return None;
		}
	};
	if let Err(error) = unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) } {
		tracing::warn!(%error, "push to talk will not hear the session lock");
	}
	Some(window)
}

fn pressed(key: u16) -> bool {
	unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

fn last_input() -> Option<u32> {
	let mut info = LASTINPUTINFO {
		cbSize: size_of::<LASTINPUTINFO>() as u32,
		dwTime: 0,
	};
	unsafe { GetLastInputInfo(&mut info) }
		.as_bool()
		.then_some(info.dwTime)
}

fn os(error: windows::core::Error) -> PlatformError {
	PlatformError::Os(error.to_string())
}

fn observe(input: Option<Input>, time: u32) {
	with_hooked(|hooked| {
		hooked.last_event = time;
		if let Some(input) = input {
			let change = hooked.matcher.feed(input);
			hooked.report(change);
		}
	});
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	if code == HC_ACTION as i32 {
		let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
		let down = match wparam.0 as u32 {
			WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
			WM_KEYUP | WM_SYSKEYUP => Some(false),
			_ => None,
		};
		let input = down.and_then(|down| {
			Some(Input {
				key: u16::try_from(event.vkCode).ok()?,
				down,
				injected: event.flags.contains(LLKHF_INJECTED),
			})
		});
		observe(input, event.time);
	}
	unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
	if code == HC_ACTION as i32 {
		let event = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
		let down = match wparam.0 as u32 {
			WM_XBUTTONDOWN => Some(true),
			WM_XBUTTONUP => Some(false),
			_ => None,
		};
		let key = match event.mouseData >> 16 {
			XBUTTON1 => Some(MOUSE_4),
			XBUTTON2 => Some(MOUSE_5),
			_ => None,
		};
		let input = down.zip(key).map(|(down, key)| Input {
			key,
			down,
			injected: event.flags & LLMHF_INJECTED != 0,
		});
		observe(input, event.time);
	}
	unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn session_proc(
	window: HWND,
	message: u32,
	wparam: WPARAM,
	lparam: LPARAM,
) -> LRESULT {
	if message == WM_WTSSESSION_CHANGE && wparam.0 as u32 == WTS_SESSION_LOCK {
		with_hooked(|hooked| {
			let change = hooked.matcher.release();
			hooked.report(change);
		});
	}
	unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
