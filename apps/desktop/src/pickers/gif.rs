use std::cell::Cell;
use std::rc::Rc;

use bridge::{
	FAVORITES_TITLE, GifTile, GifView, TRENDING_TITLE, favorite_of_gif, gif_masonry, gif_motion, gif_still,
	is_video_url,
};
use media::ImageName;
use protocol::{Gif, GifCategory};
use zgx::compose::prelude::*;
use zgx::input::{EventPhase, InputEvent, PointerButton};
use zgx::reactive::{Runtime, Signal};

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

type Hovered = Option<(String, Option<ImageName>)>;

pub(super) struct GifState {
	pub(super) view: Signal<Rc<GifView>>,
	hovered: Signal<Hovered>,
	scroll: Signal<f32>,
	starred: Rc<Cell<bool>>,
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
			starred: Rc::new(Cell::new(false)),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.scroll.set(0.);
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

fn usable_still(gif: &Gif) -> Option<&str> {
	let still = gif_still(gif);
	(!still.is_empty() && !is_video_url(still)).then_some(still)
}

impl ExpressionPicker {
	pub(super) fn gif_pane(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (view, hovered) = (self.gif.view.clone(), self.gif.hovered.clone());
		let height = self.body_height();
		column().w(INNER_WIDTH).h(height).bg(tok("bg_4")).child(switch(
			move || view.get(),
			move |view: Rc<GifView>, _| {
				let content = this.gif_content(&view);
				let motion =
					hovered.with_untracked(|hovered| hovered.as_ref().and_then(|(_, name)| name.clone()));
				let mut names = this.gif_images(&view);
				names.extend(motion);
				this.shown_images(names);
				scroll(this.gif.scroll.clone())
					.w(INNER_WIDTH)
					.h(height)
					.child(column().w(INNER_WIDTH).p(PADDING).child(content))
			},
		))
	}

	fn gif_images(&self, view: &GifView) -> Vec<ImageName> {
		match view {
			GifView::Home {
				trending, categories, ..
			} => trending
				.first()
				.and_then(usable_still)
				.map(|url| self.pictures.remote(url, (COLUMN, TILE_HEIGHT)))
				.into_iter()
				.chain(
					categories
						.iter()
						.filter(|category| !is_video_url(&category.src))
						.map(|category| self.pictures.remote(&category.src, (COLUMN, TILE_HEIGHT))),
				)
				.collect(),
			GifView::Results { gifs, .. } => self.still_names(gifs.as_deref().unwrap_or_default()),
		}
	}

	fn still_names(&self, gifs: &[Gif]) -> Vec<ImageName> {
		gif_masonry(gifs, COLUMN, GAP)
			.into_iter()
			.flatten()
			.filter_map(|tile| {
				usable_still(&tile.gif).map(|still| self.pictures.remote(still, (COLUMN, tile.height)))
			})
			.collect()
	}

	fn gif_content(self: &Rc<Self>, view: &GifView) -> View {
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
				let gifs = gifs.clone().unwrap_or_default();
				column()
					.gap(GAP)
					.child(self.masonry(&gifs, Rc::new(favorites.clone())))
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
		let still = first
			.and_then(usable_still)
			.map(|url| self.pictures.remote(url, (COLUMN, TILE_HEIGHT)));
		let this = self.clone();
		self.tile(TRENDING_TITLE, Some("picker_trending"), still, false, move || {
			this.search(Search::Trending)
		})
		.id("gif-trending")
	}

	fn category_tile(self: &Rc<Self>, category: &GifCategory) -> View {
		let still = (!is_video_url(&category.src))
			.then(|| self.pictures.remote(&category.src, (COLUMN, TILE_HEIGHT)));
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
		let still = usable_still(&gif).map(|url| {
			self.pictures
				.picture(&self.pictures.remote(url, (COLUMN, height)))
		});
		let motion = gif_motion(&gif)
			.filter(|url| !is_video_url(url))
			.map(str::to_owned);
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
		let this = self.clone();
		let star_hover = self.gif.hovered.clone();
		let star = favorite_star(self, &gif, favorite);
		div()
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
					let name = motion
						.as_deref()
						.map(|url| pictures.remote(url, (COLUMN, height)));
					pictures.hover(name.as_ref());
					hovered.set(Some((id.clone(), name)));
					let names =
						this.gif.view.with_untracked(|view| {
							let mut names = this.gif_images(view);
							names.extend(this.gif.hovered.with_untracked(|hovered| {
								hovered.as_ref().and_then(|(_, name)| name.clone())
							}));
							names
						});
					this.shown_images(names);
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
			))
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
