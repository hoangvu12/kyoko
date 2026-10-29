use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{E_FAIL, HMODULE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
	D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
	D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device,
	ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{
	DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC,
	HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::WinRT::Direct3D11::{
	CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{
	EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetWindow, GetWindowLongW, GetWindowRect,
	GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
	WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, IInspectable, Interface, PCWSTR};

use super::capture::{ScreenKind, ScreenTarget, VideoSource};
use super::scale::{Bgra, bgra_to_nv12};
use super::{VideoError, VideoFrame, VideoQuality, fit, mf};

const MONITORINFOF_PRIMARY: u32 = 1;
const FRAME_POOL_BUFFERS: i32 = 2;
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const BGRA_BYTES: usize = 4;
const SHELL_CLASSES: [&str; 2] = ["Progman", "Shell_TrayWnd"];

fn failure(error: windows::core::Error) -> VideoError {
	mf::device_error(error)
}

pub struct ScreenSource {
	item: GraphicsCaptureItem,
	closed_token: i64,
	closed: Arc<AtomicBool>,
	window: Option<isize>,
	device: ID3D11Device,
	context: ID3D11DeviceContext,
	winrt_device: IDirect3DDevice,
	pool: Direct3D11CaptureFramePool,
	session: GraphicsCaptureSession,
	staging: Option<(ID3D11Texture2D, u32, u32)>,
	pool_size: (u32, u32),
	largest: (u32, u32),
	last: Option<VideoFrame>,
	interval: Duration,
	next: Instant,
	started: Instant,
}

unsafe impl Send for ScreenSource {}

impl ScreenSource {
	pub fn list() -> Vec<ScreenTarget> {
		let mut targets = monitors();
		targets.extend(windows_on_desktop());
		targets
	}

	pub fn open(target: &ScreenTarget, quality: VideoQuality, cursor: bool) -> Result<Self, VideoError> {
		mf::prepare()?;
		if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
			return Err(VideoError::Unsupported);
		}
		let interop =
			windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(failure)?;
		let item: GraphicsCaptureItem = unsafe {
			match target.kind {
				ScreenKind::Screen => interop.CreateForMonitor(HMONITOR(target.id as isize as *mut _)),
				ScreenKind::Window => interop.CreateForWindow(HWND(target.id as isize as *mut _)),
			}
		}
		.map_err(|_| VideoError::Ended)?;
		let closed = Arc::new(AtomicBool::new(false));
		let flag = closed.clone();
		let closed_token = item
			.Closed(&TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(
				move |_, _| {
					flag.store(true, Ordering::Release);
					Ok(())
				},
			))
			.map_err(failure)?;
		let (device, context) = create_device()?;
		let dxgi = device.cast::<IDXGIDevice>().map_err(failure)?;
		let winrt_device = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
			.and_then(|inspectable| inspectable.cast::<IDirect3DDevice>())
			.map_err(failure)?;
		let size = item.Size().map_err(failure)?;
		let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
			&winrt_device,
			DirectXPixelFormat::B8G8R8A8UIntNormalized,
			FRAME_POOL_BUFFERS,
			size,
		)
		.map_err(failure)?;
		let session = pool.CreateCaptureSession(&item).map_err(failure)?;
		let _ = session.SetIsCursorCaptureEnabled(cursor);
		let _ = session.SetIsBorderRequired(false);
		session.StartCapture().map_err(failure)?;
		let now = Instant::now();
		Ok(Self {
			item,
			closed_token,
			closed,
			window: (target.kind == ScreenKind::Window).then_some(target.id as isize),
			device,
			context,
			winrt_device,
			pool,
			session,
			staging: None,
			pool_size: (size.Width.max(0) as u32, size.Height.max(0) as u32),
			largest: (quality.width, quality.height),
			last: None,
			interval: Duration::from_secs(1) / quality.fps.max(1),
			next: now,
			started: now,
		})
	}

	fn ended(&self) -> bool {
		self.closed.load(Ordering::Acquire)
			|| self
				.window
				.is_some_and(|window| !unsafe { IsWindow(Some(HWND(window as *mut _))) }.as_bool())
	}

	fn latest_surface_frame(&mut self) -> Result<Option<VideoFrame>, VideoError> {
		let mut latest = None;
		while let Ok(frame) = self.pool.TryGetNextFrame() {
			latest = Some(frame);
		}
		let Some(frame) = latest else {
			return Ok(None);
		};
		let content = frame.ContentSize().map_err(failure)?;
		let (width, height) = (content.Width.max(0) as u32, content.Height.max(0) as u32);
		if width < 2 || height < 2 {
			return Ok(None);
		}
		let surface = frame.Surface().map_err(failure)?;
		let access = surface.cast::<IDirect3DDxgiInterfaceAccess>().map_err(failure)?;
		let texture: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(failure)?;
		let picture = self.read_texture(&texture, width, height)?;
		drop(surface);
		drop(frame);
		if (width, height) != self.pool_size {
			self.pool_size = (width, height);
			self.pool
				.Recreate(
					&self.winrt_device,
					DirectXPixelFormat::B8G8R8A8UIntNormalized,
					FRAME_POOL_BUFFERS,
					SizeInt32 {
						Width: width as i32,
						Height: height as i32,
					},
				)
				.map_err(failure)?;
		}
		Ok(Some(picture))
	}

	fn read_texture(
		&mut self,
		texture: &ID3D11Texture2D,
		width: u32,
		height: u32,
	) -> Result<VideoFrame, VideoError> {
		let mut description = D3D11_TEXTURE2D_DESC::default();
		unsafe { texture.GetDesc(&mut description) };
		let staging = self.staging_for(&description)?;
		let width = width.min(description.Width);
		let height = height.min(description.Height);
		let (out_width, out_height) = fit(width, height, self.largest.0, self.largest.1);
		unsafe {
			self.context.CopyResource(&staging, texture);
			let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
			self.context
				.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
				.map_err(failure)?;
			let pitch = mapped.RowPitch as usize;
			let length = pitch * (height as usize - 1) + width as usize * BGRA_BYTES;
			let pixels = std::slice::from_raw_parts(mapped.pData.cast::<u8>(), length);
			let converted = bgra_to_nv12(
				&Bgra {
					pixels,
					width,
					height,
					pitch,
				},
				out_width,
				out_height,
			);
			self.context.Unmap(&staging, 0);
			Ok(VideoFrame {
				width: out_width,
				height: out_height,
				nv12: converted?,
				timestamp: self.started.elapsed(),
			})
		}
	}

	fn staging_for(&mut self, description: &D3D11_TEXTURE2D_DESC) -> Result<ID3D11Texture2D, VideoError> {
		if let Some((texture, width, height)) = &self.staging
			&& *width == description.Width
			&& *height == description.Height
		{
			return Ok(texture.clone());
		}
		let mut staging_description = *description;
		staging_description.Usage = D3D11_USAGE_STAGING;
		staging_description.BindFlags = 0;
		staging_description.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
		staging_description.MiscFlags = 0;
		let mut texture = None;
		unsafe {
			self.device
				.CreateTexture2D(&staging_description, None, Some(&mut texture))
		}
		.map_err(failure)?;
		let texture = texture.ok_or(VideoError::NotFound)?;
		self.staging = Some((texture.clone(), description.Width, description.Height));
		Ok(texture)
	}
}

impl Drop for ScreenSource {
	fn drop(&mut self) {
		let _ = self.item.RemoveClosed(self.closed_token);
		let _ = self.session.Close();
		let _ = self.pool.Close();
	}
}

impl VideoSource for ScreenSource {
	fn next_frame(&mut self) -> Result<VideoFrame, VideoError> {
		mf::prepare()?;
		let now = Instant::now();
		if self.next > now {
			std::thread::sleep(self.next - now);
		}
		let waiting_since = Instant::now();
		let fresh = loop {
			if self.ended() {
				return Err(VideoError::Ended);
			}
			if let Some(frame) = self.latest_surface_frame()? {
				break Some(frame);
			}
			if self.last.is_some() {
				break None;
			}
			if waiting_since.elapsed() > FIRST_FRAME_TIMEOUT {
				return Err(VideoError::Timeout);
			}
			std::thread::sleep(POLL_INTERVAL);
		};
		self.next = (self.next + self.interval).max(Instant::now());
		match fresh {
			Some(frame) => {
				self.last = Some(frame.clone());
				Ok(frame)
			}
			None => {
				let mut repeated = self.last.clone().ok_or(VideoError::Timeout)?;
				repeated.timestamp = self.started.elapsed();
				Ok(repeated)
			}
		}
	}

	fn hidden(&self) -> bool {
		self.window
			.is_some_and(|window| unsafe { IsIconic(HWND(window as *mut _)) }.as_bool())
	}

	fn size(&self) -> (u32, u32) {
		self.last.as_ref().map_or_else(
			|| fit(self.pool_size.0, self.pool_size.1, self.largest.0, self.largest.1),
			|frame| (frame.width, frame.height),
		)
	}
}

fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext), VideoError> {
	let mut last = VideoError::NotFound;
	for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
		match try_device(driver) {
			Ok(pair) => return Ok(pair),
			Err(error) => last = failure(error),
		}
	}
	Err(last)
}

fn try_device(driver: D3D_DRIVER_TYPE) -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
	let mut device = None;
	let mut context = None;
	unsafe {
		D3D11CreateDevice(
			None,
			driver,
			HMODULE::default(),
			D3D11_CREATE_DEVICE_BGRA_SUPPORT,
			None,
			D3D11_SDK_VERSION,
			Some(&mut device),
			None,
			Some(&mut context),
		)?;
	}
	match (device, context) {
		(Some(device), Some(context)) => Ok((device, context)),
		_ => Err(windows::core::Error::from_hresult(E_FAIL)),
	}
}

struct Monitor {
	handle: u64,
	width: u32,
	height: u32,
	primary: bool,
}

fn monitors() -> Vec<ScreenTarget> {
	unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, found: LPARAM) -> BOOL {
		let found = unsafe { &mut *(found.0 as *mut Vec<Monitor>) };
		let mut info = MONITORINFOEXW::default();
		info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
		if unsafe { GetMonitorInfoW(monitor, (&raw mut info).cast::<MONITORINFO>()) }.as_bool() {
			let (width, height) = physical_size(&info);
			found.push(Monitor {
				handle: monitor.0 as usize as u64,
				width,
				height,
				primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
			});
		}
		BOOL::from(true)
	}
	let mut found: Vec<Monitor> = Vec::new();
	unsafe {
		let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM((&raw mut found) as isize));
	}
	found.sort_by_key(|monitor| !monitor.primary);
	found
		.into_iter()
		.enumerate()
		.map(|(index, monitor)| ScreenTarget {
			kind: ScreenKind::Screen,
			id: monitor.handle,
			name: format!("Screen {}", index + 1),
			width: monitor.width,
			height: monitor.height,
			process: 0,
		})
		.collect()
}

fn physical_size(info: &MONITORINFOEXW) -> (u32, u32) {
	let mut mode = DEVMODEW {
		dmSize: std::mem::size_of::<DEVMODEW>() as u16,
		..Default::default()
	};
	if unsafe { EnumDisplaySettingsW(PCWSTR(info.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }
		.as_bool()
	{
		return (mode.dmPelsWidth, mode.dmPelsHeight);
	}
	let rect = info.monitorInfo.rcMonitor;
	((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32)
}

fn windows_on_desktop() -> Vec<ScreenTarget> {
	unsafe extern "system" fn collect(window: HWND, found: LPARAM) -> BOOL {
		let targets = unsafe { &mut *(found.0 as *mut Vec<ScreenTarget>) };
		if let Some(target) = describe_window(window) {
			targets.push(target);
		}
		BOOL::from(true)
	}
	let mut targets: Vec<ScreenTarget> = Vec::new();
	unsafe {
		let _ = EnumWindows(Some(collect), LPARAM((&raw mut targets) as isize));
	}
	targets
}

fn shell_window(window: HWND) -> bool {
	let mut class = [0u16; 64];
	let length = unsafe { GetClassNameW(window, &mut class) };
	let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
	SHELL_CLASSES.contains(&class.as_str())
}

fn describe_window(window: HWND) -> Option<ScreenTarget> {
	unsafe {
		if !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() || shell_window(window) {
			return None;
		}
		if GetWindowLongW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0
			|| GetWindow(window, GW_OWNER).is_ok_and(|owner| !owner.is_invalid())
		{
			return None;
		}
		let mut cloaked = 0u32;
		let _ = DwmGetWindowAttribute(
			window,
			DWMWA_CLOAKED,
			(&raw mut cloaked).cast(),
			std::mem::size_of::<u32>() as u32,
		);
		if cloaked != 0 {
			return None;
		}
		let length = GetWindowTextLengthW(window);
		if length == 0 {
			return None;
		}
		let mut title = vec![0u16; length as usize + 1];
		let copied = GetWindowTextW(window, &mut title);
		let mut rect = RECT::default();
		GetWindowRect(window, &mut rect).ok()?;
		let (width, height) = ((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32);
		let mut process = 0u32;
		GetWindowThreadProcessId(window, Some(&mut process));
		(copied > 0 && width > 1 && height > 1).then(|| ScreenTarget {
			kind: ScreenKind::Window,
			id: window.0 as usize as u64,
			name: String::from_utf16_lossy(&title[..copied as usize]),
			width,
			height,
			process,
		})
	}
}
