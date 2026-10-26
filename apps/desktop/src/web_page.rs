use bridge::{PageCookie, WebPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
	pub x: i32,
	pub y: i32,
	pub width: u32,
	pub height: u32,
}

pub enum PageEvent {
	Message { source: String, body: String },
	Navigated(String),
	Loaded(String),
}

pub use native::PageHost;

#[cfg(all(test, windows))]
mod agent;

#[cfg(windows)]
mod native {
	use std::num::NonZeroIsize;
	use std::time::{SystemTime, UNIX_EPOCH};

	use wry::dpi::{PhysicalPosition, PhysicalSize};
	use wry::raw_window_handle::{
		HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
	};
	use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

	use super::{Bounds, PageCookie, PageEvent, WebPage};

	struct Parent(NonZeroIsize);

	impl HasWindowHandle for Parent {
		fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
			let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
			Ok(unsafe { WindowHandle::borrow_raw(raw) })
		}
	}

	pub struct PageHost {
		webview: WebView,
		_context: WebContext,
	}

	impl PageHost {
		pub fn open(
			parent: usize,
			page: &WebPage,
			bounds: Bounds,
			events: impl Fn(PageEvent) + Clone + 'static,
		) -> Result<PageHost, String> {
			let parent = NonZeroIsize::new(parent as isize)
				.map(Parent)
				.ok_or_else(|| "the window has no handle".to_owned())?;
			let mut context = WebContext::new(Some(page.profile.clone()));
			let allowed = page.clone();
			let (messages, loads) = (events.clone(), events);
			let webview = WebViewBuilder::new_with_web_context(&mut context)
				.with_url(page.url.as_str())
				.with_initialization_script_for_main_only(page.script.as_str(), true)
				.with_incognito(true)
				.with_devtools(false)
				.with_bounds(rect(bounds))
				.with_ipc_handler(move |request| {
					let source = request.uri().to_string();
					messages(PageEvent::Message {
						source,
						body: request.into_body(),
					});
				})
				.with_navigation_handler(move |url| {
					let allowed = allowed.allows(&url);
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
				.with_on_page_load_handler(move |event, url| {
					loads(match event {
						PageLoadEvent::Started => PageEvent::Navigated(url),
						PageLoadEvent::Finished => PageEvent::Loaded(url),
					});
				})
				.build_as_child(&parent)
				.map_err(|error| error.to_string())?;
			let _ = webview.focus();
			Ok(PageHost {
				webview,
				_context: context,
			})
		}

		pub fn place(&self, bounds: Bounds) {
			if let Err(error) = self.webview.set_bounds(rect(bounds)) {
				tracing::debug!(%error, "the web login page could not be moved");
			}
		}

		pub fn cookies(&self, origin: &str) -> Vec<PageCookie> {
			match self.webview.cookies_for_url(&format!("{origin}/")) {
				Ok(cookies) => cookies.iter().map(page_cookie).collect(),
				Err(error) => {
					tracing::warn!(%error, "the web login page's cookies could not be read");
					Vec::new()
				}
			}
		}

		pub fn close(self) {
			if let Err(error) = self.webview.clear_all_browsing_data() {
				tracing::debug!(%error, "the web login page's browsing data was not cleared");
			}
			let _ = self.webview.focus_parent();
		}
	}

	fn rect(bounds: Bounds) -> Rect {
		Rect {
			position: PhysicalPosition::new(bounds.x, bounds.y).into(),
			size: PhysicalSize::new(bounds.width, bounds.height).into(),
		}
	}

	fn page_cookie(cookie: &wry::cookie::Cookie<'static>) -> PageCookie {
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_or(0, |since| since.as_secs() as i64);
		PageCookie {
			name: cookie.name().to_owned(),
			value: cookie.value().to_owned(),
			max_age: cookie
				.expires_datetime()
				.map(|expires| expires.unix_timestamp().saturating_sub(now).max(0) as u64),
			secure: cookie.secure().unwrap_or(false),
			http_only: cookie.http_only().unwrap_or(false),
		}
	}

	fn open_outside(url: &str) {
		if !url.starts_with("https://") {
			return;
		}
		if let Err(error) = platform::open::open_url(url) {
			tracing::warn!(%error, "a link from the web login page could not be opened");
		}
	}
}

#[cfg(not(windows))]
mod native {
	use super::{Bounds, PageCookie, PageEvent, WebPage};

	pub struct PageHost;

	impl PageHost {
		pub fn open(
			_: usize,
			_: &WebPage,
			_: Bounds,
			_: impl Fn(PageEvent) + Clone + 'static,
		) -> Result<PageHost, String> {
			Err("web pages are shown on Windows only".to_owned())
		}

		pub fn place(&self, _: Bounds) {}

		pub fn cookies(&self, _: &str) -> Vec<PageCookie> {
			Vec::new()
		}

		pub fn close(self) {}
	}
}
