use std::time::{Duration, Instant};

use bridge::{
	ChannelKind, Composer, EmojiCell, EmojiPicker, GIF_SEARCH_DELAY, GifSearch, GifView, PickerSection,
	StickerPicker, StickerSectionKey,
};
use chrono::Utc;
use protocol::Snowflake;
use session::Command;

use crate::harness::Running;

async fn opened(name: &'static str) -> (Running, Composer, Option<Snowflake>, Snowflake) {
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
	(running, composer, guild_id, channel_id)
}

async fn drop_nitro(running: &mut Running) {
	running.server.control().transform_ready(|_, ready| {
		ready["user"]["premium_type"] = serde_json::json!(0);
	});
	running.restart().await;
	running
		.until_state("the session without Nitro", |state| {
			state.current_user().is_some() && !state.has_nitro()
		})
		.await;
}

fn custom_cells(picker: &EmojiPicker, locked: bool) -> Vec<EmojiCell> {
	picker
		.rail()
		.into_iter()
		.filter(|entry| matches!(entry.section, PickerSection::Guild(_)))
		.flat_map(|entry| picker.section(entry.section, 0).1)
		.filter(|cell| cell.custom.is_some() && cell.locked == locked)
		.collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn picking_an_emoji_inserts_its_markup_at_the_caret() {
	let (mut running, mut composer, guild_id, _) = opened("expressions-emoji").await;
	let picker = EmojiPicker::of(&running.session().state().read(), guild_id);
	let smile = picker
		.search("grinning", 0)
		.into_iter()
		.find(|cell| cell.glyph.is_some())
		.expect("a unicode emoji");
	let glyph = smile.glyph.clone().expect("a glyph");
	let (text, caret) = composer
		.pick_emoji("hi there", 2, &smile)
		.expect("a unicode emoji always inserts");
	assert_eq!(text, format!("hi{glyph} there"));
	assert_eq!(caret, 2 + glyph.len());

	let own = custom_cells(&picker, false)
		.into_iter()
		.next()
		.expect("an emoji of the open server");
	let custom = own.custom.clone().expect("a custom emoji");
	let name = own.label.trim_matches(':');
	let expected = if custom.animated {
		format!("<a:{name}:{}>", custom.id)
	} else {
		format!("<:{name}:{}>", custom.id)
	};
	assert_eq!(own.markup, expected);
	let (text, caret) = composer.pick_emoji(&text, caret, &own).expect("usable here");
	assert_eq!(text, format!("hi{glyph}{expected} there"));
	assert_eq!(caret, 2 + glyph.len() + expected.len());

	let (text, _) = composer
		.pick_emoji("caf\u{e9}", 4, &smile)
		.expect("inserts beside a multi-byte letter");
	assert_eq!(
		text,
		format!("caf{glyph}\u{e9}"),
		"a caret inside a letter moves before it"
	);

	assert!(
		custom_cells(&picker, true).is_empty(),
		"with Nitro every server's emoji is usable"
	);
	drop_nitro(&mut running).await;
	let picker = EmojiPicker::of(&running.session().state().read(), guild_id);
	let foreign = custom_cells(&picker, true)
		.into_iter()
		.next()
		.expect("another server's emoji is locked without Nitro");
	assert_eq!(composer.pick_emoji("hi", 2, &foreign), None);
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gif_search_waits_for_typing_to_pause() {
	let (mut running, _, _, _) = opened("expressions-gif-debounce").await;
	let shared = running.session().state().clone();
	let mut search = GifSearch::default();
	let opened = search.open(&shared.read());
	assert!(matches!(
		opened.as_slice(),
		[Command::LoadGifTrending, Command::LoadFavorites]
	));
	let start = Instant::now();
	assert!(
		search.typed(&shared.read(), "w", start).is_empty(),
		"one letter asks nothing"
	);
	let suggested = search.typed(&shared.read(), "Wa", start + Duration::from_millis(80));
	assert!(matches!(suggested.as_slice(), [Command::SuggestGifs { query }] if query == "wa"));
	let typed_at = start + Duration::from_millis(160);
	search.typed(&shared.read(), "wav", typed_at);
	assert!(
		search
			.poll(&shared.read(), typed_at + GIF_SEARCH_DELAY / 2)
			.is_empty(),
		"no search while the user is still typing"
	);
	let searched = search.poll(&shared.read(), typed_at + GIF_SEARCH_DELAY);
	assert!(matches!(searched.as_slice(), [Command::SearchGifs { query }] if query == "wav"));
	assert!(
		search
			.poll(&shared.read(), typed_at + GIF_SEARCH_DELAY * 2)
			.is_empty()
	);
	assert_eq!(
		search.view(&shared.read()),
		GifView::Results {
			query: "wav".to_owned(),
			gifs: None
		}
	);
	for command in searched {
		running.session().send(command);
	}
	running
		.until_state("the gif results", |state| state.gif_search("wav").is_some())
		.await;
	assert!(matches!(
		search.view(&shared.read()),
		GifView::Results { gifs: Some(gifs), .. } if !gifs.is_empty()
	));
	assert!(
		search.submit(&shared.read(), "wav").is_empty(),
		"a known search is not asked twice"
	);
	search.typed(&shared.read(), "", Instant::now());
	assert!(matches!(search.view(&shared.read()), GifView::Home { .. }));
	running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sticker_picker_lists_server_stickers_from_ready_then_packs() {
	let (mut running, mut composer, guild_id, channel_id) = opened("expressions-stickers").await;
	let shared = running.session().state().clone();
	let needs = StickerPicker::needs(&shared.read(), guild_id);
	assert!(
		matches!(needs.as_slice(), [Command::LoadStickerPacks]),
		"the open server's stickers came with READY, got {needs:?}"
	);
	for command in needs {
		running.session().send(command);
	}
	running
		.until_state("the sticker packs", |state| state.sticker_packs().is_some())
		.await;
	let picker = StickerPicker::of(&shared.read(), guild_id, channel_id);
	let first = picker.sections().first().expect("a section");
	assert_eq!(first.key, StickerSectionKey::Guild(guild_id.expect("a server")));
	assert!(first.stickers.iter().all(|cell| !cell.locked));
	assert!(
		picker
			.sections()
			.iter()
			.any(|section| matches!(section.key, StickerSectionKey::Pack(_)))
	);
	let foreign = |picker: &StickerPicker| -> Vec<bool> {
		picker
			.sections()
			.iter()
			.filter(|section| matches!(section.key, StickerSectionKey::Guild(id) if Some(id) != guild_id))
			.flat_map(|section| &section.stickers)
			.map(|cell| cell.locked)
			.collect()
	};
	assert!(!foreign(&picker).is_empty() && foreign(&picker).iter().all(|locked| !locked));
	let found = picker.search("crab");
	assert!(!found.is_empty() && found.iter().all(|cell| cell.sticker.name.contains("crab")));
	let cell = first.stickers[0].clone();
	let outcome = composer.pick_sticker(&shared.read(), &cell, Utc::now());
	assert!(matches!(
		outcome.commands.as_slice(),
		[Command::SendSticker { channel_id: sent_to, sticker_id }] if *sent_to == channel_id && *sticker_id == cell.sticker.id
	));
	assert!(outcome.replace.is_none(), "sending a sticker keeps the draft");
	drop_nitro(&mut running).await;
	let shared = running.session().state().clone();
	let picker = StickerPicker::of(&shared.read(), guild_id, channel_id);
	assert!(
		foreign(&picker).iter().all(|locked| *locked),
		"other servers' stickers need Nitro"
	);
	let locked = picker
		.sections()
		.iter()
		.flat_map(|section| &section.stickers)
		.find(|cell| cell.locked)
		.expect("a locked sticker")
		.clone();
	assert!(
		composer
			.pick_sticker(&shared.read(), &locked, Utc::now())
			.commands
			.is_empty()
	);
	running.stop().await;
}
