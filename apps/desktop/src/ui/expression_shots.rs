use bridge::PopoutSubject;
use protocol::{Snowflake, StickerFormat};
use session::Command;
use state::{Expression, State};
use zgx::input::InputEvent;
use zgx::scene::Rect;
use zgx::widgets::WeakUi;

use super::Link;
use crate::capture::{self, ExpressionShot};

struct Picks {
	emoji: Option<(Snowflake, String)>,
	sticker: Option<Snowflake>,
}

fn picks(state: &State, guild_id: Option<Snowflake>) -> Picks {
	let guild = guild_id.and_then(|guild_id| state.guild(guild_id));
	let emoji = guild.and_then(|guild| {
		guild.emojis.iter().find_map(|emoji| {
			(emoji.animated != Some(true)).then_some(())?;
			Some((emoji.id?, emoji.name.clone()?))
		})
	});
	let sticker = guild_id
		.and_then(|guild_id| state.guild_stickers(guild_id))
		.and_then(|stickers| {
			stickers
				.iter()
				.find(|sticker| sticker.format_type == Some(StickerFormat::Png))
				.map(|sticker| sticker.id)
		});
	Picks { emoji, sticker }
}

fn own_messages(state: &State, channel_id: Snowflake) -> usize {
	let Some(me) = state.current_user_id() else {
		return 0;
	};
	state.messages(channel_id).map_or(0, |cache| {
		if !cache.pending.is_empty() {
			return 0;
		}
		cache
			.ordered
			.iter()
			.filter(|message| message.author.id == me)
			.count()
	})
}

fn center(rect: Rect) -> (f32, f32) {
	(rect.x + rect.width / 2., rect.y + rect.height / 2.)
}

impl Link {
	fn own_count(&self, channel_id: Snowflake) -> usize {
		self.state()
			.map_or(0, |shared| own_messages(&shared.read(), channel_id))
	}

	async fn send_and_wait(&self, command: Command, channel_id: Snowflake) {
		let before = self.own_count(channel_id);
		self.send(command);
		self.until_capture(|link| link.own_count(channel_id) > before)
			.await;
	}

	async fn say(&self, channel_id: Snowflake, content: String) {
		self.send_and_wait(
			Command::SendMessage {
				channel_id,
				content,
				reply_to: None,
				ping_reply: true,
				files: Vec::new(),
			},
			channel_id,
		)
		.await;
	}

	pub(super) async fn drive_link_preview(&self) {
		let Some(channel_id) = self.timeline.borrow().open().map(|open| open.channel_id) else {
			tracing::warn!("no open channel to send the link in");
			return;
		};
		self.say(channel_id, capture::LINK_PREVIEW.to_owned()).await;
		self.until_capture(|link| link.timeline_view.settled()).await;
		zgx::timer::sleep(capture::SETTLE).await;
		self.point_at_last_message(false).await;
	}

	pub(super) async fn drive_expression_shot(&self, shot: ExpressionShot) {
		let Some(open) = self.timeline.borrow().open() else {
			tracing::warn!("no open channel to send the expressions in");
			return;
		};
		let channel_id = open.channel_id;
		let Some(chosen) = self.state().map(|shared| picks(&shared.read(), open.guild_id)) else {
			return;
		};
		self.say(channel_id, capture::GIF_LINK.to_owned()).await;
		if let Some(sticker_id) = chosen.sticker {
			self.send_and_wait(
				Command::SendSticker {
					channel_id,
					sticker_id,
				},
				channel_id,
			)
			.await;
		}
		let markup = chosen
			.emoji
			.as_ref()
			.map(|(id, name)| bridge::custom_markup(name, *id, false))
			.unwrap_or_default();
		self.say(
			channel_id,
			format!(
				"this one {markup} and {} are my favourites",
				capture::FAVOURITE_EMOJI
			),
		)
		.await;
		self.say(channel_id, format!("\u{1f480} {markup} {}", capture::JUMBO_EMOJI))
			.await;
		self.until_capture(|link| link.timeline_view.settled()).await;
		zgx::timer::sleep(capture::SETTLE).await;
		let wanted = move |subject: &PopoutSubject| match (shot, subject) {
			(ExpressionShot::CustomEmoji, PopoutSubject::Custom(Expression::Emoji { .. })) => true,
			(ExpressionShot::UnicodeEmoji, PopoutSubject::Unicode(glyph)) => glyph == capture::JUMBO_EMOJI,
			(ExpressionShot::Sticker, PopoutSubject::Custom(Expression::Sticker { .. })) => true,
			_ => false,
		};
		let found = self.timeline_view.popout_anchor(wanted);
		match found {
			Some((subject, rect)) => self.timeline_view.open_popout(subject, rect),
			None => tracing::warn!(?shot, "nothing to open a popout from"),
		}
		zgx::timer::sleep(capture::SETTLE).await;
		self.hover_newest_gif();
	}

	fn hover_newest_gif(&self) {
		let Some(rect) = self.timeline_view.newest_gif() else {
			tracing::warn!("no GIF to hover");
			return;
		};
		let Some(mut ui) = self.ui.borrow().as_ref().and_then(WeakUi::upgrade) else {
			return;
		};
		let (x, y) = center(rect);
		ui.dispatch(InputEvent::PointerMove { x, y });
	}
}
