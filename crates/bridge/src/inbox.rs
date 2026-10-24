use std::future::poll_fn;
use std::sync::Arc;
use std::task::{Poll, Waker};

use parking_lot::Mutex;
use session::NotifyFn;
use state::Changes;

pub struct Inbox<T> {
	queue: Arc<Mutex<Queue<T>>>,
}

struct Queue<T> {
	items: Vec<T>,
	waker: Option<Waker>,
	closed: bool,
}

impl<T> Inbox<T> {
	pub fn new() -> Self {
		Self {
			queue: Arc::new(Mutex::new(Queue {
				items: Vec::new(),
				waker: None,
				closed: false,
			})),
		}
	}

	pub fn push(&self, item: T) {
		let waker = {
			let mut queue = self.queue.lock();
			if queue.closed {
				return;
			}
			queue.items.push(item);
			queue.waker.take()
		};
		if let Some(waker) = waker {
			waker.wake();
		}
	}

	pub fn close(&self) {
		let waker = {
			let mut queue = self.queue.lock();
			queue.closed = true;
			queue.waker.take()
		};
		if let Some(waker) = waker {
			waker.wake();
		}
	}

	pub async fn next(&self) -> Option<Vec<T>> {
		poll_fn(|context| {
			let mut queue = self.queue.lock();
			if !queue.items.is_empty() {
				return Poll::Ready(Some(std::mem::take(&mut queue.items)));
			}
			if queue.closed {
				return Poll::Ready(None);
			}
			queue.waker = Some(context.waker().clone());
			Poll::Pending
		})
		.await
	}
}

impl<T> Default for Inbox<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> Clone for Inbox<T> {
	fn clone(&self) -> Self {
		Self {
			queue: self.queue.clone(),
		}
	}
}

pub fn changes_notifier(inbox: Inbox<Changes>) -> NotifyFn {
	Arc::new(move |changes| inbox.push(changes))
}
