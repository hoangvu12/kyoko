use std::time::{Duration, Instant};

use bridge::{ChannelKind, CommandResult, Composer, Moderation, Outcome};
use chrono::Utc;
use mockcord::RestRequest;
use protocol::{Permissions, Snowflake};
use serde_json::{Value, json};
use session::Command;
use state::State;

use crate::harness::Running;

const LATER: Duration = Duration::from_secs(1);

async fn opened(name: &str) -> (Running, Composer, Snowflake, Snowflake) {
	let mut running = Running::start(name).await;
	let guild_id = running.view.sidebar.selected().expect("a server is selected");
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
	let user_ids: Vec<Snowflake> = running.server.fixtures().guilds[0]
		.members
		.iter()
		.filter_map(|member| member.user.as_ref().map(|user| user.id))
		.collect();
	running
		.session()
		.send(Command::RequestMembers { guild_id, user_ids });
	running
		.until_state("the server's members", |state| {
			state
				.guild(guild_id)
				.is_some_and(|guild| guild.members.len() > 3 && guild.owner_id.is_some())
		})
		.await;
	let mut composer = Composer::default();
	composer.open(
		&running.session().state().read(),
		Some(guild_id),
		Some(channel_id),
	);
	(running, composer, guild_id, channel_id)
}

fn listed(composer: &mut Composer, running: &Running) -> Vec<String> {
	let state = running.session().state().clone();
	composer.changed(&state.read(), "/", 1);
	composer
		.picker()
		.map(|picker| {
			picker
				.rows
				.iter()
				.filter(|row| row.built_in().is_some())
				.map(|row| row.name())
				.collect()
		})
		.unwrap_or_default()
}

fn pick(composer: &mut Composer, running: &Running, name: &str) {
	let state = running.session().state().clone();
	let typed = format!("/{name}");
	composer.changed(&state.read(), &typed, typed.len());
	let index = composer
		.picker()
		.and_then(|picker| {
			picker
				.rows
				.iter()
				.position(|row| row.built_in().is_some() && row.name() == name)
		})
		.unwrap_or_else(|| panic!("/{name} is listed"));
	composer.choose("", Some(index));
	assert_eq!(
		composer.command().map(|command| command.entry.name()),
		Some(name.to_owned())
	);
}

fn fill(composer: &mut Composer, values: &[&str]) {
	for (index, value) in values.iter().enumerate() {
		if composer
			.command()
			.is_some_and(|command| command.fields.len() <= index)
		{
			assert!(composer.add_optional(None), "an optional field to add");
		}
		assert!(composer.edit_option(index, value));
	}
}

fn submit(composer: &mut Composer, running: &Running) -> Outcome {
	let state = running.session().state().clone();
	composer.submit(&state.read(), "", Utc::now())
}

fn send_all(running: &Running, commands: Vec<Command>) {
	for command in commands {
		running.session().send(command);
	}
}

fn me(running: &Running) -> Snowflake {
	running
		.session()
		.state()
		.read()
		.current_user_id()
		.expect("signed in")
}

fn other_member(running: &Running, guild_id: Snowflake) -> (Snowflake, String) {
	let state = running.session().state().clone();
	let state = state.read();
	let me = state.current_user_id().expect("signed in");
	let guild = state.guild(guild_id).expect("the server");
	let owner = guild.owner_id;
	let mut ids: Vec<Snowflake> = guild
		.members
		.iter()
		.map(|(id, _)| *id)
		.filter(|id| *id != me && Some(*id) != owner && state.outranks(guild_id, me, *id))
		.collect();
	ids.sort();
	let id = ids[0];
	let username = state.user(id).expect("a known user").username.clone();
	(id, username)
}

fn requests(running: &Running, method: &str, path: &str) -> Vec<RestRequest> {
	running
		.server
		.control()
		.recording()
		.requests_to(method, path)
		.cloned()
		.collect()
}

fn body(request: &RestRequest) -> Value {
	request.json().expect("a json body")
}

async fn sent(running: &mut Running, method: &str, path: String) -> Vec<RestRequest> {
	let deadline = Instant::now() + Duration::from_secs(10);
	loop {
		let found = requests(running, method, &path);
		if !found.is_empty() || Instant::now() > deadline {
			return found;
		}
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
}

fn key_counted(outcome: &Outcome, key: &str) -> bool {
	outcome
		.commands
		.iter()
		.any(|command| matches!(command, Command::CountCommandUse { key: counted } if counted == key))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn moderation_built_ins_appear_only_with_the_permission() {
	let (mut running, mut composer, guild_id, _) = opened("built-ins-permission").await;
	let names = listed(&mut composer, &running);
	for name in ["ban", "kick", "timeout", "nick", "thread"] {
		assert!(
			names.iter().any(|row| row == name),
			"/{name} with the permission: {names:?}"
		);
	}
	let user = running.server.fixtures().current_user.clone();
	running.server.control().dispatch(
		"GUILD_MEMBER_UPDATE",
		&json!({ "guild_id": guild_id.to_string(), "user": user, "roles": [], "joined_at": "2025-01-01T00:00:00+00:00" }),
	);
	running
		.until_state("the current user without roles", |state: &State| {
			let me = state.current_user_id().expect("signed in");
			!state
				.guild_permissions(guild_id, me)
				.contains(Permissions::KICK_MEMBERS)
		})
		.await;
	composer.dismiss();
	let names = listed(&mut composer, &running);
	for name in ["ban", "kick", "timeout"] {
		assert!(
			!names.iter().any(|row| row == name),
			"/{name} without the permission: {names:?}"
		);
	}
	for name in [
		"gif",
		"me",
		"msg",
		"shrug",
		"spoiler",
		"sticker",
		"tableflip",
		"unflip",
	] {
		assert!(
			names.iter().any(|row| row == name),
			"/{name} is for everyone: {names:?}"
		);
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ban_kick_and_timeout_open_the_confirmation_then_reach_discord_as_the_dialogs_send() {
	let (mut running, mut composer, guild_id, _) = opened("built-ins-moderation").await;
	let (target, username) = other_member(&running, guild_id);

	pick(&mut composer, &running, "ban");
	fill(&mut composer, &[&format!("@{username}"), "Previous Hour"]);
	assert!(composer.add_optional(None));
	assert!(composer.edit_option(2, "spam"));
	let outcome = submit(&mut composer, &running);
	let ask = outcome.moderate.clone().expect("the ban dialog opens");
	assert_eq!(
		(
			ask.action,
			ask.guild_id,
			ask.user_id,
			ask.seconds,
			ask.reason.as_deref()
		),
		(Moderation::Ban, guild_id, target, Some(3600), Some("spam"))
	);
	assert!(
		!outcome
			.commands
			.iter()
			.any(|command| matches!(command, Command::BanMember { .. })),
		"nothing reaches Discord before the dialog is confirmed"
	);
	assert!(key_counted(&outcome, "-12"));
	running.session().send(Command::BanMember {
		guild_id,
		user_id: target,
		delete_message_seconds: 3600,
		reason: ask.reason.clone(),
	});
	let banned = sent(&mut running, "PUT", format!("/guilds/{guild_id}/bans/{target}")).await;
	assert_eq!(banned.len(), 1);
	assert_eq!(body(&banned[0])["delete_message_seconds"], json!(3600));

	pick(&mut composer, &running, "kick");
	fill(&mut composer, &[&target.to_string()]);
	let outcome = submit(&mut composer, &running);
	let ask = outcome.moderate.clone().expect("the kick dialog opens");
	assert_eq!(
		(ask.action, ask.user_id, ask.reason),
		(Moderation::Kick, target, None)
	);
	assert!(key_counted(&outcome, "-11"));

	pick(&mut composer, &running, "timeout");
	fill(&mut composer, &[&format!("<@{target}>"), "1 hour"]);
	let outcome = submit(&mut composer, &running);
	let ask = outcome.moderate.clone().expect("the timeout dialog opens");
	assert_eq!((ask.action, ask.seconds), (Moderation::Timeout, Some(3600)));
	assert!(key_counted(&outcome, "-13"));

	pick(&mut composer, &running, "kick");
	fill(&mut composer, &["@nobody-by-that-name"]);
	let outcome = submit(&mut composer, &running);
	assert!(outcome.moderate.is_none() && outcome.commands.is_empty());
	assert!(
		composer
			.command()
			.and_then(|command| command.fields[0].error.clone())
			.is_some(),
		"an unknown user is refused in its chip"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn msg_opens_the_conversation_sends_the_message_and_shows_it() {
	let (mut running, mut composer, guild_id, _) = opened("built-ins-msg").await;
	let (target, username) = other_member(&running, guild_id);
	pick(&mut composer, &running, "msg");
	fill(&mut composer, &[&username, "hello there"]);
	let outcome = submit(&mut composer, &running);
	assert!(key_counted(&outcome, "-14"));
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::MessageUser { user_id, content }) if *user_id == target && content == "hello there"
	));
	send_all(&running, outcome.commands);
	running
		.until_state("the conversation is open", |state| {
			state.selected().is_some_and(|selection| {
				selection.guild_id.is_none()
					&& state.channel(selection.channel_id).is_some_and(|channel| {
						channel.recipient_ids.contains(&target)
							|| channel.recipients.iter().any(|user| user.id == target)
					})
			})
		})
		.await;
	let dm = running
		.session()
		.state()
		.read()
		.selected()
		.map(|selection| selection.channel_id)
		.expect("the conversation");
	let opened = requests(&running, "POST", "/users/@me/channels");
	assert_eq!(body(&opened[0]), json!({ "recipients": [target.to_string()] }));
	let messages = sent(&mut running, "POST", format!("/channels/{dm}/messages")).await;
	assert_eq!(messages.len(), 1);
	assert_eq!(body(&messages[0])["content"], json!("hello there"));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nick_changes_my_nickname_on_this_server_and_an_empty_one_resets_it() {
	let (mut running, mut composer, guild_id, _) = opened("built-ins-nick").await;
	let me = me(&running);
	pick(&mut composer, &running, "nick");
	fill(&mut composer, &["night owl"]);
	let outcome = submit(&mut composer, &running);
	assert!(key_counted(&outcome, "-7"));
	send_all(&running, outcome.commands);
	running
		.until_state("the new nickname", |state| {
			state.display_name(Some(guild_id), me) == "night owl"
		})
		.await;
	pick(&mut composer, &running, "nick");
	let outcome = submit(&mut composer, &running);
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::SetNickname { nick: None, user_id, .. }) if *user_id == me
	));
	send_all(&running, outcome.commands);
	let deadline = Instant::now() + Duration::from_secs(10);
	let path = format!("/guilds/{guild_id}/members/@me");
	while requests(&running, "PATCH", &path).len() < 2 && Instant::now() < deadline {
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	let changes: Vec<Value> = requests(&running, "PATCH", &path).iter().map(body).collect();
	assert_eq!(changes, [json!({ "nick": "night owl" }), json!({ "nick": null })]);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_starts_a_thread_and_sends_its_first_message_inside() {
	let (mut running, mut composer, _, channel_id) = opened("built-ins-thread").await;
	pick(&mut composer, &running, "thread");
	assert_eq!(
		composer.command().map(|command| command
			.fields
			.iter()
			.map(|field| field.option.name.clone())
			.collect::<Vec<_>>()),
		Some(vec!["name".to_owned(), "message".to_owned()])
	);
	fill(&mut composer, &["picker polish", "notes from testing"]);
	let outcome = submit(&mut composer, &running);
	assert!(key_counted(&outcome, "-10"));
	assert_eq!(
		outcome.thread_message,
		Some((channel_id, "notes from testing".to_owned()))
	);
	send_all(&running, outcome.commands);
	running
		.until_state("the new thread", |state| {
			state.selected().is_some_and(|selection| {
				state.channel(selection.channel_id).is_some_and(|channel| {
					channel.parent_id == Some(channel_id) && channel.name.as_deref() == Some("picker polish")
				})
			})
		})
		.await;
	let thread = running
		.session()
		.state()
		.read()
		.selected()
		.map(|selection| selection.channel_id)
		.expect("the thread");
	running.session().send(Command::SendMessage {
		channel_id: thread,
		content: "notes from testing".to_owned(),
		reply_to: None,
		ping_reply: false,
		files: Vec::new(),
	});
	let created = requests(&running, "POST", &format!("/channels/{channel_id}/threads"));
	assert_eq!(
		body(&created[0]),
		json!({ "name": "picker polish", "auto_archive_duration": 4320, "type": 11 })
	);
	let messages = sent(&mut running, "POST", format!("/channels/{thread}/messages")).await;
	assert_eq!(body(&messages[0])["content"], json!("notes from testing"));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gif_searches_after_typing_pauses_and_enter_sends_the_selected_result() {
	let (mut running, mut composer, _, channel_id) = opened("built-ins-gif").await;
	pick(&mut composer, &running, "gif");
	fill(&mut composer, &["cat"]);
	let state = running.session().state().clone();
	let now = Instant::now();
	assert!(composer.strip_requests(&state.read(), now).is_empty());
	assert!(composer.results(&state.read()).is_none());
	let searched = composer.strip_requests(&state.read(), now + LATER);
	assert!(matches!(searched.as_slice(), [Command::SearchGifs { query }] if query == "cat"));
	send_all(&running, searched);
	running
		.until_state("the gif results", |state| state.gif_search("cat").is_some())
		.await;
	let results = composer.results(&state.read()).expect("the strip");
	assert!(results.items.len() > 1);
	assert!(composer.move_result(&state.read(), 1));
	let second = match &composer.results(&state.read()).expect("the strip").items[1] {
		CommandResult::Gif(gif) => gif.clone(),
		CommandResult::Sticker(_) => panic!("a gif"),
	};
	let outcome = submit(&mut composer, &running);
	assert!(key_counted(&outcome, "-16"));
	assert!(composer.command().is_none());
	send_all(&running, outcome.commands);
	let messages = sent(&mut running, "POST", format!("/channels/{channel_id}/messages")).await;
	assert_eq!(body(&messages[0])["content"], json!(second.url));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sticker_lists_my_stickers_and_a_click_sends_one() {
	let (mut running, mut composer, _, channel_id) = opened("built-ins-sticker").await;
	pick(&mut composer, &running, "sticker");
	let state = running.session().state().clone();
	let needs = composer.strip_requests(&state.read(), Instant::now());
	assert!(!needs.is_empty());
	send_all(&running, needs);
	running
		.until_state("the stickers", |state| state.sticker_packs().is_some())
		.await;
	let first = match composer.results(&state.read()).expect("the strip").items.first() {
		Some(CommandResult::Sticker(cell)) => cell.sticker.clone(),
		_ => panic!("a sticker"),
	};
	let query: String = first.name.chars().take(3).collect();
	fill(&mut composer, &[&query]);
	let found = composer.results(&state.read()).expect("the strip");
	assert!(found.items.iter().all(|item| matches!(
		item,
		CommandResult::Sticker(cell) if cell.sticker.name.to_lowercase().contains(&query.to_lowercase())
			|| cell.sticker.tags.as_deref().is_some_and(|tags| tags.to_lowercase().contains(&query.to_lowercase()))
	)));
	let outcome = composer.choose_result(&state.read(), Some(0), Utc::now());
	assert!(key_counted(&outcome, "-17"));
	send_all(&running, outcome.commands);
	let messages = sent(&mut running, "POST", format!("/channels/{channel_id}/messages")).await;
	assert_eq!(body(&messages[0])["sticker_ids"], json!([first.id.to_string()]));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_built_ins_change_the_message_as_discord_does() {
	let (running, mut composer, _, _) = opened("built-ins-text").await;
	for (name, typed, content, key) in [
		("shrug", "", "¯\\_(ツ)_/¯", "-1"),
		("tableflip", "ok", "ok (╯°□°)╯︵ ┻━┻", "-2"),
		("unflip", "", "┬─┬ノ( º _ ºノ)", "-3"),
		("me", "waves", "_waves_", "-5"),
		("spoiler", "the end", "||the end||", "-6"),
	] {
		pick(&mut composer, &running, name);
		if !typed.is_empty() {
			fill(&mut composer, &[typed]);
		}
		let outcome = submit(&mut composer, &running);
		assert!(
			matches!(outcome.commands.first(), Some(Command::SendMessage { content: sent, .. }) if sent == content),
			"/{name}: {:?}",
			outcome.commands.first()
		);
		assert!(key_counted(&outcome, key), "/{name} counts as {key}");
	}
	running.stop().await;
}
