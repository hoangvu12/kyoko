use bridge::{
	ChannelKind, ChoiceList, Composer, EmojiPicker, FileEdit, Mark, NITRO_LOCK, Notice, Visual, text_marks,
	too_big,
};
use chrono::Utc;
use mockcord::{MockConfig, Reply as MockReply};
use std::time::Duration;

use protocol::{ChannelType, CommandOptionType, Snowflake};
use rest::UploadFile;
use serde_json::json;
use session::Command;
use state::{AutocompleteStatus, RiskyReason, RiskyTarget, State};

use crate::harness::Running;

const ECHO_ID: Snowflake = Snowflake(9_100_000_000_000_000_102);
const ROLL_ID: Snowflake = Snowflake(9_100_000_000_000_000_103);

async fn opened(name: &str) -> (Running, Composer, Snowflake) {
	opened_with(name, MockConfig::default()).await
}

async fn opened_with(name: &str, config: MockConfig) -> (Running, Composer, Snowflake) {
	let mut running = Running::start_with(name, config).await;
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
async fn locked_emoji_are_listed_last_and_never_inserted() {
	let config = MockConfig {
		nitro: false,
		autocomplete_emoji: true,
		..MockConfig::default()
	};
	let (mut running, mut composer, _) = opened_with("composer-locked-emoji", config).await;
	running
		.until_state("the user without Nitro", |state| {
			state.current_user().is_some() && !state.has_nitro()
		})
		.await;
	let text = "wow :sm";
	typed(&mut composer, &running, text);
	assert_eq!(composer.heading(), Some(("Emoji matching", ":sm".to_owned())));
	let suggestions = composer.suggestions().to_vec();
	let locked = suggestions
		.iter()
		.position(|suggestion| suggestion.lock == Some(NITRO_LOCK))
		.expect("an emoji from another server is listed, locked");
	let usable = suggestions
		.iter()
		.position(|suggestion| matches!(suggestion.visual, Visual::Emoji { .. }) && suggestion.lock.is_none())
		.expect("an emoji of this server is listed, usable");
	assert!(
		suggestions[locked..]
			.iter()
			.all(|suggestion| suggestion.lock.is_some()),
		"locked rows come last: {suggestions:?}"
	);
	let name = suggestions[locked].label.trim_matches(':').to_owned();
	let picker = EmojiPicker::of(&running.session().state().read(), composer.guild());
	let cell = picker
		.search(&name, 0)
		.into_iter()
		.find(|cell| cell.label == suggestions[locked].label)
		.expect("the picker finds it too");
	assert_eq!(
		cell.lock, suggestions[locked].lock,
		"picker and autocomplete agree"
	);

	assert_eq!(
		composer.choose(text, Some(locked)),
		None,
		"a click on a locked row does nothing"
	);
	assert_eq!(composer.suggestions(), suggestions.as_slice());
	composer.move_selection(locked as i32);
	assert_eq!(composer.selected(), locked);
	assert_eq!(
		composer.choose(text, None),
		None,
		"Enter or Tab on a locked row does nothing"
	);
	assert_eq!(composer.suggestions(), suggestions.as_slice());

	let (completed, _) = composer
		.choose(text, Some(usable))
		.expect("a usable emoji completes");
	assert_eq!(completed, format!("wow {}", suggestions[usable].insert));
	assert_eq!(composer.heading(), None);

	typed(&mut composer, &running, "hi @a");
	assert_eq!(composer.heading(), Some(("Members matching", "@a".to_owned())));
	typed(&mut composer, &running, "see #");
	assert_eq!(composer.heading(), Some(("Channels matching", "#".to_owned())));
	running.stop().await;
}

fn picked(composer: &mut Composer, running: &Running, query: &str, name: &str) {
	typed(composer, running, query);
	let index = composer
		.picker()
		.and_then(|picker| picker.rows.iter().position(|row| row.name() == name))
		.unwrap_or_else(|| panic!("/{name} is listed for {query}"));
	assert_eq!(composer.choose("", Some(index)), Some((String::new(), 0)));
	assert_eq!(
		composer.command().map(|command| command.entry.name()),
		Some(name.to_owned())
	);
	assert!(composer.picker().is_none());
}

fn hint(composer: &Composer) -> (String, String, bool) {
	let hint = composer.command().expect("a command is open").hint();
	(hint.name, hint.description, hint.error)
}

fn sections(composer: &Composer) -> Vec<(String, Vec<String>)> {
	let picker = composer.picker().expect("the picker is open");
	picker
		.sections
		.iter()
		.map(|section| {
			let rows = picker.rows[section.first..section.first + section.count]
				.iter()
				.map(|row| row.name())
				.collect();
			(section.title.clone(), rows)
		})
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_picker_lists_frequent_commands_then_each_app_then_built_ins() {
	let config = MockConfig {
		frequent_commands: true,
		..MockConfig::default()
	};
	let (mut running, mut composer, channel_id) = opened_with("composer-picker", config).await;
	with_commands(&mut running, &mut composer, channel_id).await;
	running
		.until_state("the frequently used commands", |state| {
			state.command_uses().len() == 3
		})
		.await;
	typed(&mut composer, &running, "/");
	let listed = sections(&composer);
	let titles: Vec<&str> = listed.iter().map(|(title, _)| title.as_str()).collect();
	assert_eq!(
		titles,
		[
			"Frequently Used",
			"Mockbot",
			"Remindy",
			"Polly",
			"Snapper",
			"Built-In"
		]
	);
	assert_eq!(listed[0].1, ["remind", "roll", "shrug"]);
	assert_eq!(
		listed[1].1,
		[
			"broken",
			"echo",
			"ignore",
			"ping",
			"roll",
			"think",
			"translate",
			"whisper"
		]
	);
	let built_in = &listed[5].1;
	for name in [
		"gif",
		"me",
		"msg",
		"schedule",
		"shrug",
		"spoiler",
		"sticker",
		"tableflip",
		"unflip",
	] {
		assert!(
			built_in.iter().any(|row| row == name),
			"/{name} is built in: {built_in:?}"
		);
	}
	let mut sorted = built_in.clone();
	sorted.sort();
	assert_eq!(*built_in, sorted, "built-ins are alphabetical");
	let picker = composer.picker().expect("open");
	let remind = &picker.rows[0];
	assert_eq!(remind.app.name, "Remindy");
	let chips: Vec<&str> = remind.required().map(|option| option.name.as_str()).collect();
	assert_eq!(chips, ["when", "message"]);

	composer.move_selection(-1);
	assert_eq!(
		composer.picker().map(|picker| picker.selected),
		Some(picker_len(&composer) - 1)
	);
	assert!(composer.choose_section(1));
	assert_eq!(
		composer
			.picker()
			.and_then(|picker| picker.selected_entry())
			.map(|row| row.name()),
		Some("broken".to_owned())
	);

	typed(&mut composer, &running, "/rol");
	let listed = sections(&composer);
	assert_eq!(listed, [("Mockbot".to_owned(), vec!["roll".to_owned()])]);
	typed(&mut composer, &running, "/ro");
	assert_eq!(
		sections(&composer)[0].1,
		["roll", "broken"],
		"prefix matches come first"
	);
	assert!(composer.dismiss());
	assert!(composer.picker().is_none());
	running.stop().await;
}

fn choice_names(composer: &Composer, running: &Running) -> Option<Vec<String>> {
	let state = running.session().state().clone();
	let choices = composer.option_choices(&state.read())?;
	match choices.list {
		ChoiceList::Choices(list) => Some(list.into_iter().map(|choice| choice.name).collect()),
		ChoiceList::Loading => Some(vec!["loading".to_owned()]),
		ChoiceList::Failed => Some(vec!["failed".to_owned()]),
	}
}

fn pick_choice(composer: &mut Composer, running: &Running) -> bool {
	let state = running.session().state().clone();
	composer.pick_choice(&state.read(), None)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choices_filter_as_typed_autocomplete_asks_once_per_value_and_files_ride_along() {
	let (mut running, mut composer, channel_id) = opened("composer-choices").await;
	with_commands(&mut running, &mut composer, channel_id).await;

	picked(&mut composer, &running, "/ec", "echo");
	assert!(composer.edit_option(0, "hello"));
	assert_eq!(choice_names(&composer, &running), None, "text has no choices");
	assert!(composer.step_option(1));
	let all = choice_names(&composer, &running).expect("the language choices");
	assert_eq!(
		all,
		["English", "Deutsch", "Español", "Français", "日本語", "Português"]
	);
	let state = running.session().state().clone();
	assert_eq!(
		composer
			.option_choices(&state.read())
			.map(|choices| choices.title),
		Some("LANGUAGE".to_owned())
	);
	assert!(composer.edit_option(1, "is"));
	assert_eq!(
		choice_names(&composer, &running),
		Some(vec!["English".to_owned(), "Français".to_owned()])
	);
	assert!(composer.move_choice(&state.read(), 1));
	assert!(pick_choice(&mut composer, &running));
	let command = composer.command().expect("open");
	assert_eq!(command.fields[1].value, "Français");
	let outcome = submitted(&mut composer, &running, "");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::RunApplicationCommand { options, files, .. })
			if options[1].value == Some(json!("fr")) && files.is_empty()
	));

	picked(&mut composer, &running, "/tra", "translate");
	assert!(
		composer.autocomplete_request(false).is_none(),
		"text does not autocomplete"
	);
	assert!(composer.edit_option(0, "hi"));
	assert!(composer.step_option(1));
	let asked = composer
		.autocomplete_request(false)
		.expect("the focused option asks");
	let Command::AutocompleteCommand {
		channel_id: asked_channel,
		command_id,
		options,
		request: first,
	} = &asked
	else {
		panic!("expected an autocomplete, got {asked:?}");
	};
	assert_eq!(*asked_channel, channel_id);
	assert_eq!(*command_id, Snowflake(mockcord::TRANSLATE_COMMAND));
	assert_eq!(options.len(), 2);
	assert_eq!(
		(options[0].value.clone(), options[0].focused),
		(Some(json!("hi")), None)
	);
	assert_eq!(
		(options[1].value.clone(), options[1].focused),
		(Some(json!("")), Some(true))
	);
	assert!(
		composer.autocomplete_request(false).is_none(),
		"the same value is asked once"
	);
	assert_eq!(
		choice_names(&composer, &running),
		Some(vec!["loading".to_owned()])
	);
	assert!(composer.edit_option(1, "deu"));
	let asked = composer
		.autocomplete_request(false)
		.expect("a new value asks again");
	let Command::AutocompleteCommand { request, .. } = &asked else {
		panic!("expected an autocomplete");
	};
	assert!(request > first);
	let request = *request;
	running.session().send(asked);
	running
		.until_state("the autocomplete answer", |state| {
			state.autocomplete().is_some_and(|autocomplete| {
				autocomplete.request == request
					&& matches!(autocomplete.status, AutocompleteStatus::Loaded(_))
			})
		})
		.await;
	assert_eq!(
		choice_names(&composer, &running),
		Some(vec!["Deutsch".to_owned()])
	);
	assert!(pick_choice(&mut composer, &running));
	let outcome = submitted(&mut composer, &running, "");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::RunApplicationCommand { options, .. })
			if options[1].value == Some(json!("de")) && options[1].focused.is_none()
	));

	picked(&mut composer, &running, "/res", "resize");
	assert!(composer.wants_file());
	assert!(submitted(&mut composer, &running, "").commands.is_empty());
	assert_eq!(
		hint(&composer),
		("file".to_owned(), "Pick a file to upload".to_owned(), true)
	);
	let id = composer
		.attach_option(
			&state.read(),
			UploadFile::new("cat.png", vec![1u8; 64], "image/png"),
		)
		.expect("the file is taken");
	assert!(!composer.wants_file());
	assert_eq!(composer.option_file_ids(), [id]);
	assert_eq!(composer.command().map(|command| command.focused), Some(1));
	assert!(composer.edit_option(1, "200"));
	let outcome = submitted(&mut composer, &running, "");
	match outcome.commands.first() {
		Some(Command::RunApplicationCommand {
			command_id,
			options,
			files,
			..
		}) => {
			assert_eq!(*command_id, Snowflake(mockcord::RESIZE_COMMAND));
			assert_eq!(
				(options[0].kind, options[0].value.clone()),
				(CommandOptionType::Attachment, Some(json!(0)))
			);
			assert_eq!(options[1].value, Some(json!(200)));
			assert_eq!(files.len(), 1);
			assert_eq!(files[0].name, "cat.png");
		}
		other => panic!("expected the resize to run, got {other:?}"),
	}
	assert!(composer.command().is_none());
	assert!(composer.option_file_ids().is_empty());
	running.stop().await;
}

fn picker_len(composer: &Composer) -> usize {
	composer.picker().map_or(0, |picker| picker.rows.len())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_options_are_checked_before_sending_and_the_reason_shows_in_the_hint() {
	let (mut running, mut composer, channel_id) = opened("composer-options").await;
	with_commands(&mut running, &mut composer, channel_id).await;
	picked(&mut composer, &running, "/rol", "roll");
	assert_eq!(
		hint(&composer),
		("sides".to_owned(), "How many sides the die has".to_owned(), false)
	);
	let command = composer.command().expect("open");
	assert_eq!(command.fields.len(), 1);
	assert_eq!(
		command
			.optional()
			.iter()
			.map(|option| option.name.as_str())
			.collect::<Vec<_>>(),
		["count"]
	);

	let outcome = submitted(&mut composer, &running, "");
	assert!(outcome.commands.is_empty(), "nothing is sent without sides");
	assert!(outcome.replace.is_none());
	assert_eq!(
		hint(&composer),
		("sides".to_owned(), "This option is required".to_owned(), true)
	);

	assert!(composer.edit_option(0, "1000"));
	assert_eq!(
		hint(&composer),
		(
			"sides".to_owned(),
			"Must be a number between 1 and 100".to_owned(),
			true
		)
	);
	assert!(
		composer
			.command()
			.is_some_and(|command| command.fields[0].error.is_some())
	);
	assert!(submitted(&mut composer, &running, "").commands.is_empty());
	assert!(composer.edit_option(0, "2.5"));
	assert!(hint(&composer).2, "an integer option refuses a fraction");
	assert!(composer.edit_option(0, "20"));
	assert_eq!(
		hint(&composer),
		("sides".to_owned(), "How many sides the die has".to_owned(), false)
	);

	assert!(composer.toggle_optional());
	assert!(composer.command().is_some_and(|command| command.optional_open));
	assert!(composer.add_optional(None));
	let command = composer.command().expect("open");
	assert!(!command.optional_open);
	assert_eq!(command.focused, 1);
	assert_eq!(hint(&composer).0, "count");
	assert!(composer.edit_option(1, "3"));

	let outcome = submitted(&mut composer, &running, "");
	match outcome.commands.as_slice() {
		[
			Command::RunApplicationCommand {
				command_id, options, ..
			},
			Command::CountCommandUse { key },
			..,
		] => {
			assert_eq!(*command_id, ROLL_ID);
			assert_eq!(*key, ROLL_ID.to_string());
			assert_eq!(options.len(), 2);
			assert_eq!(
				(options[0].name.as_str(), options[0].kind),
				("sides", CommandOptionType::Integer)
			);
			assert_eq!(options[0].value, Some(json!(20)));
			assert_eq!(options[1].value, Some(json!(3)));
		}
		other => panic!("expected the roll to run, got {other:?}"),
	}
	assert_eq!(outcome.replace, Some((String::new(), 0)));
	assert!(composer.command().is_none());

	picked(&mut composer, &running, "/ec", "echo");
	assert!(composer.edit_option(0, "hi there"));
	assert!(composer.step_option(1));
	assert!(composer.edit_option(1, "German"));
	assert_eq!(
		hint(&composer),
		(
			"language".to_owned(),
			"Choose one of English, Deutsch, Español, Français, 日本語, Português".to_owned(),
			true
		)
	);
	assert!(composer.edit_option(1, "fr"));
	let outcome = submitted(&mut composer, &running, "");
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::RunApplicationCommand { command_id, options, .. })
			if *command_id == ECHO_ID
				&& options[0].value == Some(json!("hi there"))
				&& options[1].value == Some(json!("fr"))
	));

	picked(&mut composer, &running, "/shr", "shrug");
	assert!(
		composer
			.command()
			.is_some_and(|command| command.fields.is_empty())
	);
	let outcome = submitted(&mut composer, &running, "");
	assert!(matches!(
		outcome.commands.as_slice(),
		[Command::SendMessage { content, .. }, Command::CountCommandUse { key }, ..]
			if content == "¯\\\\\\_(ツ)\\_/¯" && key == "-1"
	));

	picked(&mut composer, &running, "/stick", "sticker");
	assert!(submitted(&mut composer, &running, "").commands.is_empty());
	assert_eq!(
		hint(&composer),
		(
			"/sticker".to_owned(),
			"kyoko can't run /sticker yet".to_owned(),
			true
		)
	);
	assert_eq!(composer.leave_command(), Some(("/sticker".to_owned(), 8)));
	running.stop().await;
}

#[test]
fn option_values_parse_and_validate_by_kind() {
	use bridge::option_value;
	use protocol::CommandOption;
	let option = |kind: CommandOptionType| CommandOption {
		kind,
		name: "value".to_owned(),
		..CommandOption::default()
	};
	assert_eq!(
		option_value(&option(CommandOptionType::Integer), " 12 "),
		Ok(json!(12))
	);
	assert_eq!(
		option_value(&option(CommandOptionType::Integer), "twelve"),
		Err("Must be a whole number".to_owned())
	);
	assert_eq!(
		option_value(&option(CommandOptionType::Number), "2.5"),
		Ok(json!(2.5))
	);
	let ranged = CommandOption {
		min_value: Some(0.5),
		..option(CommandOptionType::Number)
	};
	assert_eq!(
		option_value(&ranged, "0.1"),
		Err("Must be at least 0.5".to_owned())
	);
	assert_eq!(
		option_value(&option(CommandOptionType::Boolean), "yes"),
		Ok(json!(true))
	);
	assert_eq!(
		option_value(&option(CommandOptionType::User), "<@!42>"),
		Ok(json!("42"))
	);
	assert_eq!(
		option_value(&option(CommandOptionType::Channel), "<#7>"),
		Ok(json!("7"))
	);
	assert_eq!(
		option_value(&option(CommandOptionType::Role), "<@&9>"),
		Ok(json!("9"))
	);
	assert!(option_value(&option(CommandOptionType::Role), "<@9>").is_err());
	let short = CommandOption {
		min_length: Some(2),
		max_length: Some(4),
		..option(CommandOptionType::String)
	};
	assert_eq!(
		option_value(&short, "abcdef"),
		Err("Must be between 2 and 4 characters".to_owned())
	);
	assert_eq!(option_value(&short, "abc"), Ok(json!("abc")));
	assert!(option_value(&option(CommandOptionType::Attachment), "file").is_err());
}

#[test]
fn subcommands_are_listed_as_their_own_rows_and_nest_their_options() {
	use bridge::{CommandSource, interaction_options};
	let index: protocol::ApplicationCommandIndex = serde_json::from_value(json!({
		"applications": [{ "id": "5", "name": "Configo" }],
		"application_commands": [{
			"id": "50", "type": 1, "application_id": "5", "name": "config", "description": "Settings",
			"options": [
				{ "type": 2, "name": "role", "description": "Roles", "options": [
					{ "type": 1, "name": "set", "description": "Pick the role", "options": [
						{ "type": 8, "name": "role", "description": "Which role", "required": true },
					]},
				]},
				{ "type": 1, "name": "reset", "description": "Start over" },
				{ "type": 99, "name": "future", "description": "Unknown option kind" },
			],
		}],
	}))
	.expect("the index decodes");
	let mut state = State::new();
	state.apply(&state::Event::Local(state::LocalEvent::CommandIndexLoaded {
		scope: Snowflake(1),
		index: std::sync::Arc::new(index),
	}));
	let catalog = bridge::command_catalog(&state, None, Snowflake(1));
	let names: Vec<String> = catalog
		.iter()
		.filter(|entry| entry.app.name == "Configo")
		.map(bridge::CommandEntry::name)
		.collect();
	assert_eq!(names, ["config reset", "config role set"]);
	let set = catalog
		.iter()
		.find(|entry| entry.name() == "config role set")
		.expect("the leaf");
	assert_eq!(set.key, "50\u{0}role set");
	assert!(
		matches!(set.source, CommandSource::Application { command_id, .. } if command_id == Snowflake(50))
	);
	let role = set.options[0].clone();
	let nested = interaction_options(set, vec![(role, json!("9"))]);
	let payload = serde_json::to_value(&nested).expect("serialises");
	assert_eq!(
		payload,
		json!([{ "type": 2, "name": "role", "options": [
			{ "type": 1, "name": "set", "options": [{ "type": 8, "name": "role", "value": "9" }] },
		]}])
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frequently_used_commands_are_saved_to_the_account_and_come_back() {
	let (mut running, mut composer, channel_id) = opened("composer-frequent").await;
	with_commands(&mut running, &mut composer, channel_id).await;
	running
		.until_state("the account's frecency settings", State::favorites_loaded)
		.await;
	typed(&mut composer, &running, "/");
	assert!(
		composer
			.picker()
			.is_some_and(|picker| picker.sections.iter().all(|section| section.app.is_some())),
		"nothing is frequently used yet"
	);
	picked(&mut composer, &running, "/rol", "roll");
	assert!(composer.edit_option(0, "6"));
	let outcome = submitted(&mut composer, &running, "");
	for command in outcome.commands {
		running.session().send(command);
	}
	running
		.until_state("the use counted", |state| {
			state
				.command_uses()
				.iter()
				.any(|used| used.key == ROLL_ID.to_string() && used.total_uses == 1)
		})
		.await;
	let deadline = std::time::Instant::now() + Duration::from_secs(10);
	while !running
		.server
		.control()
		.command_uses()
		.iter()
		.any(|used| used.key == ROLL_ID.to_string() && used.total_uses == 1 && used.recent_uses.len() == 1)
	{
		assert!(
			std::time::Instant::now() < deadline,
			"the use reaches the account's settings"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}

	let _ = std::fs::remove_dir_all(running.scratch.join("accounts"));
	running.restart().await;
	let guild_id = running.view.sidebar.selected();
	if let Some(command) = running.open_channel(channel_id) {
		running.session().send(command);
	}
	let mut composer = Composer::default();
	composer.open(&running.session().state().read(), guild_id, Some(channel_id));
	with_commands(&mut running, &mut composer, channel_id).await;
	running
		.until_state("the uses from the account", |state| {
			!state.command_uses().is_empty()
		})
		.await;
	typed(&mut composer, &running, "/");
	let listed = sections(&composer);
	assert_eq!(listed[0], ("Frequently Used".to_owned(), vec!["roll".to_owned()]));
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
	let small = UploadFile::new("small.png", vec![1u8; 16], "image/png");
	let id = composer.attach(&state.read(), small).expect("a small file fits");
	assert!(limit >= 10 * 1024 * 1024);
	for index in 0..9 {
		let file = UploadFile::new(format!("{index}.txt"), vec![2u8; 8], "text/plain");
		composer.attach(&state.read(), file).expect("room for ten");
	}
	let eleventh = UploadFile::new("eleventh.txt", vec![3u8; 8], "text/plain");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shift_on_drop_uploads_the_files_directly_and_leaves_the_draft() {
	let (running, mut composer, channel_id) = opened("composer-drop-direct").await;
	let state = running.session().state().clone();
	let (message_id, _) = other_author(&state.read(), channel_id);
	assert!(composer.start_reply(&state.read(), message_id));
	typed(&mut composer, &running, "still writing");
	let queued = UploadFile::new("queued.txt", vec![1u8; 8], "text/plain");
	composer.attach(&state.read(), queued).expect("a queued file");
	let dropped = vec![
		UploadFile::new("one.png", vec![2u8; 8], "image/png"),
		UploadFile::new("two.png", vec![3u8; 8], "image/png"),
	];
	let outcome = composer.upload_now(&state.read(), dropped, Utc::now());
	assert!(outcome.replace.is_none());
	match outcome.commands.as_slice() {
		[
			Command::SendMessage {
				channel_id: sent_to,
				content,
				reply_to: None,
				files,
				..
			},
		] => {
			assert_eq!(*sent_to, channel_id);
			assert!(content.is_empty());
			let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
			assert_eq!(names, ["one.png", "two.png"]);
		}
		other => panic!("expected one direct upload, got {other:?}"),
	}
	assert_eq!(composer.files().len(), 1);
	assert!(composer.reply().is_some());
	let eleven = (0..11)
		.map(|index| UploadFile::new(format!("{index}.txt"), vec![4u8; 8], "text/plain"))
		.collect();
	assert!(
		composer
			.upload_now(&state.read(), eleven, Utc::now())
			.commands
			.is_empty()
	);
	assert_eq!(
		composer.notice(&state.read(), Utc::now()),
		Some(Notice::Refused("You can attach up to 10 files".to_owned()))
	);
	assert_eq!(
		too_big("cat.mov", 512 * 1024 * 1024, 10 * 1024 * 1024).as_deref(),
		Some("cat.mov is too big (512 MB). The limit here is 10 MB.")
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_spoilered_and_edited_file_is_sent_as_edited() {
	let (running, mut composer, _) = opened("composer-file-edit").await;
	let state = running.session().state().clone();
	let photo = UploadFile::new("IMG_0001.png", vec![1u8; 8], "image/png");
	let id = composer.attach(&state.read(), photo).expect("attached");
	assert!(composer.toggle_spoiler(id));
	assert!(
		composer
			.file(id)
			.is_some_and(|attachment| attachment.file.spoiler)
	);
	assert!(composer.edit_file(
		id,
		FileEdit {
			name: " cat.png ".to_owned(),
			description: "a sleepy cat".to_owned(),
			spoiler: true,
		},
	));
	let outcome = submitted(&mut composer, &running, "");
	match outcome.commands.first() {
		Some(Command::SendMessage { files, .. }) => {
			assert_eq!(files[0].name, "cat.png");
			assert_eq!(files[0].upload_name(), "SPOILER_cat.png");
			assert_eq!(files[0].description.as_deref(), Some("a sleepy cat"));
		}
		other => panic!("expected a send, got {other:?}"),
	}
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn markdown_in_the_field_is_marked_but_sent_exactly_as_typed() {
	let (running, mut composer, channel_id) = opened("composer-markdown").await;
	let text = "**Midnight** is *almost* ready\n> thanks <@42>\n`code` and ||secret|| <:uwu:7>";
	let marks = text_marks(text);
	let marked = |mark: Mark| -> Vec<&str> {
		marks
			.iter()
			.filter(|found| found.mark == mark)
			.map(|found| &text[found.range.clone()])
			.collect()
	};
	assert_eq!(
		marked(Mark::Syntax),
		["**", "**", "*", "*", ">", "`", "`", "||", "||", "<:", ":7>"]
	);
	assert_eq!(marked(Mark::Mention), ["<@42>"]);
	assert_eq!(marked(Mark::Code), ["code"]);
	assert_eq!(marked(Mark::Spoiler), ["secret"]);
	typed(&mut composer, &running, text);
	let outcome = submitted(&mut composer, &running, text);
	assert!(matches!(
		outcome.commands.first(),
		Some(Command::SendMessage { content, channel_id: sent_to, .. }) if content == text && *sent_to == channel_id
	));
	running.stop().await;
}
