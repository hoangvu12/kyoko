use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use media::ImageName;
use zgx::compose::NodeHandle;
use zgx::compose::prelude::*;
use zgx::scene::{Color, NodeId, NodeKind, Rect, Scene};
use zgx::svg::SvgData;

use super::pictures::Pictures;

pub enum Art {
	Image(ImageName),
	Svg(Arc<SvgData>),
}

pub struct Placed {
	pub range: Range<usize>,
	pub art: Art,
}

const COPY_TEXT_SIZE: f32 = 1.;
const COPY_TEXT_TRACKING: f32 = -0.5;

pub fn placeholder(copied: TextSpan, room: f32) -> TextSpan {
	copied
		.text_size(COPY_TEXT_SIZE)
		.letter_spacing(COPY_TEXT_TRACKING)
		.text_color(Color(0, 0, 0, 0))
		.padding(room, 0.)
}

pub struct Paragraph {
	pub spans: Vec<TextSpan>,
	pub placed: Vec<Placed>,
	pub side: f32,
}

pub fn paragraph(paragraph: Paragraph, pictures: &Rc<Pictures>) -> View {
	let Paragraph { spans, placed, side } = paragraph;
	let text = View::from(rich_text().children(spans)).w_full().text_wrap(true);
	if placed.is_empty() {
		return text;
	}
	let rects = pictures.runtime().signal(Vec::<Rect>::new());
	let ranges: Rc<[Range<usize>]> = placed.iter().map(|item| item.range.clone()).collect();
	let images = placed.into_iter().enumerate().map(|(index, item)| {
		let rects = rects.clone();
		let at = move || match rects.with(|rects| rects.get(index).copied()) {
			Some(rect) => Styles::new()
				.left(rect.x + (rect.width - side) / 2.)
				.top(rect.y + (rect.height - side) / 2.)
				.visible(),
			None => Styles::new().invisible(),
		};
		let frame = div().absolute().size(side, side);
		match item.art {
			Art::Image(name) => {
				let picture = pictures.picture(&name);
				frame.reactive_style(move || match picture.get() {
					Some(image) => at().bg_image(image, ObjectFit::Contain),
					None => at(),
				})
			}
			Art::Svg(data) => frame
				.reactive_style(at)
				.child(svg("emoji", data).size(side, side)),
		}
	});
	div()
		.relative()
		.w_full()
		.child(text)
		.children(images)
		.on_mount(move |handle| track(handle, ranges, rects))
}

fn track(handle: NodeHandle, ranges: Rc<[Range<usize>]>, rects: zgx::reactive::Signal<Vec<Rect>>) {
	let measure = {
		let (handle, ranges, rects) = (handle.clone(), ranges.clone(), rects.clone());
		move || {
			if let Some(found) = place(&handle, &ranges)
				&& rects.with_untracked(|current| *current != found)
			{
				rects.set(found);
			}
		}
	};
	let first = measure.clone();
	handle.after_layout(first);
	handle.on_layout(measure);
}

fn place(handle: &NodeHandle, ranges: &[Range<usize>]) -> Option<Vec<Rect>> {
	let scene = handle.scene()?;
	let scene = scene.borrow();
	let owner = handle.node();
	let paragraph = rich_node(&scene, owner)?;
	let NodeKind::RichText { text } = scene.kind(paragraph) else {
		return None;
	};
	let bounds = scene.bounds(paragraph);
	let origin = scene.bounds(owner);
	let layout = scene.shape_rich_text(text, Some(bounds.width));
	let dx = bounds.x - origin.x;
	let dy = bounds.y - origin.y;
	Some(
		ranges
			.iter()
			.map(|range| {
				layout
					.selection(range.clone())
					.into_iter()
					.next()
					.map(|rect| Rect::new(rect.x + dx, rect.y + dy, rect.width, rect.height))
					.unwrap_or_default()
			})
			.collect(),
	)
}

fn rich_node(scene: &Scene, from: NodeId) -> Option<NodeId> {
	let mut pending = vec![from];
	while let Some(node) = pending.pop() {
		if matches!(scene.kind(node), NodeKind::RichText { .. }) {
			return Some(node);
		}
		pending.extend(scene.children(node).iter().rev().copied());
	}
	None
}
