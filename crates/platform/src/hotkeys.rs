use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;

use crate::{PlatformError, Result};

static CREATED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyEvent {
	pub id: String,
	pub pressed: bool,
}

pub struct Hotkeys {
	manager: GlobalHotKeyManager,
	registered: HashMap<String, HotKey>,
	ids: Arc<Mutex<HashMap<u32, String>>>,
}

impl Hotkeys {
	pub fn new(on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static) -> Result<Hotkeys> {
		if CREATED.swap(true, Ordering::SeqCst) {
			return Err(PlatformError::AlreadyCreated("hotkeys"));
		}
		let manager = match GlobalHotKeyManager::new() {
			Ok(manager) => manager,
			Err(error) => {
				CREATED.store(false, Ordering::SeqCst);
				return Err(error.into());
			}
		};
		let ids: Arc<Mutex<HashMap<u32, String>>> = Arc::default();
		let handler_ids = Arc::clone(&ids);
		GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
			let id = handler_ids.lock().get(&event.id()).cloned();
			if let Some(id) = id {
				on_event(HotkeyEvent {
					id,
					pressed: event.state() == HotKeyState::Pressed,
				});
			}
		}));
		Ok(Hotkeys {
			manager,
			registered: HashMap::new(),
			ids,
		})
	}

	pub fn register(&mut self, binding: &str, id: &str) -> Result<()> {
		let hotkey: HotKey = binding.parse()?;
		self.unregister(id).ok();
		self.manager.register(hotkey)?;
		self.ids.lock().insert(hotkey.id(), id.to_owned());
		self.registered.insert(id.to_owned(), hotkey);
		Ok(())
	}

	pub fn unregister(&mut self, id: &str) -> Result<()> {
		let hotkey = self
			.registered
			.remove(id)
			.ok_or_else(|| PlatformError::UnknownHotkey(id.to_owned()))?;
		self.ids.lock().remove(&hotkey.id());
		self.manager.unregister(hotkey)?;
		Ok(())
	}
}

impl Drop for Hotkeys {
	fn drop(&mut self) {
		let hotkeys: Vec<HotKey> = self.registered.drain().map(|(_, hotkey)| hotkey).collect();
		let _ = self.manager.unregister_all(&hotkeys);
		GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
		CREATED.store(false, Ordering::SeqCst);
	}
}
