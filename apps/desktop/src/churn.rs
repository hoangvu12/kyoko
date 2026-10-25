use std::sync::Arc;
use std::time::Duration;

use mockcord::Server;
use protocol::{ChannelType, Snowflake};
use state::SharedState;

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

pub async fn chatter(server: Arc<Server>, state: SharedState, every: Duration) {
	let mut ticker = tokio::time::interval(every);
	ticker.tick().await;
	let mut step = 0u64;
	loop {
		ticker.tick().await;
		let Some(open) = state.read().selected() else {
			continue;
		};
		step += 1;
		if server
			.post_message(open.channel_id, &format!("Live message {step}"))
			.is_some()
		{
			tracing::info!(step, "mock server posted a message into the open channel");
		}
	}
}

pub async fn pings(server: Arc<Server>, every: Duration) {
	let fixtures = server.fixtures();
	let direct = fixtures.private_channel_ids.first().copied().map(Snowflake);
	let mention = fixtures.guilds.first().and_then(|guild| {
		fixtures
			.channels
			.values()
			.find(|channel| {
				channel.guild_id == Some(Snowflake(guild.id)) && channel.kind == ChannelType::GuildText
			})
			.map(|channel| channel.id)
	});
	let me = fixtures.current_user.id;
	let mut ticker = tokio::time::interval(every);
	ticker.tick().await;
	let mut step = 0u64;
	loop {
		ticker.tick().await;
		step += 1;
		let (channel, content) = if step % 2 == 1 {
			(direct, format!("Direct ping {step}"))
		} else {
			(mention, format!("<@{}> mention ping {step}", me.0))
		};
		let Some(channel) = channel else {
			continue;
		};
		if server.post_message(channel, &content).is_some() {
			tracing::info!(step, "mock server pinged the user");
		}
	}
}
