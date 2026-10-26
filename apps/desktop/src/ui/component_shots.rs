use bridge::RowKey;
use mockcord::components as mock;
use mockcord::fixtures::showcase;
use protocol::{ComponentType, Snowflake};
use session::Command;

use super::Link;
use crate::capture::{self, ComponentShot};

const BUG_DETAILS: &str = "Hover a reaction with a long name";

impl Link {
	fn showcase_message(&self, content: &str) -> Option<Snowflake> {
		let timeline = self.timeline.borrow();
		timeline
			.messages()
			.skip_while(|entry| entry.content != showcase::marker(showcase::COMPONENTS_SECTION))
			.find(|entry| entry.content == content)
			.map(|entry| entry.id)
	}

	fn release_message(&self) -> Option<Snowflake> {
		let timeline = self.timeline.borrow();
		timeline
			.messages()
			.skip_while(|entry| entry.content != showcase::RELEASE_QUESTION)
			.find(|entry| !entry.message.components.is_empty())
			.map(|entry| entry.id)
	}

	async fn open_components_section(&self) -> Option<Snowflake> {
		let channel = self
			.sidebar
			.sidebar()
			.channels()
			.find(|channel| channel.name == showcase::CHANNEL_NAME)
			.map(|channel| channel.id);
		let Some(channel_id) = channel else {
			tracing::warn!("the showcase channel is missing");
			return None;
		};
		self.select_channel(channel_id);
		self.until_capture(|link| link.release_message().is_some() && link.timeline_view.settled())
			.await;
		Some(channel_id)
	}

	async fn scroll_to_message(&self, id: Snowflake, end: bool) {
		for _ in 0..capture::SCROLL_PASSES {
			if end {
				self.timeline_view.scroll_to_end(&RowKey::Message(id));
			} else {
				self.timeline_view.scroll_to(&RowKey::Message(id));
			}
			zgx::timer::sleep(capture::POLL).await;
		}
	}

	pub(super) async fn drive_component_shot(&self, shot: ComponentShot) {
		match shot {
			ComponentShot::Buttons | ComponentShot::MultiSelect => self.drive_component_messages(shot).await,
			ComponentShot::V2 => {
				if self.open_components_section().await.is_none() {
					return;
				}
				if let Some(question) = self.showcase_message(showcase::RELEASE_QUESTION) {
					self.scroll_to_message(question, false).await;
				}
			}
			ComponentShot::Modal | ComponentShot::ModalError => self.drive_app_modal(shot).await,
		}
	}

	async fn drive_component_messages(&self, shot: ComponentShot) {
		let Some(channel_id) = self.open_components_section().await else {
			return;
		};
		let (Some(buttons), Some(fruit), Some(fruits)) = (
			self.showcase_message(mock::BUTTONS_PROMPT),
			self.showcase_message(mock::FRUIT_PROMPT),
			self.showcase_message(mock::FRUITS_PROMPT),
		) else {
			tracing::warn!("the component messages were not found");
			return;
		};
		self.scroll_to_message(buttons, false).await;
		zgx::timer::sleep(capture::SETTLE).await;
		if shot == ComponentShot::MultiSelect {
			self.timeline_view
				.pick_in_select(fruits, "fruits", &["apple", "grapes"], true);
			return;
		}
		self.send(Command::PressComponent {
			channel_id,
			message_id: buttons,
			custom_id: "refresh".to_owned(),
			kind: ComponentType::Button,
			values: None,
		});
		self.send(Command::PressComponent {
			channel_id,
			message_id: fruit,
			custom_id: "fruit".to_owned(),
			kind: ComponentType::StringSelect,
			values: Some(vec!["banana".to_owned()]),
		});
		self.until_capture(|link| {
			link.timeline
				.borrow()
				.messages()
				.any(|entry| entry.id == buttons && entry.waiting_component.is_some())
		})
		.await;
		zgx::timer::sleep(capture::SETTLE).await;
		self.timeline_view
			.pick_in_select(fruit, "fruit", &["banana"], true);
	}

	async fn drive_app_modal(&self, shot: ComponentShot) {
		let Some(channel_id) = self.timeline.borrow().open().map(|open| open.channel_id) else {
			return;
		};
		self.send(Command::LoadApplicationCommands { channel_id });
		self.until_capture(move |link| {
			link.state()
				.is_some_and(|shared| shared.read().command_index(channel_id).is_some())
		})
		.await;
		self.send(Command::RunApplicationCommand {
			channel_id,
			command_id: Snowflake(mockcord::BUG_COMMAND),
			options: Vec::new(),
			files: Vec::new(),
		});
		self.until_capture(|link| link.timeline_view.modal_open()).await;
		zgx::timer::sleep(capture::SETTLE).await;
		self.timeline_view
			.type_into_modal("title", capture::BUG_TITLE_TEXT);
		if shot == ComponentShot::ModalError {
			self.timeline_view.type_into_modal("details", BUG_DETAILS);
			self.timeline_view.submit_modal();
			self.until_capture(|link| {
				link.state().is_some_and(|shared| {
					shared
						.read()
						.app_modal()
						.is_some_and(|open| open.status == state::ModalStatus::Failed)
				})
			})
			.await;
		}
	}
}
