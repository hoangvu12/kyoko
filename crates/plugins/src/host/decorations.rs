use std::collections::BTreeSet;
use std::sync::Arc;

use session::{Badge, DecoratedUser, Decorations};

use super::PluginHost;
use crate::calls::{Call, DECORATE_USER, Decor, Mark, Reply};
use crate::permission::{Permission, Scope, Slot};

fn decorates(grants: &BTreeSet<Permission>) -> bool {
	Slot::all()
		.filter(|slot| slot.is_decoration())
		.any(|slot| grants.contains(&Permission::Ui(slot)))
}

impl PluginHost {
	pub(crate) fn decorators(&self) -> Vec<String> {
		self.formatters_where(decorates, DECORATE_USER)
	}

	pub(crate) fn decorations_of(&self, user: &DecoratedUser) -> Option<Decorations> {
		let deadline = self.inner.config.limits.decoration_deadline;
		let mut decorations = Decorations::default();
		for target in self.inner.targets_where(decorates, DECORATE_USER, true) {
			let call = Call::DecorateUser {
				user: Box::new(user.clone()),
				presence: target.grants.contains(&Permission::Read(Scope::Users)),
			};
			match target.worker.call(call, deadline) {
				Ok(Reply::Decor(Some(decor))) => {
					self.add(&mut decorations, &target.id, &target.grants, user, decor)
				}
				Ok(_) => {}
				Err(fault) => self.inner.strike(&target.id, fault),
			}
		}
		(!decorations.is_plain()).then_some(decorations)
	}

	fn add(
		&self,
		decorations: &mut Decorations,
		plugin: &str,
		grants: &BTreeSet<Permission>,
		user: &DecoratedUser,
		decor: Decor,
	) {
		if grants.contains(&Permission::Ui(Slot::MemberDecoration)) {
			decorations.members.extend(self.badges(plugin, decor.members));
			decorations.crown |= decor.crown && user.owner;
		}
		if grants.contains(&Permission::Ui(Slot::UserDecoration)) {
			decorations.messages.extend(self.badges(plugin, decor.messages));
			decorations.profiles.extend(self.badges(plugin, decor.profiles));
		}
	}

	fn badges(&self, plugin: &str, marks: Vec<Mark>) -> Vec<Badge> {
		if marks.is_empty() {
			return Vec::new();
		}
		let registry = self.inner.registry.lock();
		let package = registry.entries.get(plugin).map(|entry| &entry.package);
		marks
			.into_iter()
			.map(|mark| Badge {
				plugin: plugin.to_owned(),
				key: mark.icon.trim_end_matches(".svg").to_owned(),
				icon: package
					.and_then(|package| package.read_icon(&mark.icon))
					.map(Arc::from),
				color: mark.color,
				tooltip: mark.tooltip,
			})
			.collect()
	}
}
