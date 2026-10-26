use std::rc::Rc;

use bridge::{FAVORITES_TITLE, GifTile, GifView, gif_masonry, gif_motion, gif_still, is_video_url};
use media::ImageName;
use protocol::{Gif, GifCategory};
use zgx::compose::prelude::*;
use zgx::input::InputEvent;
use zgx::reactive::{Runtime, Signal};

use super::{ExpressionPicker, Search, Tab, section_title};
use crate::timeline::{icon, tok};

const PANE_PADDING: f32 = 12.;
const PANE_WIDTH: f32 = super::WIDTH - 2.;
const GAP: f32 = 4.;
const COLUMN: f32 = (super::WIDTH - PANE_PADDING * 2. - 8. - GAP) / 2.;
const CATEGORY_HEIGHT: f32 = 104.;
const CHIP_HEIGHT: f32 = 28.;
const EMPTY_ICON: f32 = 48.;

type Hovered = Option<(String, Option<ImageName>)>;

pub(super) struct GifState {
	pub(super) view: Signal<Rc<GifView>>,
	hovered: Signal<Hovered>,
	scroll: Signal<f32>,
}

impl GifState {
	pub(super) fn new(runtime: &Runtime) -> Self {
		Self {
			view: runtime.signal(Rc::new(GifView::Home {
				suggestions: Vec::new(),
				favorites: Vec::new(),
				categories: Vec::new(),
				trending: Vec::new(),
			})),
			hovered: runtime.signal(None),
			scroll: runtime.signal(0.),
		}
	}

	pub(super) fn reset(&self) {
		self.hovered.set(None);
		self.scroll.set(0.);
	}
}

impl ExpressionPicker {
	pub(super) fn gif_pane(self: &Rc<Self>) -> View {
		let this = self.clone();
		let (view, hovered) = (self.gif.view.clone(), self.gif.hovered.clone());
		column().grow().h_full().min_h(0.).child(switch(
			move || view.get(),
			move |view: Rc<GifView>, _| {
				let content = this.gif_content(&view);
				let motion =
					hovered.with_untracked(|hovered| hovered.as_ref().and_then(|(_, name)| name.clone()));
				let mut names = this.gif_images(&view);
				names.extend(motion);
				this.shown_images(names);
				column().grow().min_h(0.).child(
					scroll(this.gif.scroll.clone())
						.w(PANE_WIDTH)
						.h(super::COMPOSER_PANE_HEIGHT)
						.child(
							column()
								.w(PANE_WIDTH)
								.px(PANE_PADDING)
								.pb(PANE_PADDING)
								.child(content),
						),
				)
			},
		))
	}

	fn gif_images(&self, view: &GifView) -> Vec<ImageName> {
		match view {
			GifView::Home {
				favorites,
				categories,
				trending,
				..
			} => favorites
				.first()
				.map(gif_still)
				.filter(|url| !url.is_empty() && !is_video_url(url))
				.map(|url| self.pictures.remote(url, (COLUMN, CATEGORY_HEIGHT)))
				.into_iter()
				.chain(
					categories
						.iter()
						.filter(|category| !is_video_url(&category.src))
						.map(|category| self.pictures.remote(&category.src, (COLUMN, CATEGORY_HEIGHT))),
				)
				.chain(self.still_names(trending))
				.collect(),
			GifView::Results { gifs, .. } => self.still_names(gifs.as_deref().unwrap_or_default()),
		}
	}

	fn still_names(&self, gifs: &[Gif]) -> Vec<ImageName> {
		gif_masonry(gifs, COLUMN, GAP)
			.into_iter()
			.flatten()
			.filter(|tile| !is_video_url(gif_still(&tile.gif)))
			.map(|tile| self.pictures.remote(gif_still(&tile.gif), (COLUMN, tile.height)))
			.collect()
	}

	fn gif_content(self: &Rc<Self>, view: &GifView) -> View {
		match view {
			GifView::Home {
				suggestions,
				favorites,
				categories,
				trending,
			} => column()
				.gap(8.)
				.when(!suggestions.is_empty(), |content| {
					content.child(self.suggestion_chips(suggestions))
				})
				.child(heading("Categories"))
				.child(
					row()
						.flex_wrap()
						.gap(GAP)
						.child(self.favorites_tile(favorites.first()))
						.children(categories.iter().map(|category| self.category_tile(category))),
				)
				.child(heading("Trending GIFs"))
				.child(self.masonry(trending)),
			GifView::Results { gifs, .. } => {
				let gifs = gifs.clone().unwrap_or_default();
				let empty = gifs.is_empty();
				column()
					.gap(8.)
					.child(self.masonry(&gifs))
					.when(empty, |content| {
						content.child(
							column()
								.items_center()
								.gap(8.)
								.p(32.)
								.child(icon("gif", EMPTY_ICON, "interactive_muted"))
								.child(
									text("No GIFs found. Try another search.")
										.text_size(tok("font_sm"))
										.text_color(tok("text_muted")),
								),
						)
					})
			}
		}
	}

	fn suggestion_chips(self: &Rc<Self>, terms: &[String]) -> View {
		row()
			.flex_wrap()
			.gap(8.)
			.pt(8.)
			.children(terms.iter().map(|term| {
				let (query, this, term_query) = (self.query.clone(), self.clone(), term.to_lowercase());
				row()
					.h(CHIP_HEIGHT)
					.px(12.)
					.items_center()
					.rounded(CHIP_HEIGHT / 2.)
					.cursor(Cursor::Pointer)
					.bg(tok("background_secondary"))
					.hover(|style| style.bg(tok("modifier_selected")))
					.text_size(tok("font_sm"))
					.text_color(tok("interactive_normal"))
					.on_click(move || {
						query.set(term_query.clone());
						this.search(Search::Submitted(Tab::Gif, term_query.clone()));
					})
					.child(text(term.clone()))
			}))
	}

	fn favorites_tile(self: &Rc<Self>, first: Option<&Gif>) -> View {
		let still = first
			.map(gif_still)
			.filter(|url| !url.is_empty() && !is_video_url(url))
			.map(|url| self.pictures.remote(url, (COLUMN, CATEGORY_HEIGHT)));
		let this = self.clone();
		self.tile(FAVORITES_TITLE, still, move || this.search(Search::Favorites))
			.id("gif-favorites")
	}

	fn category_tile(self: &Rc<Self>, category: &GifCategory) -> View {
		let still = (!is_video_url(&category.src))
			.then(|| self.pictures.remote(&category.src, (COLUMN, CATEGORY_HEIGHT)));
		let (query, this, name) = (self.query.clone(), self.clone(), category.name.to_lowercase());
		self.tile(&category.name, still, move || {
			query.set(name.clone());
			this.search(Search::Submitted(Tab::Gif, name.clone()));
		})
	}

	fn tile(&self, label: &str, still: Option<ImageName>, chosen: impl FnMut() + 'static) -> View {
		let picture = still.map(|name| self.pictures.picture(&name));
		div()
			.relative()
			.size(COLUMN, CATEGORY_HEIGHT)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.bg(tok("background_secondary"))
			.when_some(picture, |tile, picture| {
				tile.reactive_style(move || match picture.get() {
					Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
					None => Styles::new(),
				})
			})
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
					.bg(tok("gif_shade"))
					.child(
						text(label.to_owned())
							.text_size(tok("font_md"))
							.font_weight(700)
							.text_color(tok("header_primary")),
					),
			)
	}

	fn masonry(self: &Rc<Self>, gifs: &[Gif]) -> View {
		let [left, right] = gif_masonry(gifs, COLUMN, GAP);
		row()
			.gap(GAP)
			.items_start()
			.child(
				column()
					.gap(GAP)
					.children(left.into_iter().map(|tile| self.gif_tile(tile))),
			)
			.child(
				column()
					.gap(GAP)
					.children(right.into_iter().map(|tile| self.gif_tile(tile))),
			)
	}

	fn gif_tile(self: &Rc<Self>, tile: GifTile) -> View {
		let GifTile { gif, height } = tile;
		let still_url = gif_still(&gif);
		let still = (!is_video_url(still_url)).then(|| {
			self.pictures
				.picture(&self.pictures.remote(still_url, (COLUMN, height)))
		});
		let motion = gif_motion(&gif)
			.filter(|url| !is_video_url(url))
			.map(str::to_owned);
		let (hovered, styled, pictures, chooser) = (
			self.gif.hovered.clone(),
			self.gif.hovered.clone(),
			self.pictures.clone(),
			self.clone(),
		);
		let shown_pictures = self.pictures.clone();
		let (id, styled_id) = (gif.id.clone(), gif.id.clone());
		let this = self.clone();
		div()
			.size(COLUMN, height)
			.shrink_0()
			.rounded(tok("radius_sm"))
			.overflow_hidden()
			.cursor(Cursor::Pointer)
			.bg(tok("background_secondary"))
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
					Some(_) => styles.border(2.).border_color(tok("brand")),
					None => styles,
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
			.on_click(move || chooser.chose_gif(gif.clone()))
	}
}

fn heading(title: &str) -> View {
	section_title(title.to_owned()).pt(8.).pb(4.)
}
