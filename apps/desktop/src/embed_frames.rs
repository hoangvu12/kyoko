pub use native::WebFrames;

#[cfg(windows)]
mod native {
	use std::borrow::Cow;
	use std::collections::HashMap;
	use std::num::NonZeroIsize;
	use std::path::PathBuf;
	use std::sync::OnceLock;

	use bridge::{FRAME_SCHEME, FRAME_URL, FrameHost, FrameRect};
	use webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString;
	use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
	use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
	use windows::Win32::System::LibraryLoader::GetModuleHandleW;
	use windows::Win32::UI::WindowsAndMessaging::{
		CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SW_HIDE, SWP_NOACTIVATE,
		SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WNDCLASSW, WS_CHILD,
		WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
	};
	use windows::core::{PCWSTR, PWSTR, w};
	use wry::dpi::{PhysicalPosition, PhysicalSize};
	use wry::http::header::CONTENT_TYPE;
	use wry::http::{Request, Response};
	use wry::raw_window_handle::{
		HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
	};
	use wry::{
		NewWindowResponse, PermissionResponse, Rect, WebContext, WebView, WebViewBuilder,
		WebViewBuilderExtWindows, WebViewId,
	};

	const CLASS: PCWSTR = w!("KyokoEmbedFrame");
	const BLACK: (u8, u8, u8, u8) = (0, 0, 0, 255);

	struct Parent(HWND);

	impl HasWindowHandle for Parent {
		fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
			let handle = NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?;
			Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(handle))) })
		}
	}

	struct Frame {
		container: HWND,
		webview: WebView,
		shown: bool,
	}

	impl Frame {
		fn destroy(self) {
			let Self {
				container, webview, ..
			} = self;
			if let Err(error) = webview.clear_all_browsing_data() {
				tracing::debug!(%error, "an embed player's browsing data was not cleared");
			}
			let _ = webview.focus_parent();
			drop(webview);
			let _ = unsafe { DestroyWindow(container) };
		}
	}

	pub struct WebFrames {
		parent: HWND,
		profile: PathBuf,
		context: Option<WebContext>,
		frames: HashMap<u64, Frame>,
		available: bool,
	}

	impl WebFrames {
		pub fn new(parent: usize, profile: PathBuf) -> Self {
			let available = match webview_version() {
				Ok(version) => {
					tracing::debug!(%version, "WebView2 is available for embed players");
					true
				}
				Err(error) => {
					tracing::info!(%error, "WebView2 is missing; embed players open in the browser");
					false
				}
			};
			Self {
				parent: HWND(parent as *mut _),
				profile,
				context: None,
				frames: HashMap::new(),
				available,
			}
		}

		fn container(&self) -> Result<HWND, String> {
			if !registered() {
				return Err("the embed frame class was not registered".to_owned());
			}
			let instance = unsafe { GetModuleHandleW(None) }.ok();
			unsafe {
				CreateWindowExW(
					WINDOW_EX_STYLE::default(),
					CLASS,
					PCWSTR::null(),
					WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
					0,
					0,
					1,
					1,
					Some(self.parent),
					None,
					instance.map(Into::into),
					None,
				)
			}
			.map_err(|error| error.to_string())
		}
	}

	fn webview_version() -> Result<String, windows::core::Error> {
		let mut version = PWSTR::null();
		unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }?;
		Ok(webview2_com::take_pwstr(version))
	}

	fn registered() -> bool {
		static REGISTERED: OnceLock<bool> = OnceLock::new();
		*REGISTERED.get_or_init(|| {
			let instance = unsafe { GetModuleHandleW(None) }.ok();
			let class = WNDCLASSW {
				lpfnWndProc: Some(procedure),
				hInstance: instance.map(Into::into).unwrap_or_default(),
				lpszClassName: CLASS,
				hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
				..WNDCLASSW::default()
			};
			unsafe { RegisterClassW(&class) != 0 }
		})
	}

	unsafe extern "system" fn procedure(
		window: HWND,
		message: u32,
		wparam: WPARAM,
		lparam: LPARAM,
	) -> LRESULT {
		unsafe { DefWindowProcW(window, message, wparam, lparam) }
	}

	fn document_response(document: &str) -> Response<Cow<'static, [u8]>> {
		Response::builder()
			.header(CONTENT_TYPE, "text/html; charset=utf-8")
			.header("Content-Security-Policy", bridge::frame_policy())
			.header("Referrer-Policy", "strict-origin")
			.header("X-Content-Type-Options", "nosniff")
			.body(Cow::Owned(document.as_bytes().to_vec()))
			.unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
	}

	fn inner(rect: FrameRect, clip: FrameRect) -> Rect {
		Rect {
			position: PhysicalPosition::new(rect.x - clip.x, rect.y - clip.y).into(),
			size: PhysicalSize::new(rect.width.max(1) as u32, rect.height.max(1) as u32).into(),
		}
	}

	fn open_outside(url: &str) {
		if !url.starts_with("https://") {
			return;
		}
		if let Err(error) = platform::open::open_url(url) {
			tracing::warn!(%error, "a link from an embed player could not be opened");
		}
	}

	impl FrameHost for WebFrames {
		fn available(&self) -> bool {
			self.available
		}

		fn open(&mut self, id: u64, document: String) -> Result<(), String> {
			let container = self.container()?;
			let context = self
				.context
				.get_or_insert_with(|| WebContext::new(Some(self.profile.clone())));
			let built = WebViewBuilder::new_with_web_context(context)
				.with_custom_protocol(
					FRAME_SCHEME.to_owned(),
					move |_: WebViewId, _: Request<Vec<u8>>| document_response(&document),
				)
				.with_https_scheme(true)
				.with_url(FRAME_URL)
				.with_bounds(inner(FrameRect::default(), FrameRect::default()))
				.with_background_color(BLACK)
				.with_incognito(true)
				.with_autoplay(true)
				.with_devtools(false)
				.with_clipboard(false)
				.with_hotkeys_zoom(false)
				.with_focused(false)
				.with_browser_accelerator_keys(false)
				.with_default_context_menus(false)
				.with_navigation_handler(|url| {
					let allowed = bridge::frame_allows(&url);
					if !allowed {
						open_outside(&url);
					}
					allowed
				})
				.with_new_window_req_handler(|url, _| {
					open_outside(&url);
					NewWindowResponse::Deny
				})
				.with_download_started_handler(|_, _| false)
				.with_permission_handler(|_| PermissionResponse::Deny)
				.build_as_child(&Parent(container));
			match built {
				Ok(webview) => {
					self.frames.insert(
						id,
						Frame {
							container,
							webview,
							shown: false,
						},
					);
					Ok(())
				}
				Err(error) => {
					let _ = unsafe { DestroyWindow(container) };
					Err(error.to_string())
				}
			}
		}

		fn place(&mut self, id: u64, rect: FrameRect, clip: FrameRect) {
			let Some(frame) = self.frames.get_mut(&id) else {
				return;
			};
			let moved = unsafe {
				SetWindowPos(
					frame.container,
					None,
					clip.x,
					clip.y,
					clip.width.max(1),
					clip.height.max(1),
					SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
				)
			};
			if let Err(error) = moved {
				tracing::debug!(%error, "an embed player could not be moved");
			}
			if let Err(error) = frame.webview.set_bounds(inner(rect, clip)) {
				tracing::debug!(%error, "an embed player could not be sized");
			}
			frame.shown = true;
		}

		fn hide(&mut self, id: u64) {
			if let Some(frame) = self.frames.get_mut(&id).filter(|frame| frame.shown) {
				let _ = unsafe { ShowWindow(frame.container, SW_HIDE) };
				frame.shown = false;
			}
		}

		fn close(&mut self, id: u64) {
			if let Some(frame) = self.frames.remove(&id) {
				frame.destroy();
			}
		}
	}

	impl Drop for WebFrames {
		fn drop(&mut self) {
			for (_, frame) in self.frames.drain() {
				frame.destroy();
			}
		}
	}
}

#[cfg(not(windows))]
mod native {
	use std::path::PathBuf;

	use bridge::{FrameHost, FrameRect};

	pub struct WebFrames;

	impl WebFrames {
		pub fn new(_: usize, _: PathBuf) -> Self {
			Self
		}
	}

	impl FrameHost for WebFrames {
		fn available(&self) -> bool {
			false
		}

		fn open(&mut self, _: u64, _: String) -> Result<(), String> {
			Err("embed players run on Windows only".to_owned())
		}

		fn place(&mut self, _: u64, _: FrameRect, _: FrameRect) {}

		fn hide(&mut self, _: u64) {}

		fn close(&mut self, _: u64) {}
	}
}
