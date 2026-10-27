use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bridge::{EmojiPicker, Inbox, StickerPicker};
use media::{Gallery, Limits, MediaService, Update};
use mockcord::{MockConfig, Server};
use protocol::fingerprint::Fingerprint;
use protocol::{Snowflake, Token};
use session::{Session, SessionConfig};
use state::State;
use store::{Paths, Settings, Store};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::input::{InputEvent, Modifiers, PointerButton};
use zgx_script::ScriptHost;
use zgx_script::testing::TestApp;

use super::{ExpressionPicker, Favorite, Mode, Tab};
use crate::timeline::Pictures;

type Made = Rc<RefCell<Option<(Rc<ExpressionPicker>, Rc<Pictures>)>>>;

const WAIT: Duration = Duration::from_secs(30);
const WINDOW: (f32, f32) = (1200., 900.);
const ANCHOR: (f32, f32) = (1100., 860.);
const FIRST_ROWS: usize = 3;
const MOST_EMOJI_FETCHES: usize = 90;
const MOST_STICKER_FETCHES: usize = 40;

fn scratch(name: &str) -> std::path::PathBuf {
	let root = std::env::temp_dir().join(format!("kyoko-{name}-{}", std::process::id()));
	let _ = std::fs::remove_dir_all(&root);
	root
}

fn open_session(server: &Server, root: &std::path::Path) -> Session {
	let paths = Paths::at(root.join("session")).expect("scratch paths");
	let mut settings = Settings::load(&paths);
	settings.storage.plaintext_token = true;
	settings.save(&paths).expect("settings save");
	let store = Store::open(paths).expect("store opens");
	Session::start(SessionConfig {
		identity: session::client_identity(&store),
		build_source: None,
		store: Arc::new(store),
		rest_base: url::Url::parse(server.rest_base_url()).expect("rest url"),
		gateway_url: server.gateway_url().to_owned(),
		token: Token::new(server.token()),
		runtime: None,
		notify: bridge::changes_notifier(Inbox::new()),
		hooks: None,
		gateway: Default::default(),
		rest: Default::default(),
		caches: Default::default(),
		alerts: Default::default(),
		retention: Default::default(),
		remember_account: false,
		theme_links: Default::default(),
		trust: Default::default(),
	})
	.expect("session starts")
}

fn until(session: &Session, what: &str, ready: impl Fn(&State) -> bool) {
	let deadline = Instant::now() + WAIT;
	while !ready(&session.state().read()) {
		assert!(Instant::now() < deadline, "timed out waiting for {what}");
		std::thread::sleep(Duration::from_millis(20));
	}
}

#[derive(Debug, Clone, PartialEq)]
enum Picked {
	Emoji(String),
	Sticker(Snowflake),
	Gif(String),
	Favorite(Favorite),
}

struct Rig {
	app: TestApp,
	picker: Rc<ExpressionPicker>,
	pictures: Rc<Pictures>,
	gallery: Gallery,
	session: Session,
	guild_id: Snowflake,
	channel_id: Snowflake,
	picked: Rc<RefCell<Vec<Picked>>>,
	_server: Server,
	_tokio: tokio::runtime::Runtime,
}

fn rig(config: MockConfig, name: &str) -> Rig {
	let tokio = tokio::runtime::Builder::new_multi_thread()
		.worker_threads(6)
		.enable_all()
		.build()
		.expect("a tokio runtime");
	let guilds = config.guilds;
	let server = tokio.block_on(Server::start(config)).expect("mockcord starts");
	let root = scratch(name);
	let session = {
		let _entered = tokio.enter();
		open_session(&server, &root)
	};
	let guild_id = Snowflake(server.fixtures().guilds[0].id);
	until(&session, "every server's emoji and stickers", |state| {
		state.emoji_catalog(Some(guild_id)).guilds.len() == guilds
			&& state
				.guilds()
				.all(|guild| state.guild_stickers(guild.id).is_some())
	});
	session.send(session::Command::LoadStickerPacks);
	until(&session, "the sticker packs", |state| {
		state.sticker_packs().is_some()
	});
	let channel_id = {
		let state = session.state().read();
		state
			.guild(guild_id)
			.and_then(|guild| guild.channels.keys().next().copied())
			.expect("a channel")
	};
	let fingerprint = Fingerprint::web_client("en-US", 1);
	let fetcher =
		bridge::media_fetcher(&fingerprint, Some(server.address().port())).expect("a media fetcher");
	let service = MediaService::new(fetcher, root.join("cache"), Limits::default());
	let gallery = Gallery::new(service, tokio.handle().clone());
	let origin = format!("http://{}", server.address());
	let cdn = server.cdn_url();
	let made: Made = Rc::default();
	let (into, shown) = (made.clone(), made.clone());
	let pictures_gallery = gallery.clone();
	let app = TestApp::wrapped(
		move |runtime| {
			crate::theme::Catalog::load(&[
				std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes")
			])
			.install(runtime, crate::theme::DEFAULT);
			let pictures = Rc::new(Pictures::new(
				runtime.clone(),
				pictures_gallery,
				cdn,
				Some(origin),
			));
			let picker = ExpressionPicker::new(runtime, pictures.clone(), Mode::Composer);
			*into.borrow_mut() = Some((picker, pictures));
			ScriptHost::inline(runtime, "app", "return ui.column {}")
		},
		move |_| {
			let picker = shown.borrow().as_ref().map(|(picker, _)| picker.clone());
			div()
				.relative()
				.w_full()
				.h_full()
				.when_some(picker, |root, picker| root.child(picker.view()))
		},
	);
	app.ui.scene.borrow_mut().resize(WINDOW.0, WINDOW.1);
	app.ui.prepare_frame();
	let (picker, pictures) = made.borrow().clone().expect("the picker is built");
	let picked: Rc<RefCell<Vec<Picked>>> = Rc::default();
	let heard = picked.clone();
	picker.on_emoji(move |cell| heard.borrow_mut().push(Picked::Emoji(cell.key)));
	let heard = picked.clone();
	picker.on_sticker(move |cell| heard.borrow_mut().push(Picked::Sticker(cell.sticker.id)));
	let heard = picked.clone();
	picker.on_gif(move |gif| heard.borrow_mut().push(Picked::Gif(gif.id)));
	let heard = picked.clone();
	picker.on_favorite(move |favorite| heard.borrow_mut().push(Picked::Favorite(favorite)));
	Rig {
		app,
		picker,
		pictures,
		gallery,
		session,
		guild_id,
		channel_id,
		picked,
		_server: server,
		_tokio: tokio,
	}
}

impl Rig {
	fn models(&self) -> (EmojiPicker, StickerPicker) {
		let state = self.session.state().read();
		(
			EmojiPicker::of(&state, Some(self.guild_id)),
			StickerPicker::of(&state, Some(self.guild_id), self.channel_id),
		)
	}

	fn pump(&self) {
		let names = self.picker.images();
		self.gallery
			.set_visible(names.iter().map(media::ImageName::as_str));
		for update in self.gallery.drain(Instant::now()) {
			match update {
				Update::Show { name, pixels } => {
					let image = ImageData::new(pixels.width, pixels.height, pixels.rgba)
						.ok()
						.map(Arc::new);
					self.pictures.show(&name, image);
				}
				Update::Clear { name } => self.pictures.show(&name, None),
			}
		}
	}
}

impl Rig {
	fn open_on(&mut self, tab: Tab) {
		let (emoji, stickers) = self.models();
		self.picker.set_emoji(emoji);
		self.picker.set_stickers(stickers);
		self.picker.open(tab, ANCHOR);
		self.app.frames(30);
	}

	fn press(&mut self, id: &str, button: PointerButton, modifiers: Modifiers) {
		let bounds = self.app.bounds(id);
		let (x, y) = (bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.);
		for event in [
			InputEvent::PointerMove { x, y },
			InputEvent::PointerDown { x, y, button },
			InputEvent::PointerUp { x, y, button },
		] {
			self.app.ui.dispatch_with_modifiers(event, modifiers);
		}
		self.app.frames(2);
	}

	fn click(&mut self, id: &str) {
		self.press(id, PointerButton::Primary, Modifiers::default());
	}

	fn right_click(&mut self, id: &str) {
		self.press(id, PointerButton::Secondary, Modifiers::default());
	}

	fn shift_click(&mut self, id: &str) {
		self.press(
			id,
			PointerButton::Primary,
			Modifiers {
				shift: true,
				..Modifiers::default()
			},
		);
	}

	fn shows(&mut self, id: &str) -> bool {
		self.app
			.find(id)
			.is_some_and(|node| self.app.ui.scene.borrow().layout_visible(node))
	}

	fn picked(&self) -> Vec<Picked> {
		self.picked.borrow_mut().drain(..).collect()
	}
}

fn usable_emoji(rig: &Rig) -> bridge::EmojiCell {
	rig.models()
		.0
		.sections(0)
		.into_iter()
		.flat_map(|section| section.cells)
		.find(|cell| cell.custom.is_some() && !cell.locked)
		.expect("a usable custom emoji")
}

#[test]
fn a_left_click_on_an_emoji_picks_it_and_a_right_click_offers_favorite() {
	let mut rig = rig(MockConfig::default(), "picker-emoji-buttons");
	let cell = usable_emoji(&rig);
	let id = format!("picker-emoji-{}", cell.key);
	rig.open_on(Tab::Emoji);
	rig.right_click(&id);
	assert!(rig.picked().is_empty(), "a right click picks nothing");
	assert!(rig.shows("picker-favorite"), "a right click opens the menu");
	assert!(rig.picker.is_open(), "the picker stays open under its menu");
	let (shown, menu) = (rig.app.bounds(&id), rig.app.bounds("picker-favorite"));
	assert!(
		menu.y >= shown.y + shown.height - 1. && (menu.x - shown.x).abs() < shown.width,
		"the menu opens under the cell: {menu:?} for {shown:?}"
	);
	rig.click("picker-favorite");
	assert_eq!(
		rig.picked(),
		vec![Picked::Favorite(Favorite::Emoji(
			cell.favorite_entry.clone(),
			!cell.favorite
		))]
	);
	rig.click(&id);
	assert_eq!(rig.picked(), vec![Picked::Emoji(cell.key.clone())]);
	assert!(!rig.picker.is_open(), "a pick closes the picker");
	rig.open_on(Tab::Emoji);
	rig.shift_click(&id);
	assert_eq!(rig.picked(), vec![Picked::Emoji(cell.key.clone())]);
	assert!(rig.picker.is_open(), "shift keeps the picker open");
	rig.click(&id);
	assert_eq!(rig.picked(), vec![Picked::Emoji(cell.key)]);
	assert!(!rig.picker.is_open());
}

#[test]
fn a_left_click_on_a_sticker_sends_it_and_a_right_click_does_not() {
	let mut rig = rig(MockConfig::default(), "picker-sticker-buttons");
	let sticker = rig
		.models()
		.1
		.sections()
		.iter()
		.flat_map(|section| section.stickers.iter())
		.find(|cell| !cell.locked)
		.map(|cell| cell.sticker.id)
		.expect("a usable sticker");
	let id = format!("picker-sticker-{sticker}");
	rig.open_on(Tab::Sticker);
	rig.right_click(&id);
	assert!(rig.picked().is_empty(), "a right click sends nothing");
	assert!(rig.picker.is_open());
	rig.click(&id);
	assert_eq!(rig.picked(), vec![Picked::Sticker(sticker)]);
	assert!(!rig.picker.is_open(), "a pick closes the picker");
	rig.open_on(Tab::Sticker);
	rig.shift_click(&id);
	assert_eq!(rig.picked(), vec![Picked::Sticker(sticker)]);
	assert!(rig.picker.is_open(), "shift keeps the picker open");
}

#[test]
fn a_left_click_on_a_gif_sends_it_and_a_right_click_offers_favorite() {
	let mut rig = rig(MockConfig::default(), "picker-gif-buttons");
	let gifs: Vec<protocol::Gif> = (0..2)
		.map(|index| {
			serde_json::from_value(serde_json::json!({
				"id": format!("gif-{index}"),
				"url": format!("https://tenor.com/view/wave-gif-{index}"),
				"src": format!("http://{}/media/wave-{index}.gif", rig._server.address()),
				"gif_src": format!("http://{}/media/wave-{index}.gif", rig._server.address()),
				"preview": format!("http://{}/media/wave-{index}-preview.png", rig._server.address()),
				"width": 400,
				"height": 300,
			}))
			.expect("a gif")
		})
		.collect();
	rig.picker.set_gifs(bridge::GifView::Results {
		query: "wave".to_owned(),
		browsing: false,
		gifs: Some(gifs.clone()),
		favorites: Vec::new(),
	});
	rig.open_on(Tab::Gif);
	let id = format!("picker-gif-{}", gifs[0].id);
	rig.right_click(&id);
	assert!(rig.picked().is_empty(), "a right click sends nothing");
	rig.click("picker-favorite");
	assert_eq!(
		rig.picked(),
		vec![Picked::Favorite(Favorite::Gif(
			bridge::favorite_of_gif(&gifs[0]),
			true
		))]
	);
	rig.click(&id);
	assert_eq!(rig.picked(), vec![Picked::Gif(gifs[0].id.clone())]);
	assert!(!rig.picker.is_open(), "a pick closes the picker");
}

struct Timing {
	first_frame: Duration,
	first_images: Duration,
	fetched: usize,
}

struct Wanted {
	cell: String,
	images: Vec<media::ImageName>,
}

type Pick = fn(&Rig, &EmojiPicker, &StickerPicker) -> Wanted;

fn timed_open(rig: &mut Rig, tab: Tab, first: Pick) -> Timing {
	let entries = rig.gallery.stats().entries;
	let started = Instant::now();
	let (emoji, stickers) = rig.models();
	rig.picker.set_emoji(emoji.clone());
	rig.picker.set_stickers(stickers.clone());
	rig.picker.open(tab, ANCHOR);
	rig.app.frame();
	let first_frame = started.elapsed();
	let wanted = first(rig, &emoji, &stickers);
	assert!(
		rig.app.find(&wanted.cell).is_some(),
		"{} shows on the first frame after opening",
		wanted.cell
	);
	let deadline = Instant::now() + WAIT;
	while !wanted
		.images
		.iter()
		.all(|name| rig.pictures.picture(name).with_untracked(Option::is_some))
	{
		assert!(Instant::now() < deadline, "the first rows' images arrive");
		rig.pump();
		rig.app.frame();
		std::thread::sleep(Duration::from_millis(2));
	}
	Timing {
		first_frame,
		first_images: started.elapsed(),
		fetched: rig.gallery.stats().entries - entries,
	}
}

fn first_emoji(rig: &Rig, emoji: &EmojiPicker, _: &StickerPicker) -> Wanted {
	let cells: Vec<bridge::EmojiCell> = emoji
		.sections(0)
		.into_iter()
		.flat_map(|section| section.cells)
		.take(FIRST_ROWS * 9)
		.collect();
	Wanted {
		cell: format!("picker-emoji-{}", cells[0].key),
		images: cells
			.iter()
			.filter_map(|cell| cell.custom.as_ref())
			.map(|custom| rig.picker.cell_image(custom))
			.collect(),
	}
}

fn first_sticker(rig: &Rig, _: &EmojiPicker, stickers: &StickerPicker) -> Wanted {
	let cells: Vec<&bridge::StickerCell> = stickers
		.sections()
		.iter()
		.flat_map(|section| section.stickers.iter())
		.take(FIRST_ROWS * 4)
		.collect();
	Wanted {
		cell: format!("picker-sticker-{}", cells[0].sticker.id),
		images: cells
			.iter()
			.filter_map(|cell| rig.picker.sticker_image(cell))
			.collect(),
	}
}

#[test]
fn a_crowded_picker_shows_its_first_rows_on_the_first_frame_and_fetches_only_what_shows() {
	let mut rig = rig(MockConfig::crowded(), "picker-crowded");
	let (emoji, stickers) = rig.models();
	let custom = emoji
		.sections(0)
		.iter()
		.flat_map(|section| section.cells.iter())
		.filter(|cell| cell.custom.is_some())
		.count();
	let sticker_count = stickers
		.sections()
		.iter()
		.map(|section| section.stickers.len())
		.sum::<usize>();
	assert!(custom >= 3000, "{custom} custom emoji");
	assert!(sticker_count >= 300, "{sticker_count} stickers");
	for (label, tab, first, most) in [
		("emoji", Tab::Emoji, first_emoji as Pick, MOST_EMOJI_FETCHES),
		("emoji again", Tab::Emoji, first_emoji, 0),
		("stickers", Tab::Sticker, first_sticker, MOST_STICKER_FETCHES),
		("stickers again", Tab::Sticker, first_sticker, 0),
	] {
		let timing = timed_open(&mut rig, tab, first);
		eprintln!(
			"{label}: first frame {:?}, first rows' images {:?}, {} fetched",
			timing.first_frame, timing.first_images, timing.fetched
		);
		assert!(
			timing.fetched <= most,
			"{label} fetched {} images, more than the {most} it shows",
			timing.fetched
		);
		rig.picker.close();
		rig.app.frame();
	}
}
