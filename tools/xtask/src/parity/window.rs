use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use image::RgbaImage;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
	BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC,
	DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::HiDpi::{
	DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
	EnumWindows, GetClientRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW,
	SW_RESTORE, SetForegroundWindow, ShowWindow, WM_CLOSE,
};
use windows::core::BOOL;

use super::App;
use super::compose::{HEIGHT, WIDTH};

const CLIENT_AND_FULL_CONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(3);
const FIND_LIMIT: Duration = Duration::from_secs(10);
const FOREGROUND_SETTLE: Duration = Duration::from_millis(400);
const STANDARD_DPI: u32 = 96;

pub fn init_dpi_awareness() {
	let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

pub fn capture(pid: u32, app: App) -> Result<RgbaImage> {
	let window = find(pid, app)?;
	if unsafe { IsIconic(window) }.as_bool() {
		let _ = unsafe { ShowWindow(window, SW_RESTORE) };
		std::thread::sleep(FOREGROUND_SETTLE);
	}
	let dpi = unsafe { GetDpiForWindow(window) };
	if dpi != STANDARD_DPI {
		eprintln!(
			"{app}: the window is at {dpi} dpi, not {STANDARD_DPI}; captures assume 100% display scale"
		);
	}
	let (width, height) = client_size(window);
	if (width, height) != (WIDTH as i32, HEIGHT as i32) {
		eprintln!("{app}: the client area is {width}x{height}, not {WIDTH}x{HEIGHT}");
	}
	let printed = print(window, width, height)?;
	if !is_blank(&printed) {
		return Ok(printed);
	}
	let _ = unsafe { SetForegroundWindow(window) };
	std::thread::sleep(FOREGROUND_SETTLE);
	let copied = copy_from_screen(window, width, height)?;
	if is_blank(&copied) {
		bail!("{app}: the window captured blank");
	}
	Ok(copied)
}

pub fn request_close(pid: u32) {
	for window in windows_of(pid) {
		let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
	}
}

fn find(pid: u32, app: App) -> Result<HWND> {
	let deadline = Instant::now() + FIND_LIMIT;
	loop {
		let largest = windows_of(pid)
			.into_iter()
			.filter(|window| unsafe { IsWindowVisible(*window) }.as_bool())
			.map(|window| (window, client_size(window)))
			.filter(|(_, (width, height))| *width > 0 && *height > 0)
			.max_by_key(|(_, (width, height))| width * height);
		if let Some((window, _)) = largest {
			return Ok(window);
		}
		if Instant::now() >= deadline {
			bail!("{app}: no visible window for process {pid}");
		}
		std::thread::sleep(Duration::from_millis(100));
	}
}

struct Search {
	pid: u32,
	found: Vec<HWND>,
}

unsafe extern "system" fn collect(window: HWND, search: LPARAM) -> BOOL {
	let search = unsafe { &mut *(search.0 as *mut Search) };
	let mut owner = 0u32;
	unsafe { GetWindowThreadProcessId(window, Some(&mut owner)) };
	if owner == search.pid {
		search.found.push(window);
	}
	BOOL(1)
}

fn windows_of(pid: u32) -> Vec<HWND> {
	let mut search = Search {
		pid,
		found: Vec::new(),
	};
	let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut search as *mut Search as isize)) };
	search.found
}

fn client_size(window: HWND) -> (i32, i32) {
	let mut rect = RECT::default();
	if unsafe { GetClientRect(window, &mut rect) }.is_err() {
		return (0, 0);
	}
	(rect.right - rect.left, rect.bottom - rect.top)
}

struct Canvas {
	source: HDC,
	memory: HDC,
	bitmap: HBITMAP,
	window: Option<HWND>,
}

impl Canvas {
	fn new(window: Option<HWND>, width: i32, height: i32) -> Result<Canvas> {
		let source = unsafe { GetDC(window) };
		if source.is_invalid() {
			bail!("no device context for the window");
		}
		let memory = unsafe { CreateCompatibleDC(Some(source)) };
		let bitmap = unsafe { CreateCompatibleBitmap(source, width, height) };
		unsafe { SelectObject(memory, bitmap.into()) };
		Ok(Canvas {
			source,
			memory,
			bitmap,
			window,
		})
	}

	fn pixels(&self, width: i32, height: i32) -> Result<RgbaImage> {
		let mut info = BITMAPINFO {
			bmiHeader: BITMAPINFOHEADER {
				biSize: size_of::<BITMAPINFOHEADER>() as u32,
				biWidth: width,
				biHeight: -height,
				biPlanes: 1,
				biBitCount: 32,
				biCompression: BI_RGB.0,
				..Default::default()
			},
			..Default::default()
		};
		let mut buffer = vec![0u8; width as usize * height as usize * 4];
		let lines = unsafe {
			GetDIBits(
				self.memory,
				self.bitmap,
				0,
				height as u32,
				Some(buffer.as_mut_ptr().cast()),
				&mut info,
				DIB_RGB_COLORS,
			)
		};
		if lines != height {
			bail!("read {lines} of {height} lines from the window bitmap");
		}
		for pixel in buffer.as_chunks_mut::<4>().0 {
			pixel.swap(0, 2);
			pixel[3] = 255;
		}
		RgbaImage::from_raw(width as u32, height as u32, buffer)
			.ok_or_else(|| anyhow::anyhow!("the window bitmap has the wrong size"))
	}
}

impl Drop for Canvas {
	fn drop(&mut self) {
		unsafe {
			let _ = DeleteObject(self.bitmap.into());
			let _ = DeleteDC(self.memory);
			ReleaseDC(self.window, self.source);
		}
	}
}

fn print(window: HWND, width: i32, height: i32) -> Result<RgbaImage> {
	let canvas = Canvas::new(Some(window), width, height)?;
	if !unsafe { PrintWindow(window, canvas.memory, CLIENT_AND_FULL_CONTENT) }.as_bool() {
		bail!("PrintWindow failed");
	}
	canvas.pixels(width, height)
}

fn copy_from_screen(window: HWND, width: i32, height: i32) -> Result<RgbaImage> {
	let mut origin = POINT::default();
	let _ = unsafe { ClientToScreen(window, &mut origin) };
	let canvas = Canvas::new(None, width, height)?;
	unsafe {
		BitBlt(
			canvas.memory,
			0,
			0,
			width,
			height,
			Some(canvas.source),
			origin.x,
			origin.y,
			SRCCOPY,
		)?
	};
	canvas.pixels(width, height)
}

fn is_blank(image: &RgbaImage) -> bool {
	let first = image.get_pixel(0, 0);
	image.pixels().all(|pixel| pixel == first)
}
