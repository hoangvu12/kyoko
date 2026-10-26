use std::sync::Arc;
use std::time::Duration;

use bridge::{ChannelKind, Composer};
use plugins::{EnableError, HostConfig, Limits, PluginHost};
use session::Command;

use crate::harness::Running;

const PLUGIN: &str = "favorite-emoji-first";
const QUERY: &str = ":wa";

fn host(name: &str) -> (PluginHost, std::path::PathBuf) {
	let root = std::env::temp_dir().join(format!("kyoko-bridge-plugins-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	let host = PluginHost::new(HostConfig {
		data_dir: Some(root.join("data")),
		user_dir: None,
		development_dirs: Vec::new(),
		bundled: true,
		limits: Limits {
			load_deadline: Duration::from_secs(15),
			..Limits::default()
		},
	});
	(host, root)
}

fn labels(composer: &Composer) -> Vec<String> {
	composer
		.suggestions()
		.iter()
		.map(|suggestion| suggestion.label.clone())
		.collect()
}

fn rank_requests(commands: &[Command]) -> usize {
	commands
		.iter()
		.filter(|command| matches!(command, Command::RankEmoji { .. }))
		.count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_emoji_autocomplete_lists_favourites_first_once_favorite_emoji_first_runs() {
	let (host, root) = host("favourites");
	let mut running = Running::start_hooked("favourites", Arc::new(host.clone())).await;
	host.attach(running.session().state().clone(), running.session().commander());
	let Err(EnableError::NeedsApproval(asked)) = host.enable(PLUGIN) else {
		panic!("the plugin asks for approval");
	};
	host.approve(PLUGIN, &asked).expect("approved");
	let first = protocol::Snowflake(running.server.fixtures().guilds[0].id);
	if let Some(command) = running.select_guild(first) {
		running.session().send(command);
	}
	let guild_id = running.view.sidebar.selected();
	assert_eq!(guild_id, Some(first));
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
	let state = running.session().state().clone();
	let mut composer = Composer::default();
	composer.open(&state.read(), guild_id, Some(channel_id));
	let commands = composer.changed(&state.read(), QUERY, QUERY.len());
	assert_eq!(rank_requests(&commands), 1);
	let before = labels(&composer);
	assert!(!before.is_empty(), "suggestions show at once, in kyoko's order");
	assert_ne!(before[0], ":blob_wave:");
	for command in commands {
		running.session().send(command);
	}
	running
		.until_state("the favourites", |state| state.favorites_loaded())
		.await;
	if let Some(command) = composer.rank_request(&state.read()) {
		running.session().send(command);
	}
	running
		.until_state("the ranking", |state| {
			state.emoji_ranking(guild_id, "wa").is_some()
		})
		.await;
	assert!(composer.refresh_suggestions(&state.read()));
	let after = labels(&composer);
	assert_eq!(after[0], ":blob_wave:", "{after:?}");
	assert_eq!(after.len(), before.len());
	assert!(
		composer.rank_request(&state.read()).is_none(),
		"one request per query and favourites"
	);
	let again = composer.changed(&state.read(), ":wa ", 4);
	assert_eq!(rank_requests(&again), 0, "no request once the query is gone");
	host.disable(PLUGIN);
	running.stop().await;
	let _ = std::fs::remove_dir_all(&root);
}
