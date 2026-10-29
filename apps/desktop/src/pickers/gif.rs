use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use bridge::{FAVORITES_TITLE, GifTile, GifView, TRENDING_TITLE, favorite_of_gif, gif_masonry};
use media::ImageName;
use protocol::{Gif, GifCategory};
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent, PointerButton};
use zgx::reactive::{Effect, Runtime, Signal};

use super::{ExpressionPicker, Favorite, INNER_WIDTH, Search};
use crate::timeline::{icon, tok};

const PADDING: f32 = 12.;
const GAP: f32 = 12.;
const COLUMN: f32 = (INNER_WIDTH - PADDING * 2. - GAP) / 2.;
const TILE_HEIGHT: f32 = 110.;
const TILE_ICON: f32 = 16.;
const HOVER_RING: f32 = 2.;
const STAR_BUTTON: f32 = 28.;
const STAR_ICON: f32 = 20.;
const STAR_INSET: f32 = 6.;
const EMPTY_ICON: f32 = 48.;
pub(super) const GIF_BATCH: usize = 25;
const REVEAL_AHEAD: f32 = TILE_HEIGHT * 2.;

type Hovered = Option<(String, Option<ImageName>)>;

pub(super) struct GifState {
	pub(super) view: Signal<Rc<GifView>>,
	hovered: Signal<Hovered>,
	scroll: Signal<f32>,
	shown: Signal<usize>,
	starred: Rc<Cell<bool>>,
	visible: RefCell<Option<Effect>>,
}

struct Seen {
	names: Vec<ImageName>,
	bottom: f32,
	total: usize,
}

fn same_list(shown: &GifView, next: &GifView) -> bool {
	match (shown, next) {
		(GifView::Home { .. }, GifView::Home { .. }) => true,
		(
			GifView::Results { query, browsing, .. },
			GifView::Results {
				query: next_query,
				browsing: next_browsing,
				..
			},
		) => query == next_query && browsing == next_browsing,
		_ => false,
	}
}

fn placed(gifs: &[Gif], count: usize) -> Vec<(GifTile, f32)> {
	gif_masonry(&gifs[..count.min(gifs.len())], COLUMN, GAP)
		.into_iter()
		.flat_map(|tiles| {
			let mut top = PADDING;
			tiles.into_iter().map(move |tile| {
				let at = top;
				top += tile.height + GAP;
				(tile, at)
			})
		})
		.collect()
}

impl GifState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			view: runtime.signal(Rc::new(GifView::Home {
				favorites: Vec::new(),
				trending: Vec::new(),
				categories: Vec::new(),
			})),
			hovered: runtime.signal(None),
			scroll: runtime.signal(0.),
			shown: runtime.signal(GIF_BATCH),
			starred: Rc::new(Cell::new(false)),
			visible: RefCell::new(None),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.scroll.set(0.);
		self.shown.set(GIF_BATCH);
	}

	pub(super) fn show(&self, view: GifView) {
		let (same, new_list) = self
			.view
			.with_untracked(|shown| (**shown == view, !same_list(shown, &view)));
		if same {
			return;
		}
		if new_list {
			self.shown.set(GIF_BATCH);
			self.scroll.set(0.);
		}
		self.view.set(Rc::new(view));
	}

	pub(super) fn watch(&self, picker: &Rc<ExpressionPicker>) {
		let weak: Weak<ExpressionPicker> = Rc::downgrade(picker);
		let (view, shown, offset, hovered) = (
			self.view.clone(),
			self.shown.clone(),
			self.scroll.clone(),
			self.hovered.clone(),
		);
		let (open, tab) = (picker.open.clone(), picker.tab.clone());
		let height = picker.body_height();
		let effect = picker.pictures.runtime().effect(move || {
			if !open.get() || tab.get() != super::Tab::Gif {
				return;
			}
			let top = offset.get();
			let (view, count) = (view.get(), shown.get());
			let moving = hovered.with(|hovered| hovered.as_ref().and_then(|(_, name)| name.clone()));
			let Some(picker) = weak.upgrade() else {
				return;
			};
			let mut seen = picker.gif_images(&view, count, (top, top + height));
			seen.names.extend(moving);
			picker.shown_images(seen.names);
			if count < seen.total && top + height + REVEAL_AHEAD >= seen.bottom {
				shown.set(count + GIF_BATCH);
			}
		});
		*self.visible.borrow_mut() = Some(effect);
	}

	pub(super) fn hover_first(&self) {
		let first = self.view.with_untracked(|view| match &**view {
			GifView::Results { gifs, .. } => gifs
				.as_ref()
				.and_then(|gifs| gif_masonry(gifs, COLUMN, GAP)[0].first().cloned()),
			GifView::Home { .. } => None,
		});
		if let Some(tile) = first {
			self.hovered.set(Some((tile.gif.id.clone(), None)));
		}
	}
}

impl ExpressionPicker {
	pub(super) fn gif_pane(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (view, shown) = (self.gif.view.clone(), self.gif.shown.clone());
		let height = self.body_height();
		column().w(INNER_WIDTH).h(height).bg(tok("bg_4")).child(switch(
			move || (view.get(), shown.get()),
			move |(view, count): (Rc<GifView>, usize), _| {
				scroll(this.gif.scroll.clone()).w(INNER_WIDTH).h(height).child(
					column()
						.w(INNER_WIDTH)
						.p(PADDING)
						.child(this.gif_content(&view, count)),
				)
			},
		))
	}

	fn gif_images(&self, view: &GifView, count: usize, (top, bottom): (f32, f32)) -> Seen {
		match view {
			GifView::Home {
				trending, categories, ..
			} => {
				Seen {
					names: trending
						.first()
						.and_then(|gif| self.pictures.gif_still(gif, (COLUMN, TILE_HEIGHT)))
						.into_iter()
						.chain(categories.iter().filter_map(|category| {
							self.pictures.gif_category(category, (COLUMN, TILE_HEIGHT))
						}))
						.collect(),
					bottom: f32::MAX,
					total: 0,
				}
			}
			GifView::Results { gifs, .. } => {
				let gifs = gifs.as_deref().unwrap_or_default();
				let tiles = placed(gifs, count);
				let names = tiles
					.iter()
					.filter(|(tile, at)| *at < bottom && at + tile.height > top)
					.filter_map(|(tile, _)| self.pictures.gif_still(&tile.gif, (COLUMN, tile.height)))
					.collect();
				let bottom = tiles
					.iter()
					.map(|(tile, at)| at + tile.height + PADDING)
					.fold(PADDING, f32::max);
				Seen {
					names,
					bottom,
					total: gifs.len(),
				}
			}
		}
	}

	fn gif_content(self: &Rc<Self>, view: &GifView, count: usize) -> View {
		match view {
			GifView::Home {
				trending, categories, ..
			} => row()
				.w(INNER_WIDTH - PADDING * 2.)
				.flex_wrap()
				.gap(GAP)
				.child(self.favorites_tile())
				.child(self.trending_tile(trending.first()))
				.children(categories.iter().map(|category| self.category_tile(category))),
			GifView::Results { gifs, favorites, .. } => {
				let empty = gifs.as_ref().is_some_and(Vec::is_empty);
				let gifs = gifs.as_deref().unwrap_or_default();
				column()
					.gap(GAP)
					.child(self.masonry(&gifs[..count.min(gifs.len())], Rc::new(favorites.clone())))
					.when(empty, |content| {
						content.child(
							column()
								.w(INNER_WIDTH - PADDING * 2.)
								.items_center()
								.gap(8.)
								.p(32.)
								.child(icon("gif", EMPTY_ICON, "text_5"))
								.child(
									text("No GIFs found. Try another search.")
										.text_size(tok("font_size_sm"))
										.text_color(tok("text_4")),
								),
						)
					})
			}
		}
	}

	fn favorites_tile(self: &Rc<Self>) -> View {
		let this = self.clone();
		self.tile(FAVORITES_TITLE, Some("picker_star"), None, true, move || {
			this.search(Search::Favorites)
		})
		.id("gif-favorites")
	}

	fn trending_tile(self: &Rc<Self>, first: Option<&Gif>) -> View {
		let still = first.and_then(|gif| self.pictures.gif_still(gif, (COLUMN, TILE_HEIGHT)));
		let this = self.clone();
		self.tile(TRENDING_TITLE, Some("picker_trending"), still, false, move || {
			this.search(Search::Trending)
		})
		.id("gif-trending")
	}

	fn category_tile(self: &Rc<Self>, category: &GifCategory) -> View {
		let still = self.pictures.gif_category(category, (COLUMN, TILE_HEIGHT));
		let (this, name) = (self.clone(), category.name.clone());
		self.tile(&category.name, None, still, false, move || {
			this.search(Search::Category(name.clone()))
		})
	}

	fn tile(
		&self,
		label: &str,
		glyph: Option<&'static str>,
		still: Option<ImageName>,
		accent: bool,
		chosen: impl FnMut() + 'static,
	) -> View {
		let picture = still.map(|name| self.pictures.picture(&name));
		let label = row()
			.gap(6.)
			.items_center()
			.when_some(glyph, |label, glyph| {
				label.child(icon(glyph, TILE_ICON, "overlay_text"))
			})
			.child(
				text(label.to_owned())
					.text_size(tok("font_size_md"))
					.line_height(20.)
					.font_weight(600)
					.text_color(tok("overlay_text")),
			);
		div()
			.relative()
			.size(COLUMN, TILE_HEIGHT)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.bg(tok(if accent { "accent_2" } else { "bg_2" }))
			.when_some(picture, |tile, picture| {
				tile.reactive_style(move || match picture.get() {
					Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
					None => Styles::new(),
				})
			})
			.hover(|style| style.border(HOVER_RING).border_color(tok("accent_2")))
			.on_click(chosen)
			.child(
				div()
					.absolute()
					.left(0.)
					.top(0.)
					.w_full()
					.h_full()
					.items_center()
					.justify_center()
					.when(!accent, |shade| shade.bg(tok("overlay_button_background")))
					.child(label),
			)
	}

	fn masonry(self: &Rc<Self>, gifs: &[Gif], favorites: Rc<Vec<String>>) -> View {
		let [left, right] = gif_masonry(gifs, COLUMN, GAP);
		let column_of = |tiles: Vec<GifTile>| {
			column()
				.w(COLUMN)
				.gap(GAP)
				.children(tiles.into_iter().map(|tile| self.gif_tile(tile, &favorites)))
		};
		row()
			.gap(GAP)
			.items_start()
			.child(column_of(left))
			.child(column_of(right))
	}

	fn gif_tile(self: &Rc<Self>, tile: GifTile, favorites: &Rc<Vec<String>>) -> View {
		let GifTile { gif, height } = tile;
		let still = self
			.pictures
			.gif_still(&gif, (COLUMN, height))
			.map(|name| self.pictures.picture(&name));
		let moving = gif.clone();
		let favorite = favorites.contains(&gif.url);
		let (hovered, styled, pictures, chooser) = (
			self.gif.hovered.clone(),
			self.gif.hovered.clone(),
			self.pictures.clone(),
			self.clone(),
		);
		let shown_pictures = self.pictures.clone();
		let (id, styled_id, star_id) = (gif.id.clone(), gif.id.clone(), gif.id.clone());
		let (starred, unstarred) = (self.gif.starred.clone(), self.gif.starred.clone());
		let star_hover = self.gif.hovered.clone();
		let star = favorite_star(self, &gif, favorite);
		let menu_favorite = Favorite::Gif(favorite_of_gif(&gif), !favorite);
		let tile = div()
			.id(format!("picker-gif-{}", gif.id))
			.relative()
			.size(COLUMN, height)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.bg(tok("bg_2"))
			.reactive_style(move || {
				let current = styled.get();
				let over = current.as_ref().filter(|(hovered, _)| *hovered == styled_id);
				let moving = over
					.and_then(|(_, name)| name.as_ref())
					.and_then(|name| shown_pictures.picture(name).get());
				let image = moving.or_else(|| still.as_ref().and_then(Signal::get));
				let styles = match image {
					Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
					None => Styles::new(),
				};
				match over {
					Some(_) => styles.border(HOVER_RING).border_color(tok("accent_2")),
					None => styles,
				}
			})
			.on_event(move |event| {
				if event.phase == EventPhase::Capture && matches!(event.event, InputEvent::PointerDown { .. })
				{
					unstarred.set(false);
				}
			})
			.on_event(move |event| match event.event {
				InputEvent::PointerEnter => {
					let name = pictures.gif_motion(&moving, (COLUMN, height));
					pictures.hover(name.as_ref());
					hovered.set(Some((id.clone(), name)));
				}
				InputEvent::PointerLeave
					if hovered
						.with_untracked(|current| current.as_ref().is_some_and(|(over, _)| *over == id)) =>
				{
					hovered.set(None);
					pictures.hover(None);
				}
				_ => {}
			})
			.on_click(move || {
				if !starred.replace(false) {
					chooser.chose_gif(gif.clone());
				}
			})
			.child(switch(
				move || {
					favorite
						|| star_hover
							.with(|hovered| hovered.as_ref().is_some_and(|(over, _)| *over == star_id))
				},
				move |shown: bool, _| if shown { star() } else { div() },
			));
		self.expression_menu(tile, (COLUMN, height), None, Some(menu_favorite))
	}
}

fn favorite_star(picker: &Rc<ExpressionPicker>, gif: &Gif, favorite: bool) -> impl Fn() -> View + 'static {
	let (picker, gif) = (picker.clone(), gif.clone());
	move || {
		let (pressed, chooser, gif) = (picker.gif.starred.clone(), picker.clone(), gif.clone());
		div()
			.id(format!("picker-gif-favorite-{}", gif.id))
			.absolute()
			.right(STAR_INSET)
			.top(STAR_INSET)
			.w(STAR_BUTTON)
			.h(STAR_BUTTON)
			.rounded(tok("radius_xs"))
			.bg(tok("overlay_button_background"))
			.items_center()
			.justify_center()
			.cursor(Cursor::Pointer)
			.on_event(move |event| {
				if event.phase != EventPhase::Capture
					&& let InputEvent::PointerDown {
						button: PointerButton::Primary,
						..
					} = event.event
				{
					pressed.set(true);
				}
			})
			.on_click(move || chooser.favorite(Favorite::Gif(favorite_of_gif(&gif), !favorite)))
			.child(if favorite {
				icon("picker_star", STAR_ICON, "yellow_2")
			} else {
				icon("picker_star_outline", STAR_ICON, "overlay_text")
			})
	}
}
