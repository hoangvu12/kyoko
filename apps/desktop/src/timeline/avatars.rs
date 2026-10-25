use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use media::{Gallery, ImageName};
use protocol::{Snowflake, User};
use zgx::image::ImageData;
use zgx::reactive::{Runtime, Signal};

use super::rows::AVATAR;

type Picture = Signal<Option<Arc<ImageData>>>;

pub struct Avatars {
	runtime: Runtime,
	gallery: Gallery,
	cdn: String,
	scale: Cell<f64>,
	names: RefCell<HashMap<(Snowflake, String), ImageName>>,
	pictures: RefCell<HashMap<ImageName, Picture>>,
}

impl Avatars {
	pub fn new(runtime: Runtime, gallery: Gallery, cdn: String) -> Self {
		Self {
			runtime,
			gallery,
			cdn,
			scale: Cell::new(1.),
			names: RefCell::default(),
			pictures: RefCell::default(),
		}
	}

	pub fn set_scale(&self, scale: f64) {
		if self.scale.replace(scale) != scale {
			self.names.borrow_mut().clear();
		}
	}

	pub fn name_for(&self, user: &User) -> Option<ImageName> {
		let key = (user.id, user.avatar.clone().unwrap_or_default());
		if let Some(name) = self.names.borrow().get(&key) {
			return Some(name.clone());
		}
		let request = bridge::avatar_request(&self.cdn, user, AVATAR, self.scale.get(), false)?;
		let name = self.gallery.request(request);
		self.names.borrow_mut().insert(key, name.clone());
		Some(name)
	}

	pub fn picture(&self, name: &ImageName) -> Picture {
		self.pictures
			.borrow_mut()
			.entry(name.clone())
			.or_insert_with(|| self.runtime.signal(None))
			.clone()
	}

	pub fn is_shown(&self, name: &ImageName) -> bool {
		self.pictures
			.borrow()
			.get(name)
			.is_some_and(|picture| picture.with_untracked(Option::is_some))
	}

	pub fn show(&self, name: &ImageName, image: Option<Arc<ImageData>>) {
		let picture = match image {
			Some(_) => Some(self.picture(name)),
			None => self.pictures.borrow().get(name).cloned(),
		};
		if let Some(picture) = picture {
			picture.update(|slot| *slot = image);
		}
	}
}
