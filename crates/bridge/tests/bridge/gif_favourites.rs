use bridge::{FAVORITES_TITLE, GifSearch, GifView, favorite_gif};
use protocol::{Embed, EmbedKind, EmbedMedia};
use session::Command;

use crate::harness::Running;

const PAGE: &str = "https://klipy.com/gifs/cat-dance";
const VIDEO: &str = "https://static.klipy.com/cat-dance.mp4";

fn gif_embed() -> Embed {
	Embed {
		kind: Some(EmbedKind::Gifv),
		url: Some(PAGE.to_owned()),
		video: Some(EmbedMedia {
			url: Some(VIDEO.to_owned()),
			width: Some(498),
			height: Some(280),
			..EmbedMedia::default()
		}),
		..Embed::default()
	}
}

fn urls(view: &GifView) -> Vec<String> {
	match view {
		GifView::Home { favorites, .. } => favorites.iter().map(|gif| gif.url.clone()).collect(),
		GifView::Results { gifs, .. } => gifs.iter().flatten().map(|gif| gif.url.clone()).collect(),
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gif_favourited_from_a_message_appears_in_the_pickers_favourites() {
	let mut running = Running::start("gif-favourites").await;
	let favorite = favorite_gif(&gif_embed()).expect("a GIF from the GIF provider can be favourited");
	assert_eq!(favorite.url, PAGE);
	assert_eq!(favorite.src, VIDEO);
	assert!(favorite.video);
	let mut search = GifSearch::default();
	let opened = search.open(&running.session().state().read());
	assert!(
		opened
			.iter()
			.any(|command| matches!(command, Command::LoadFavorites)),
		"opening the picker reads the favourites"
	);
	running.session().send(Command::SetGifFavorite {
		gif: favorite,
		favorite: true,
	});
	running
		.until_state("the favourite in state", |state| state.is_favorite_gif(PAGE))
		.await;
	let state = running.session().state().clone();
	assert_eq!(
		urls(&search.view(&state.read())),
		[PAGE],
		"the Favorites tile shows it"
	);
	search.show_favorites();
	assert!(search.browsing(), "the Favorites tile opens with a back arrow");
	assert_eq!(search.active(), None, "favourites are not a search to share");
	let view = search.view(&state.read());
	let GifView::Results { query, favorites, .. } = &view else {
		panic!("the favourites open as a result list");
	};
	assert_eq!(query, FAVORITES_TITLE);
	assert_eq!(favorites, &[PAGE.to_owned()], "the star shows filled");
	assert_eq!(urls(&view), [PAGE]);
	running.session().send(Command::SetGifFavorite {
		gif: favorite_gif(&gif_embed()).expect("the GIF"),
		favorite: false,
	});
	running
		.until_state("the favourite to go", |state| !state.is_favorite_gif(PAGE))
		.await;
	assert!(urls(&search.view(&state.read())).is_empty());
	running.stop().await;
}
