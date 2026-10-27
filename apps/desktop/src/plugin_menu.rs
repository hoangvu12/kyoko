use std::cell::RefCell;
use std::rc::Rc;

use bridge::{MenuFacts, PluginMenuItem};
use protocol::Snowflake;
use state::Expression;
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent};

use crate::timeline::{MENU_ICON, menu_row, plugin_icon};

type Hook<T> = RefCell<Option<Box<dyn Fn(T)>>>;
type Facts = RefCell<Option<Box<dyn Fn(Option<Expression>) -> MenuFacts>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
	pub item: PluginMenuItem,
	pub target: Option<Expression>,
	pub message: Option<(Snowflake, Snowflake)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessoryClick {
	pub plugin: String,
	pub key: String,
	pub channel_id: Snowflake,
	pub message_id: Snowflake,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown {
	pub items: Rc<[PluginMenuItem]>,
	pub facts: MenuFacts,
}

impl Shown {
	pub fn entries(&self) -> Vec<bridge::MenuEntry> {
		bridge::plugin_entries(&self.items, &self.facts)
	}
}

#[derive(Default)]
pub struct PluginMenu {
	items: RefCell<Rc<[PluginMenuItem]>>,
	facts: Facts,
	chosen: Hook<Choice>,
	pressed: Hook<()>,
	accessed: Hook<AccessoryClick>,
}

impl PluginMenu {
	pub fn new() -> Rc<Self> {
		Rc::new(Self::default())
	}

	pub fn set_items(&self, items: Vec<PluginMenuItem>) {
		if *self.items.borrow().as_ref() != items[..] {
			*self.items.borrow_mut() = items.into();
		}
	}

	pub fn on_facts(&self, facts: impl Fn(Option<Expression>) -> MenuFacts + 'static) {
		*self.facts.borrow_mut() = Some(Box::new(facts));
	}

	pub fn on_choose(&self, chosen: impl Fn(Choice) + 'static) {
		*self.chosen.borrow_mut() = Some(Box::new(chosen));
	}

	pub fn on_press(&self, pressed: impl Fn(()) + 'static) {
		*self.pressed.borrow_mut() = Some(Box::new(pressed));
	}

	pub fn on_accessory(&self, accessed: impl Fn(AccessoryClick) + 'static) {
		*self.accessed.borrow_mut() = Some(Box::new(accessed));
	}

	pub fn press(&self) {
		if let Some(pressed) = self.pressed.borrow().as_ref() {
			pressed(());
		}
	}

	pub fn access(&self, click: AccessoryClick) {
		if let Some(accessed) = self.accessed.borrow().as_ref() {
			accessed(click);
		}
	}

	pub fn has_items(&self) -> bool {
		!self.items.borrow().is_empty()
	}

	pub fn shown(&self, target: Option<Expression>) -> Shown {
		let items = self.items.borrow().clone();
		if items.is_empty() {
			return Shown::default();
		}
		let facts = match self.facts.borrow().as_ref() {
			Some(facts) => facts(target),
			None => MenuFacts {
				target,
				can_create_expressions: false,
			},
		};
		Shown { items, facts }
	}

	pub fn item(
		self: &Rc<Self>,
		shown: &Shown,
		index: usize,
		message: Option<(Snowflake, Snowflake)>,
		then: impl Fn() + 'static,
	) -> View {
		let Some(item) = shown.items.get(index).cloned() else {
			return div();
		};
		let glyph = plugin_icon(item.icon.as_ref(), MENU_ICON, "text_4");
		let (pressing, choosing) = (self.clone(), self.clone());
		let choice = Choice {
			item: item.clone(),
			target: shown.facts.target.clone(),
			message,
		};
		menu_row(item.label.clone(), glyph, "text_3")
			.id(format!("plugin-menu-{}", item.key))
			.on_event(move |event| {
				if event.phase != EventPhase::Capture
					&& matches!(event.event, InputEvent::PointerDown { .. })
					&& let Some(pressed) = pressing.pressed.borrow().as_ref()
				{
					pressed(());
				}
			})
			.on_click(move || {
				if let Some(chosen) = choosing.chosen.borrow().as_ref() {
					chosen(choice.clone());
				}
				then();
			})
	}
}
