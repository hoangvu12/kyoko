use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::time::Duration;

use parking_lot::{Condvar, Mutex};

use crate::calls::{Call, Fault, PluginEvent, Reply};
use crate::context::{ActionLog, Context, Link, Settings};
use crate::limits::Limits;
use crate::package::Package;
use crate::permission::Permission;
use crate::requests::RequestSink;
use crate::storage::Storage;
use crate::vm::LogicVm;

const CALL_QUEUE: usize = 16;

pub(crate) type FaultSink = Arc<dyn Fn(Fault) + Send + Sync>;

pub(crate) struct Setup {
	pub package: Package,
	pub grants: BTreeSet<Permission>,
	pub settings: Settings,
	pub link: Link,
	pub limits: Limits,
	pub actions: ActionLog,
	pub storage: Option<PathBuf>,
	pub faults: FaultSink,
	pub requests: RequestSink,
}

struct Job {
	call: Call,
	deadline: Duration,
	reply: SyncSender<Result<Reply, Fault>>,
}

#[derive(Default)]
struct Queue {
	calls: VecDeque<Job>,
	events: VecDeque<PluginEvent>,
	stopped: bool,
}

#[derive(Default)]
struct Mailbox {
	queue: Mutex<Queue>,
	ready: Condvar,
}

enum Next {
	Call(Job),
	Event(PluginEvent),
	Stop,
}

impl Mailbox {
	fn next(&self) -> Next {
		let mut queue = self.queue.lock();
		loop {
			if queue.stopped {
				return Next::Stop;
			}
			if let Some(job) = queue.calls.pop_front() {
				return Next::Call(job);
			}
			if let Some(event) = queue.events.pop_front() {
				return Next::Event(event);
			}
			self.ready.wait(&mut queue);
		}
	}
}

pub(crate) struct Worker {
	mailbox: Arc<Mailbox>,
	defined: BTreeSet<String>,
	limits: Limits,
}

impl Worker {
	pub(crate) fn start(setup: Setup) -> Result<Worker, String> {
		let mailbox = Arc::new(Mailbox::default());
		let (loaded, load_result) = mpsc::sync_channel(1);
		let limits = setup.limits;
		let id = setup.package.id().to_owned();
		let inbox = mailbox.clone();
		std::thread::Builder::new()
			.name(format!("plugin-{id}"))
			.spawn(move || run(setup, &inbox, loaded))
			.map_err(|error| format!("the plugin thread did not start: {error}"))?;
		let wait = limits.load_deadline + limits.queue_grace;
		match load_result.recv_timeout(wait) {
			Ok(Ok(defined)) => Ok(Worker {
				mailbox,
				defined,
				limits,
			}),
			Ok(Err(error)) => Err(error),
			Err(_) => {
				mailbox.queue.lock().stopped = true;
				mailbox.ready.notify_all();
				Err("the plugin took too long to load".to_owned())
			}
		}
	}

	pub(crate) fn defines(&self, hook: &str) -> bool {
		self.defined.contains(hook)
	}

	pub(crate) fn call(&self, call: Call, deadline: Duration) -> Result<Reply, Fault> {
		let (reply, answer) = mpsc::sync_channel(1);
		{
			let mut queue = self.mailbox.queue.lock();
			if queue.stopped {
				return Err(Fault::Gone);
			}
			if queue.calls.len() >= CALL_QUEUE {
				return Err(Fault::Busy);
			}
			queue.calls.push_back(Job {
				call,
				deadline,
				reply,
			});
		}
		self.mailbox.ready.notify_one();
		match answer.recv_timeout(deadline.saturating_mul(2) + self.limits.queue_grace) {
			Ok(result) => result,
			Err(RecvTimeoutError::Timeout) => Err(Fault::Overrun),
			Err(RecvTimeoutError::Disconnected) => Err(Fault::Gone),
		}
	}

	pub(crate) fn deliver(&self, event: PluginEvent) -> bool {
		{
			let mut queue = self.mailbox.queue.lock();
			if queue.stopped || queue.events.len() >= self.limits.event_queue {
				return false;
			}
			queue.events.push_back(event);
		}
		self.mailbox.ready.notify_one();
		true
	}

	pub(crate) fn stop(&self) {
		let mut queue = self.mailbox.queue.lock();
		queue.stopped = true;
		queue.calls.clear();
		queue.events.clear();
		drop(queue);
		self.mailbox.ready.notify_all();
	}
}

impl Drop for Worker {
	fn drop(&mut self) {
		self.stop();
	}
}

fn run(setup: Setup, mailbox: &Mailbox, loaded: SyncSender<Result<BTreeSet<String>, String>>) {
	let Setup {
		package,
		grants,
		settings,
		link,
		limits,
		actions,
		storage,
		faults,
		requests,
	} = setup;
	let context = Rc::new(Context {
		id: package.id().to_owned(),
		version: package.manifest.version.clone(),
		grants,
		settings,
		link,
		limits,
		actions,
		storage: RefCell::new(Storage::open(storage, limits.storage_bytes, limits.storage_keys)),
		deadline: Cell::new(None),
		overran: Cell::new(false),
		gesture: Cell::new(false),
		gesture_actions: Cell::new(0),
		gesture_target: RefCell::new(None),
		requests,
	});
	let vm = match catch_unwind(AssertUnwindSafe(|| LogicVm::load(&package, context))) {
		Ok(Ok(vm)) => vm,
		Ok(Err(error)) => {
			let _ = loaded.send(Err(error));
			return;
		}
		Err(_) => {
			let _ = loaded.send(Err("the plugin crashed while loading".to_owned()));
			return;
		}
	};
	if loaded.send(Ok(vm.defined().clone())).is_err() {
		return;
	}
	loop {
		match mailbox.next() {
			Next::Stop => return,
			Next::Call(job) => {
				let result = catch_unwind(AssertUnwindSafe(|| vm.run(&job.call, job.deadline)))
					.unwrap_or_else(|_| Err(Fault::Error("the plugin crashed".to_owned())));
				let _ = job.reply.send(result);
			}
			Next::Event(event) => {
				let result = catch_unwind(AssertUnwindSafe(|| vm.deliver(&event, limits.event_deadline)))
					.unwrap_or_else(|_| Err(Fault::Error("the plugin crashed".to_owned())));
				if let Err(fault) = result {
					faults(fault);
				}
			}
		}
	}
}
