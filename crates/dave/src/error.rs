use std::fmt::Display;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct DaveError(pub String);

impl DaveError {
	pub fn new(reason: &str) -> Self {
		Self(reason.to_owned())
	}
}

pub type Result<T> = std::result::Result<T, DaveError>;

pub(crate) fn failure<E: Display>(context: &'static str) -> impl FnOnce(E) -> DaveError {
	move |error| DaveError(format!("{context}: {error}"))
}
