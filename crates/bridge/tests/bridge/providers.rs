use bridge::{
	ChannelKind, FRAME_ORIGINS, FrameCall, FrameRect, Played, PlayerLayout, Provider, ProviderFrames,
	ProviderPlayer, Sight, StandInFrames,
};
use mockcord::providers::{LINKS, SOUNDCLOUD_LINK, SPOTIFY_LINK, VIMEO_LINK, YOUTUBE_LINK};
use protocol::{Embed, EmbedMedia, Snowflake};

use crate::harness::Running;

async fn opened(running: &mut Running) -> Snowflake {
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("the first guild has a text channel");
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	channel_id
}

async fn unfurled(running: &mut Running, channel_id: Snowflake, link: &str) -> Embed {
	let message_id = running
		.server
		.post_message(channel_id, link)
		.expect("mockcord posts the link");
	running
		.until("the link to reach the timeline", |view| {
			view.timeline.messages().any(|entry| entry.id == message_id)
		})
		.await;
	running
		.view
		.timeline
		.messages()
		.find(|entry| entry.id == message_id)
		.and_then(|entry| entry.message.embeds.first().cloned())
		.expect("the link arrives with its embed")
}

fn player(src: &str) -> ProviderPlayer {
	ProviderPlayer {
		provider: Provider::YouTube,
		layout: PlayerLayout::Video,
		src: src.to_owned(),
		size: (400., 225.),
		link: None,
	}
}

fn rect(x: i32, y: i32) -> FrameRect {
	FrameRect {
		x,
		y,
		width: 600,
		height: 338,
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_links_become_their_players_embed_urls() {
	let mut running = Running::start("provider-embeds").await;
	let channel_id = opened(&mut running).await;

	let mut players = Vec::new();
	for link in LINKS {
		let embed = unfurled(&mut running, channel_id, link).await;
		players.push((link, ProviderPlayer::of(&embed)));
	}
	let found = |wanted: &str| {
		players
			.iter()
			.find(|(link, _)| *link == wanted)
			.and_then(|(_, player)| player.clone())
			.unwrap_or_else(|| panic!("{wanted} has a player"))
	};

	let youtube = found(YOUTUBE_LINK);
	assert_eq!(youtube.provider, Provider::YouTube);
	assert_eq!(youtube.layout, PlayerLayout::Video);
	assert_eq!(
		youtube.src,
		"https://www.youtube.com/embed/jNQXAC9IVRw?autoplay=1&auto_play=1&start=5"
	);
	assert_eq!(youtube.size, (400., 225.), "Discord's 400 wide 16:9 box");
	assert_eq!(youtube.link.as_deref(), Some(YOUTUBE_LINK));

	let vimeo = found(VIMEO_LINK);
	assert_eq!(vimeo.provider, Provider::Vimeo);
	assert_eq!(vimeo.src, "https://player.vimeo.com/video/76979871?autoplay=1");
	assert_eq!(vimeo.size, (400., 225.));

	let spotify = found(SPOTIFY_LINK);
	assert_eq!(spotify.provider, Provider::Spotify);
	assert_eq!(spotify.layout, PlayerLayout::Widget);
	assert_eq!(
		spotify.src,
		"https://open.spotify.com/embed/track/4cOdK2wGLETKBW3PvgPWqT?autoplay=1"
	);
	assert_eq!(spotify.size, (400., 80.));

	let soundcloud = found(SOUNDCLOUD_LINK);
	assert_eq!(soundcloud.provider, Provider::SoundCloud);
	assert_eq!(soundcloud.layout, PlayerLayout::Widget);
	assert_eq!(
		soundcloud.src,
		"https://w.soundcloud.com/player/?url=https%3A%2F%2Fsoundcloud.com%2Fforss%2Fflickermood&auto_play=true&visual=false",
		"the API track URL in Discord's player link is not trusted; the page link is rebuilt"
	);
	assert_eq!(soundcloud.size, (400., 166.));

	let plain = unfurled(
		&mut running,
		channel_id,
		"https://example.com/watch?v=jNQXAC9IVRw",
	)
	.await;
	assert_eq!(
		ProviderPlayer::of(&plain),
		None,
		"other sites keep their link card"
	);

	running.stop().await;
}

#[test]
fn untrusted_embed_fields_never_reach_the_player() {
	let embed = |url: &str, video: Option<&str>| Embed {
		url: Some(url.to_owned()),
		video: video.map(|video| EmbedMedia {
			url: Some(video.to_owned()),
			..EmbedMedia::default()
		}),
		..Embed::default()
	};
	let refused = [
		embed("https://www.youtube.com.evil.example/watch?v=jNQXAC9IVRw", None),
		embed("http://www.youtube.com/watch?v=jNQXAC9IVRw", None),
		embed("https://user:pass@www.youtube.com/watch?v=jNQXAC9IVRw", None),
		embed("https://www.youtube.com/watch?v=jNQXAC9IVR\"><script>", None),
		embed("https://www.youtube.com/watch?v=short", None),
		embed("https://vimeo.com/123abc", None),
		embed("https://open.spotify.com/track/../../evil", None),
		embed("https://soundcloud.com/a/b/c/d", None),
		embed(
			"https://example.com/page",
			Some("https://w.soundcloud.com/player/?url=https%3A%2F%2Fevil.example%2Fx"),
		),
	];
	for embed in &refused {
		assert_eq!(ProviderPlayer::of(embed), None, "{:?} is refused", embed.url);
	}

	let short =
		ProviderPlayer::of(&embed("https://youtu.be/jNQXAC9IVRw?t=1m30s", None)).expect("a short link");
	assert_eq!(
		short.src,
		"https://www.youtube.com/embed/jNQXAC9IVRw?autoplay=1&auto_play=1&start=90"
	);
	let shorts =
		ProviderPlayer::of(&embed("https://www.youtube.com/shorts/jNQXAC9IVRw", None)).expect("a short");
	assert!(
		shorts
			.src
			.starts_with("https://www.youtube.com/embed/jNQXAC9IVRw?")
	);
	let playlist = ProviderPlayer::of(&embed(
		"https://open.spotify.com/intl-de/playlist/37i9dQZF1DXcBWIGoYBM5M",
		None,
	))
	.expect("a playlist");
	assert_eq!(playlist.size, (400., 352.));
}

#[test]
fn the_frame_document_only_frames_the_provider_origins() {
	let document = player("https://www.youtube.com/embed/jNQXAC9IVRw?a=1&b=\"x\"").document();
	let policy = format!(
		"default-src 'none'; style-src 'unsafe-inline'; frame-src {}",
		FRAME_ORIGINS.join(" ")
	);
	assert!(document.contains(&policy), "{document}");
	assert!(document.contains("src=\"https://www.youtube.com/embed/jNQXAC9IVRw?a=1&amp;b=&quot;x&quot;\""));
	assert!(
		!document.contains("allow-top-navigation"),
		"the frame can't leave the page"
	);
	assert!(!document.contains("<script"), "the page itself runs no script");
	assert!(bridge::frame_allows("https://kyoko.localhost/frame"));
	assert!(!bridge::frame_allows(
		"https://www.youtube.com/watch?v=jNQXAC9IVRw"
	));
	assert!(!bridge::frame_allows("https://kyoko.localhost.evil.example/"));
}

#[test]
fn one_player_lives_at_a_time_and_follows_its_card() {
	let host = StandInFrames::new(true);
	let frames = ProviderFrames::new(Box::new(host.clone()));
	assert!(host.calls().is_empty(), "nothing is made before play");

	assert_eq!(
		frames.play("embed-1-0", &player("https://www.youtube.com/embed/aaaaaaaaaaa")),
		Played::Opened
	);
	assert_eq!(frames.live().as_deref(), Some("embed-1-0"));
	frames.follow(Sight::Shown {
		rect: rect(10, 20),
		clip: rect(10, 20),
	});
	frames.follow(Sight::Shown {
		rect: rect(10, 20),
		clip: rect(10, 20),
	});
	frames.follow(Sight::Hidden);
	frames.follow(Sight::Hidden);
	let partly = FrameRect {
		height: 100,
		..rect(10, 0)
	};
	frames.follow(Sight::Shown {
		rect: rect(10, -238),
		clip: partly,
	});
	assert_eq!(host.placement(), Some((rect(10, -238), partly)));

	assert_eq!(
		frames.play("embed-2-0", &player("https://www.youtube.com/embed/bbbbbbbbbbb")),
		Played::Opened
	);
	assert_eq!(host.open_frames(), 1, "a second player closes the first");
	frames.follow(Sight::Gone);
	assert_eq!(frames.live(), None, "scrolling far away destroys it");
	assert!(!frames.close());

	let calls = host.calls();
	assert!(matches!(calls[0], FrameCall::Open { id: 1, .. }));
	assert_eq!(
		calls[1..],
		[
			FrameCall::Place {
				id: 1,
				rect: rect(10, 20),
				clip: rect(10, 20)
			},
			FrameCall::Hide { id: 1 },
			FrameCall::Place {
				id: 1,
				rect: rect(10, -238),
				clip: partly
			},
			FrameCall::Close { id: 1 },
			calls[5].clone(),
			FrameCall::Close { id: 2 },
		]
	);
	assert!(matches!(&calls[5], FrameCall::Open { id: 2, document } if document.contains("bbbbbbbbbbb")));
}

#[test]
fn without_webview2_play_opens_the_browser() {
	let host = StandInFrames::new(false);
	let frames = ProviderFrames::new(Box::new(host.clone()));
	assert!(!frames.available());
	assert_eq!(
		frames.play("embed-1-0", &player("https://www.youtube.com/embed/aaaaaaaaaaa")),
		Played::Browser
	);
	assert_eq!(frames.live(), None);
	assert!(host.calls().is_empty());
}
