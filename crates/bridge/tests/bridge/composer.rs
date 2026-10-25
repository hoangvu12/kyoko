use bridge::{ChannelKind, Composer, Notice, Visual};
use chrono::Utc;
use mockcord::Reply as MockReply;
use std::time::Duration;

use protocol::{ChannelType, CommandOptionType, Snowflake};
use rest::UploadFile;
use serde_json::json;
use session::Command;
use state::{RiskyReason, RiskyTarget, State};

use crate::harness::Running;

const ECHO_ID: Snowflake = Snowflake(9_100_000_000_000_000_102);
const ROLL_ID: Snowflake = Snowflake(9_100_000_000_000_000_103);

async fn opened(name: &str) -> (Running, Composer, Snowflake) {
	let mut running = Running::start(name).await;
	let guild_id = running.view.sidebar.selected();
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	if let Some(command) = running.open_channel(channel_id) {
		running.session().send(command);
	}
	running
		.until("the channel history", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	let mut composer = Composer::default();
	let state = running.session().state().clone();
	composer.open(&state.read(), guild_id, Some(channel_id));
	(running, composer, channel_id)
}

fn other_author(state: &State, channel_id: Snowflake) -> (Snowflake, Snowflake) {
	let me = state.current_user_id();
	state
		.messages(channel_id)
		.and_then(|cache| {
			cache
				.ordered
				.iter()
				.rev()
				.find(|message| Some(message.author.id) != me)
		})
		.map(|message| (message.id, message.author.id))
		.expect("a message from someone else")
}

fn typed(composer: &mut Composer, running: &Running, text: &str) -> Vec<Command> {
	let state = running.session().state().clone();
	let state = state.read();
	composer.changed(&state, text, text.len())
}

fn submitted(composer: &mut Composer, running: &Running, text: &str) -> bridge::Outcome {
	let state = running.session().state().clone();
	let state = state.read();
	composer.submit(&state, text, Utc::now())
}

async fn with_commands(running: &mut Running, composer: &mut Composer, channel_id: Snowflake) {
	let commands = typed(composer, running, "/");
	assert!(commands.iter().any(
		|command| matches!(command, Command::LoadApplicationCommands { channel_id: asked } if *asked == channel_id)
	));
	for command in commands {
		running.session().send(command);
	}
	running
		.until_state("the command index", |state| {
			state.command_index(channel_id).is_some()
		})
		.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mentions_channels_and_emoji_complete_from_state() {
	let (running, mut composer, channel_id) = opened("composer-complete").await;
	let state = running.session().state().clone();
	let (_, author_id) = other_author(&state.read(), channel_id);
	let name = state.read().display_name(composer.guild(), author_id);
	let prefix: String = name.chars().take(3).collect();
	let text = format!("hello @{prefix}");
	typed(&mut composer, &running, &text);
	let index = composer
		.suggestions()
		.iter()
		.position(|suggestion| suggestion.visual == Visual::Avatar(author_id))
		.expect("the author is suggested");
	let (completed, caret) = composer.choose(&text, Some(index)).expect("a completion");
	assert_eq!(completed, format!("hello <@{author_id}> "));
	assert_eq!(caret, completed.len());
	assert!(composer.suggestions().is_empty());

	let channel_name = state
		.read()
		.channel(channel_id)
		.and_then(|channel| channel.name.clone())
		.expect("a channel name");
	let text = format!("see #{}", &channel_name[..2]);
	typed(&mut composer, &running, &text);
	let (completed, _) = composer
		.choose(
			&text,
			composer
				.suggestions()
				.iter()
				.position(|suggestion| suggestion.label == channel_name),
		)
		.expect("the channel is suggested");
	assert_eq!(completed, format!("see <#{channel_id}> "));

	typed(&mut composer, &running, "nice :smil");
	assert!(
		composer
			.suggestions()
			.iter()
			.any(|suggestion| suggestion.label == ":smile:" && suggestion.insert == "😄 ")
	);
	typed(&mut composer, &running, "nice :s");
	assert!(composer.suggestions().is_empty());
	assert!(composer.dismiss() || composer.suggestions().is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_commands_complete_and_parse_their_options() {
	let (mut running, mut composer, channel_id) = opened("composer-slash").await;
	with_commands(&mut running, &mut composer, channel_id).await;
	typed(&mut composer, &running, "/ec");
	assert_eq!(composer.suggestions()[0].label, "/echo");
	assert_eq!(
		composer.suggestions()[0].detail.as_deref(),
		Some("Repeat some text · Mockbot")
	);

	for text in ["/echo text:hi there", "/echo hi there"] {
		let outcome = submitted(&mut composer, &running, text);
		match outcome.commands.first() {
			Some(Command::RunApplicationCommand {
				command_id, options, ..
			}) => {
				assert_eq!(*command_id, ECHO_ID);
				assert_eq!(options[0].kind, CommandOptionType::String);
				assert_eq!(options[0].value, Some(json!("hi there")));
			}
			other => panic!("{text}: expected a command, got {other:?}"),
		}
		assert_eq!(outcome.replace, Some((String::new(), 0)));
	}

	let outcome = submitted(&mut composer, &running, "/roll sides:twenty");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::RunApplicationCommand { command_id, options, .. })
			if *command_id == ROLL_ID && options[0].value == Some(json!(20))
	));

	let outcome = submitted(&mut composer, &running, "/roll sides:7");
	assert!(outcome.commands.is_empty());
	assert!(outcome.replace.is_none());
	let state = running.session().state().clone();
	assert_eq!(
		composer.notice(&state.read(), Utc::now()),
		Some(Notice::Refused(
			"/roll: sides must be one of six, twenty".to_owned()
		))
	);

	let outcome = submitted(&mut composer, &running, "/echo");
	assert!(outcome.commands.is_empty());
	assert_eq!(
		composer.notice(&state.read(), Utc::now()),
		Some(Notice::Refused("/echo needs text".to_owned()))
	);

	let outcome = submitted(&mut composer, &running, "/shrug not a command");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::SendMessage { content, .. }) if content == "/shrug not a command"
	));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reply_carries_the_ping_choice_and_clears_after_sending() {
	let (running, mut composer, channel_id) = opened("composer-reply").await;
	let state = running.session().state().clone();
	let (message_id, author_id) = other_author(&state.read(), channel_id);
	assert!(composer.start_reply(&state.read(), message_id));
	let reply = composer.reply().cloned().expect("replying");
	assert!(reply.ping);
	assert_eq!(reply.name, state.read().display_name(composer.guild(), author_id));
	composer.toggle_ping();
	let outcome = submitted(&mut composer, &running, "  thanks  ");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::SendMessage { content, reply_to, ping_reply: false, files, .. })
			if content == "thanks" && *reply_to == Some(message_id) && files.is_empty()
	));
	assert!(matches!(
		outcome.commands.get(1),
		Some(Command::SetDraft { content, .. }) if content.is_empty()
	));
	assert!(composer.reply().is_none());
	assert!(submitted(&mut composer, &running, "   ").commands.is_empty());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_last_own_message_opens_for_editing() {
	let (mut running, mut composer, channel_id) = opened("composer-edit").await;
	running.session().send(Command::SendMessage {
		channel_id,
		content: "typo here".to_owned(),
		reply_to: None,
		ping_reply: true,
		files: Vec::new(),
	});
	running
		.until_state("the sent message", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty() && cache.ordered.iter().any(|message| message.content == "typo here")
			})
		})
		.await;
	let state = running.session().state().clone();
	let (content, caret) = composer.edit_last(&state.read()).expect("an own message");
	assert_eq!(content, "typo here");
	assert_eq!(caret, content.len());
	let editing = composer.editing().expect("editing");
	assert!(typed(&mut composer, &running, "typo fixed").is_empty());
	let outcome = submitted(&mut composer, &running, "typo fixed");
	assert!(matches!(
		outcome.commands.as_slice(),
		[Command::EditMessage { message_id, content, .. }] if *message_id == editing && content == "typo fixed"
	));
	assert!(composer.editing().is_none());
	let (other, _) = other_author(&state.read(), channel_id);
	assert!(composer.start_edit(&state.read(), other).is_none());
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_send_becomes_a_notice_with_retry() {
	let (mut running, mut composer, channel_id) = opened("composer-failed").await;
	running.server.control().respond_once(
		"POST",
		&format!("/channels/{channel_id}/messages"),
		MockReply::captcha_required(),
	);
	for command in submitted(&mut composer, &running, "needs a captcha").commands {
		running.session().send(command);
	}
	running
		.until_state("the failure", |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.pending.iter().any(|pending| pending.failure.is_some()))
		})
		.await;
	let state = running.session().state().clone();
	let notice = composer.notice(&state.read(), Utc::now());
	let Some(Notice::Failed { nonce, text }) = notice else {
		panic!("expected a failure notice, got {notice:?}");
	};
	assert_eq!(
		text,
		"Discord asked for a captcha. Complete this in the official client."
	);
	assert!(matches!(
		composer.retry(nonce.clone()),
		Some(Command::RetryMessage { nonce: retried, .. }) if retried == nonce
	));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attachments_are_checked_against_the_limit() {
	let (running, mut composer, channel_id) = opened("composer-files").await;
	let state = running.session().state().clone();
	let limit = state.read().upload_limit(channel_id);
	let small = UploadFile {
		name: "small.png".to_owned(),
		bytes: vec![1u8; 16].into(),
		content_type: "image/png".to_owned(),
	};
	let id = composer.attach(&state.read(), small).expect("a small file fits");
	assert!(limit >= 10 * 1024 * 1024);
	for index in 0..9 {
		let file = UploadFile {
			name: format!("{index}.txt"),
			bytes: vec![2u8; 8].into(),
			content_type: "text/plain".to_owned(),
		};
		composer.attach(&state.read(), file).expect("room for ten");
	}
	let eleventh = UploadFile {
		name: "eleventh.txt".to_owned(),
		bytes: vec![3u8; 8].into(),
		content_type: "text/plain".to_owned(),
	};
	assert_eq!(
		composer.attach(&state.read(), eleventh),
		Err("You can attach up to 10 files".to_owned())
	);
	assert!(composer.remove_file(id));
	assert_eq!(composer.files().len(), 9);
	let outcome = submitted(&mut composer, &running, "");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::SendMessage { files, content, .. }) if files.len() == 9 && content.is_empty()
	));
	assert!(composer.files().is_empty());
	running.stop().await;
}

async fn quiet_channel(running: &mut Running, guild_id: Snowflake) -> Snowflake {
	running.session().send(Command::CreateChannel {
		guild_id,
		name: "quiet-corner".to_owned(),
		kind: ChannelType::GuildText,
		parent_id: None,
	});
	running
		.until_state("the new channel", move |state| {
			state.guild(guild_id).is_some_and(|guild| {
				guild
					.channels
					.values()
					.any(|channel| channel.name.as_deref() == Some("quiet-corner"))
			})
		})
		.await;
	let channel_id = running
		.session()
		.state()
		.read()
		.guild(guild_id)
		.and_then(|guild| {
			guild
				.channels
				.values()
				.find(|channel| channel.name.as_deref() == Some("quiet-corner"))
				.map(|channel| channel.id)
		})
		.expect("the new channel");
	running.session().send(Command::SelectChannel {
		guild_id: Some(guild_id),
		channel_id,
	});
	running
		.until_state("the empty history", move |state| {
			state
				.messages(channel_id)
				.is_some_and(|cache| cache.reached_start && cache.ordered.is_empty())
		})
		.await;
	channel_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_empty_channel_holds_the_send_until_the_user_confirms() {
	let (mut running, _, _) = opened("composer-lock").await;
	let guild_id = running.view.sidebar.selected().expect("a selected guild");
	let channel_id = quiet_channel(&mut running, guild_id).await;
	let state = running.session().state().clone();
	let mut composer = Composer::default();
	composer.open(&state.read(), Some(guild_id), Some(channel_id));
	assert_eq!(
		composer.notice(&state.read(), Utc::now()),
		Some(Notice::Lock {
			text: RiskyReason::EmptyChannel.explanation().to_owned(),
			confirmable: true,
		})
	);
	let held = submitted(&mut composer, &running, "first post");
	assert!(held.commands.is_empty());
	assert!(held.replace.is_none());
	let confirmed = composer.confirm_lock(&state.read(), Utc::now());
	assert_eq!(confirmed.replace, Some((String::new(), 0)));
	match confirmed.commands.as_slice() {
		[
			Command::ConfirmRisky { target, reason },
			Command::SendMessage {
				content,
				channel_id: sent_to,
				..
			},
		] => {
			assert_eq!(*target, RiskyTarget::Channel(channel_id));
			assert_eq!(*reason, RiskyReason::EmptyChannel);
			assert_eq!(content, "first post");
			assert_eq!(*sent_to, channel_id);
		}
		other => panic!("expected a confirmation and the held send, got {other:?}"),
	}
	for command in confirmed.commands {
		running.session().send(command);
	}
	running
		.until_state("the held message to arrive", move |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty()
					&& cache
						.ordered
						.iter()
						.any(|message| message.content == "first post")
			})
		})
		.await;
	tokio::time::sleep(Duration::from_millis(600)).await;
	let path = format!("/channels/{channel_id}/messages");
	let posts = running
		.server
		.control()
		.recording()
		.requests_to("POST", &path)
		.count();
	assert_eq!(posts, 1);
	assert_eq!(composer.notice(&state.read(), Utc::now()), None);
	assert!(
		composer
			.confirm_lock(&state.read(), Utc::now())
			.commands
			.is_empty()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_gate_points_to_the_official_client_without_a_confirm() {
	let (mut running, mut composer, channel_id) = opened("composer-gate").await;
	let guild_id = running.view.sidebar.selected().expect("a selected guild");
	let guild = running
		.server
		.fixtures()
		.guilds
		.iter()
		.find(|guild| guild.id == guild_id.0)
		.expect("the guild fixture");
	let member = json!({
		"guild_id": guild_id.to_string(),
		"user": running.server.fixtures().current_user,
		"roles": guild.current_member().roles,
		"joined_at": "2025-01-01T00:00:00+00:00",
		"pending": true,
		"flags": 0,
	});
	running.server.control().dispatch("GUILD_MEMBER_UPDATE", &member);
	running
		.until_state("the screening gate", move |state| {
			state
				.channel_lock(channel_id, Utc::now())
				.is_some_and(|lock| lock.reason == RiskyReason::Screening)
		})
		.await;
	let state = running.session().state().clone();
	let notice = composer.notice(&state.read(), Utc::now());
	assert_eq!(
		notice,
		Some(Notice::Lock {
			text: RiskyReason::Screening.explanation().to_owned(),
			confirmable: false,
		})
	);
	let Some(Notice::Lock { text, .. }) = notice else {
		unreachable!();
	};
	assert!(text.contains("in the official client"));
	assert!(
		submitted(&mut composer, &running, "let me in")
			.commands
			.is_empty()
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_picked_sticker_waits_for_the_lock_and_keeps_the_draft() {
	let (mut running, _, _) = opened("composer-sticker-lock").await;
	let guild_id = running.view.sidebar.selected().expect("a selected guild");
	let channel_id = quiet_channel(&mut running, guild_id).await;
	let state = running.session().state().clone();
	let mut composer = Composer::default();
	composer.open(&state.read(), Some(guild_id), Some(channel_id));
	let picker = bridge::StickerPicker::of(&state.read(), Some(guild_id), channel_id);
	let cell = picker.sections()[0].stickers[0].clone();
	let held = composer.pick_sticker(&state.read(), &cell, Utc::now());
	assert!(held.commands.is_empty(), "a locked channel holds the sticker");
	let confirmed = composer.confirm_lock(&state.read(), Utc::now());
	assert_eq!(
		confirmed.replace, None,
		"releasing a sticker leaves the draft alone"
	);
	match confirmed.commands.as_slice() {
		[
			Command::ConfirmRisky { reason, .. },
			Command::SendSticker { sticker_id, .. },
		] => {
			assert_eq!(*reason, RiskyReason::EmptyChannel);
			assert_eq!(*sticker_id, cell.sticker.id);
		}
		other => panic!("expected a confirmation and the held sticker, got {other:?}"),
	}
	running.stop().await;
}
