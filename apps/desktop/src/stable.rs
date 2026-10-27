use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use zgx_script::Value;

#[derive(Default)]
pub(crate) struct StableRows {
	last: RefCell<HashMap<Rc<str>, Value>>,
}

impl StableRows {
	pub(crate) fn rows(&self, fresh: impl IntoIterator<Item = Value>) -> Rc<[Value]> {
		let mut last = self.last.borrow_mut();
		let mut next = HashMap::with_capacity(last.len());
		let rows = fresh
			.into_iter()
			.map(|row| {
				let Some(key) = key_of(&row) else {
					return row;
				};
				let kept = match last.remove(&key) {
					Some(previous) if previous == row => previous,
					_ => row,
				};
				next.insert(key, kept.clone());
				kept
			})
			.collect();
		*last = next;
		rows
	}

	pub(crate) fn forget(&self) {
		self.last.borrow_mut().clear();
	}
}

fn key_of(row: &Value) -> Option<Rc<str>> {
	let Value::Map(fields) = row else {
		return None;
	};
	fields.iter().find_map(|(name, value)| match value {
		Value::Str(key) if &**name == "key" => Some(key.clone()),
		_ => None,
	})
}
