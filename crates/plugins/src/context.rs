use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::{Mutex, RwLock};
use serde_json::{Map, Value as Json};
use session::Commander;
use state::{Expression, SharedState};

use crate::limits::Limits;
use crate::permission::Permission;
use crate::requests::RequestSink;
use crate::storage::Storage;

#[derive(Clone)]
pub struct Attachment {
	pub state: SharedState,
	pub commander: Commander,
}

pub(crate) type Link = Arc<RwLock<Option<Attachment>>>;
pub(crate) type Settings = Arc<RwLock<Map<String, Json>>>;
pub(crate) type ActionLog = Arc<Mutex<VecDeque<Instant>>>;

pub(crate) struct Context {
	pub id: String,
	pub version: String,
	pub grants: BTreeSet<Permission>,
	pub settings: Settings,
	pub link: Link,
	pub limits: Limits,
	pub actions: ActionLog,
	pub storage: RefCell<Storage>,
	pub deadline: Cell<Option<Instant>>,
	pub overran: Cell<bool>,
	pub gesture: Cell<bool>,
	pub gesture_actions: Cell<usize>,
	pub gesture_target: RefCell<Option<Expression>>,
	pub requests: RequestSink,
}

impl Context {
	pub(crate) fn allows(&self, permission: &Permission) -> bool {
		self.grants.contains(permission)
	}

	pub(crate) fn attachment(&self) -> Option<Attachment> {
		self.link.read().clone()
	}
}
