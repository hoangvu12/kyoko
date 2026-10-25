use std::time::Duration;

use mockcord::{Reply, RestRequest};
use session::{Command, RestTuning};

use crate::harness::{Options, Running};

const MESSAGES: &str = "/channels/*/messages";

fn unpaced() -> RestTuning {
	RestTuning {
		mutation_gap: Duration::ZERO,
		send_gap: Duration::ZERO,
		channel_send_gap: Duration::ZERO,
		..RestTuning::default()
	}
}

fn query_keys(request: &RestRequest) -> Vec<String> {
	request
		.query
		.as_deref()
		.unwrap_or_default()
		.split('&')
		.filter_map(|pair| pair.split_once('=').map(|(key, _)| key.to_owned()))
		.collect()
}

fn param<'a>(request: &'a RestRequest, name: &str) -> Option<&'a str> {
	request.query.as_deref()?.split('&').find_map(|pair| {
		let (key, value) = pair.split_once('=')?;
		(key == name).then_some(value)
	})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gif_search_asks_discord_the_way_the_web_client_does() {
	let running = Running::start("expressions-gif-search").await;
	running.connected().await;
	running.session().send(Command::LoadGifTrending);
	running.session().send(Command::SuggestGifs {
		query: "ca".to_owned(),
	});
	running.session().send(Command::SearchGifs {
		query: "cat".to_owned(),
	});
	running
		.until("trending, suggestions and results", |state| {
			state.gif_trending().is_some()
				&& state.gif_suggestions("ca").is_some()
				&& state.gif_search("cat").is_some_and(|gifs| !gifs.is_empty())
		})
		.await;
	let recording = running.mock().recording();
	let search = recording
		.requests_to("GET", "/gifs/search")
		.next()
		.expect("a gif search");
	assert_eq!(query_keys(search), ["q", "media_format", "provider", "locale"]);
	assert_eq!(param(search, "q"), Some("cat"));
	assert_eq!(param(search, "media_format"), Some("mp4"));
	assert_eq!(param(search, "provider"), Some("tenor"));
	let trending = recording
		.requests_to("GET", "/gifs/trending")
		.next()
		.expect("trending gifs");
	assert_eq!(param(trending, "media_format"), Some("mp4"));
	assert_eq!(param(trending, "provider"), Some("tenor"));
	let suggest = recording
		.requests_to("GET", "/gifs/suggest")
		.next()
		.expect("gif suggestions");
	assert_eq!(query_keys(suggest), ["q", "provider", "limit", "locale"]);
	assert_eq!(param(suggest, "limit"), Some("5"));
	assert!(
		recording
			.requests_to("GET", "/gifs/*")
			.all(|request| request.header("authorization").is_some()),
		"gif routes are Discord's API and carry the session like the web client"
	);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn picking_a_gif_sends_its_link_through_the_outbox() {
	let running = Running::start_with(Options::new("expressions-gif-send").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	running.session().send(Command::SearchGifs {
		query: "wave".to_owned(),
	});
	running
		.until("the search results", |state| {
			state.gif_search("wave").is_some_and(|gifs| !gifs.is_empty())
		})
		.await;
	let gif = running
		.session()
		.state()
		.read()
		.gif_search("wave")
		.expect("results")[0]
		.clone();
	running.session().send(Command::SendGif {
		channel_id,
		gif: gif.clone(),
	});
	let body = running
		.until_wire("the gif message", |recording| {
			recording
				.requests_to("POST", MESSAGES)
				.filter_map(RestRequest::json)
				.find(|body| body["content"].as_str() == Some(gif.url.as_str()))
		})
		.await;
	assert!(
		body["nonce"].is_string(),
		"sends carry a nonce like typed messages"
	);
	running
		.until("the gif to be remembered", |state| {
			state
				.recent_gifs()
				.first()
				.is_some_and(|recent| recent.id == gif.id)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_picked_sticker_is_sent_with_its_id_through_the_outbox() {
	let running = Running::start_with(Options::new("expressions-sticker-send").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	let sticker = {
		let state = running.session().state().read();
		let guild_id = state.channel_guild(channel_id).expect("a guild channel");
		state
			.guild_stickers(guild_id)
			.and_then(|stickers| stickers.first().cloned())
			.expect("READY carries the server's stickers")
	};
	running.session().send(Command::SendSticker {
		channel_id,
		sticker_id: sticker.id,
	});
	let body = running
		.until_wire("the sticker message", |recording| {
			recording
				.requests_to("POST", MESSAGES)
				.filter_map(RestRequest::json)
				.find(|body| body["sticker_ids"].is_array())
		})
		.await;
	assert_eq!(body["sticker_ids"], serde_json::json!([sticker.id.to_string()]));
	assert!(body["nonce"].is_string());
	assert!(body.get("content").is_none(), "a sticker alone has no text");
	running
		.until("the sticker message in the channel", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache
					.ordered
					.iter()
					.any(|message| message.sticker_items.iter().any(|item| item.id == sticker.id))
			})
		})
		.await;
	running
		.until("the sticker to be remembered", |state| {
			state.recent_stickers().first().map(|recent| recent.id) == Some(sticker.id)
		})
		.await;
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_sticker_stays_pending_and_retries_as_a_sticker() {
	let running = Running::start_with(Options::new("expressions-sticker-retry").rest(unpaced())).await;
	let channel_id = running.open_text_channel().await;
	let sticker_id = {
		let state = running.session().state().read();
		let guild_id = state.channel_guild(channel_id).expect("a guild channel");
		state.guild_stickers(guild_id).expect("stickers")[0].id
	};
	running.mock().respond_once(
		"POST",
		MESSAGES,
		Reply::json(500, serde_json::json!({ "message": "boom" })),
	);
	running.session().send(Command::SendSticker {
		channel_id,
		sticker_id,
	});
	running
		.until("the failed sticker", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.iter().any(|pending| {
					pending.failure.is_some()
						&& pending
							.message
							.sticker_items
							.iter()
							.any(|item| item.id == sticker_id)
				})
			})
		})
		.await;
	let nonce = {
		let state = running.session().state().read();
		let cache = state.messages(channel_id).expect("the channel");
		cache
			.pending
			.iter()
			.find(|pending| pending.failure.is_some())
			.and_then(|pending| pending.message.nonce.clone())
			.expect("the failed nonce")
	};
	running
		.session()
		.send(Command::RetryMessage { channel_id, nonce });
	running
		.until_wire("the sticker sent again", |recording| {
			(recording
				.requests_to("POST", MESSAGES)
				.filter_map(RestRequest::json)
				.filter(|body| body["sticker_ids"] == serde_json::json!([sticker_id.to_string()]))
				.count() >= 2)
				.then_some(())
		})
		.await;
	running
		.until("the sticker message in the channel", |state| {
			state.messages(channel_id).is_some_and(|cache| {
				cache.pending.is_empty()
					&& cache
						.ordered
						.iter()
						.any(|message| message.sticker_items.iter().any(|item| item.id == sticker_id))
			})
		})
		.await;
	running.stop().await;
}
