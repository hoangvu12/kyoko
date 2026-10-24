use std::sync::Arc;
use std::time::Duration;

use mockcord::Server;
use protocol::Snowflake;

const GUILD_EVERY: u64 = 3;

pub async fn run(server: Arc<Server>, every: Duration) {
	let Some(home) = server.fixtures().guilds.first().map(|guild| Snowflake(guild.id)) else {
		return;
	};
	let mut ticker = tokio::time::interval(every);
	ticker.tick().await;
	let mut step = 0u64;
	loop {
		ticker.tick().await;
		step += 1;
		if step.is_multiple_of(GUILD_EVERY) {
			let name = format!("Live Guild {}", step / GUILD_EVERY);
			if server.create_guild(&name).is_some() {
				tracing::info!(%name, "mock server created a guild");
			}
		} else {
			let name = format!("live-{step}");
			if server.create_channel(home, &name).is_some() {
				tracing::info!(%name, "mock server created a channel");
			}
		}
	}
}
