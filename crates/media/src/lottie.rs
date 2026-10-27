use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tiny_skia::{
	Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, Path, PathBuilder, Pixmap,
	Point, RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};

use crate::decode::{Limits, Plan};
use crate::error::{MediaError, Result};
use crate::types::{Animation, Decoded, DecodedImage, Frame, Replay, Streamed};

const MAX_FPS: f32 = 30.;
const ORIGINAL_SIDE: f32 = 512.;
const MAX_DEPTH: usize = 12;
const MAX_LAYERS: usize = 2000;
const MAX_VERTICES: usize = 20_000;
const BEZIER_STEPS: usize = 24;
const ROUND_KAPPA: f32 = 0.552_284_8;

pub fn looks_like(bytes: &[u8]) -> bool {
	bytes
		.iter()
		.find(|byte| !byte.is_ascii_whitespace())
		.is_some_and(|byte| *byte == b'{')
}

pub fn decode(bytes: &[u8], limits: &Limits, plan: &Plan) -> Result<Decoded> {
	let value: Value =
		serde_json::from_slice(bytes).map_err(|_| MediaError::Decode("lottie is not json".to_owned()))?;
	let scene = Arc::new(Scene::parse(&value)?);
	let (width, height) = scene.target(plan);
	if u64::from(width) * u64::from(height) > limits.max_pixels {
		return Err(MediaError::Limit(format!("{width}x{height} lottie is too large")));
	}
	let times = scene.times(limits);
	let delay_ms = scene.delay_ms();
	let mut frames: Vec<Frame> = Vec::new();
	let mut kept = 0_usize;
	let mut first: Option<DecodedImage> = None;
	for time in &times {
		let image = (plan.prepare)(scene.render(*time, width, height)?);
		if first.is_none() {
			first = Some(image.clone());
		}
		kept = kept.saturating_add(image.rgba.len());
		if kept > plan.kept_bytes {
			frames.clear();
			break;
		}
		frames.push(Frame { image, delay_ms });
	}
	let first = first.ok_or_else(|| MediaError::Decode("lottie has no frames".to_owned()))?;
	if times.len() == 1 {
		return Ok(Decoded::Still(first));
	}
	if frames.len() == times.len() {
		return Ok(Decoded::Animated(Animation { frames, loops: None }));
	}
	let prepare = plan.prepare.clone();
	let replay_scene = scene.clone();
	let replay_times = times.clone();
	Ok(Decoded::Streamed(Streamed {
		first,
		delays_ms: vec![delay_ms; times.len()],
		loops: None,
		source_bytes: bytes.len(),
		replay: Replay::new(move |sink| {
			for time in &replay_times {
				let Ok(image) = replay_scene.render(*time, width, height) else {
					return;
				};
				if !sink(prepare(image)) {
					return;
				}
			}
		}),
	}))
}

struct Scene {
	width: f32,
	height: f32,
	in_point: f32,
	out_point: f32,
	frame_rate: f32,
	layers: Vec<Layer>,
	assets: HashMap<String, Vec<Layer>>,
}

impl Scene {
	fn parse(value: &Value) -> Result<Self> {
		let width = number(value, "w");
		let height = number(value, "h");
		if !(width > 0. && height > 0. && width.is_finite() && height.is_finite()) {
			return Err(MediaError::Decode("lottie has no size".to_owned()));
		}
		let mut budget = MAX_LAYERS;
		let layers = layers(value.get("layers"), &mut budget);
		let assets = value
			.get("assets")
			.and_then(Value::as_array)
			.map(|assets| {
				assets
					.iter()
					.filter_map(|asset| {
						let id = asset.get("id").and_then(Value::as_str)?;
						let layers = asset.get("layers")?;
						Some((id.to_owned(), self::layers(Some(layers), &mut budget)))
					})
					.collect()
			})
			.unwrap_or_default();
		let frame_rate = number(value, "fr");
		Ok(Self {
			width,
			height,
			in_point: number(value, "ip"),
			out_point: number(value, "op"),
			frame_rate: if frame_rate > 0. && frame_rate.is_finite() {
				frame_rate
			} else {
				MAX_FPS
			},
			layers,
			assets,
		})
	}

	fn target(&self, plan: &Plan) -> (u32, u32) {
		let (box_w, box_h) = if plan.hint.is_original() {
			(ORIGINAL_SIDE, ORIGINAL_SIDE)
		} else {
			(plan.hint.width as f32, plan.hint.height as f32)
		};
		let scale = (box_w / self.width).min(box_h / self.height);
		(
			((self.width * scale).round() as u32).max(1),
			((self.height * scale).round() as u32).max(1),
		)
	}

	fn step(&self) -> f32 {
		(self.frame_rate / MAX_FPS).max(1.)
	}

	fn delay_ms(&self) -> u32 {
		((self.step() / self.frame_rate) * 1000.).round().max(1.) as u32
	}

	fn times(&self, limits: &Limits) -> Vec<f32> {
		let span = self.out_point - self.in_point;
		if !(span > 0. && span.is_finite()) {
			return vec![self.in_point];
		}
		let step = self.step();
		let count = ((span / step).ceil() as usize).clamp(1, limits.max_frames.max(1) as usize);
		(0..count)
			.map(|index| self.in_point + index as f32 * step)
			.collect()
	}

	fn render(&self, time: f32, width: u32, height: u32) -> Result<DecodedImage> {
		let mut pixmap =
			Pixmap::new(width, height).ok_or_else(|| MediaError::Decode("lottie canvas".to_owned()))?;
		let root = Transform::from_scale(width as f32 / self.width, height as f32 / self.height);
		self.draw_layers(&mut pixmap, &self.layers, time, root, 1., 0);
		let rgba = pixmap
			.pixels()
			.iter()
			.flat_map(|pixel| {
				let color = pixel.demultiply();
				[color.red(), color.green(), color.blue(), color.alpha()]
			})
			.collect();
		Ok(DecodedImage { width, height, rgba })
	}

	fn draw_layers(
		&self,
		pixmap: &mut Pixmap,
		layers: &[Layer],
		time: f32,
		root: Transform,
		opacity: f32,
		depth: usize,
	) {
		if depth > MAX_DEPTH {
			return;
		}
		for layer in layers.iter().rev() {
			if layer.hidden || layer.matte_source || time < layer.in_point || time >= layer.out_point {
				continue;
			}
			let world = root.pre_concat(world_of(layers, layer, time, 0));
			let opacity = opacity * layer.transform.opacity(time);
			if opacity <= 0. {
				continue;
			}
			match &layer.kind {
				LayerKind::Shapes(items) => {
					let mut ops = Vec::new();
					group(items, world, opacity, time, &mut ops);
					for op in ops.iter().rev() {
						op.draw(pixmap);
					}
				}
				LayerKind::Precomp(id) => {
					if let Some(inner) = self.assets.get(id) {
						let local = (time - layer.start) / layer.stretch;
						self.draw_layers(pixmap, inner, local, world, opacity, depth + 1);
					}
				}
				LayerKind::Solid { color, width, height } => {
					if let (Some(rect), Some(color)) =
						(Rect::from_xywh(0., 0., *width, *height), color_of(color, opacity))
					{
						let mut paint = Paint::default();
						paint.set_color(color);
						paint.anti_alias = true;
						pixmap.fill_rect(rect, &paint, world, None);
					}
				}
				LayerKind::Other => {}
			}
		}
	}
}

fn world_of(layers: &[Layer], layer: &Layer, time: f32, depth: usize) -> Transform {
	let local = layer.transform.matrix(time);
	let parent = layer
		.parent
		.filter(|_| depth < MAX_DEPTH)
		.and_then(|index| layers.iter().find(|candidate| candidate.index == Some(index)));
	match parent {
		Some(parent) => world_of(layers, parent, time, depth + 1).pre_concat(local),
		None => local,
	}
}

struct Layer {
	index: Option<i64>,
	parent: Option<i64>,
	kind: LayerKind,
	transform: Placement,
	in_point: f32,
	out_point: f32,
	start: f32,
	stretch: f32,
	hidden: bool,
	matte_source: bool,
}

enum LayerKind {
	Shapes(Vec<Item>),
	Precomp(String),
	Solid {
		color: [f32; 4],
		width: f32,
		height: f32,
	},
	Other,
}

fn layers(value: Option<&Value>, budget: &mut usize) -> Vec<Layer> {
	let Some(list) = value.and_then(Value::as_array) else {
		return Vec::new();
	};
	let mut layers = Vec::new();
	for layer in list {
		if *budget == 0 {
			break;
		}
		*budget -= 1;
		let kind = match layer.get("ty").and_then(Value::as_i64) {
			Some(0) => layer
				.get("refId")
				.and_then(Value::as_str)
				.map_or(LayerKind::Other, |id| LayerKind::Precomp(id.to_owned())),
			Some(1) => LayerKind::Solid {
				color: hex_color(layer.get("sc").and_then(Value::as_str).unwrap_or_default()),
				width: number(layer, "sw"),
				height: number(layer, "sh"),
			},
			Some(4) => {
				let mut vertices = MAX_VERTICES;
				LayerKind::Shapes(items(layer.get("shapes"), &mut vertices, 0))
			}
			_ => LayerKind::Other,
		};
		let stretch = number(layer, "sr");
		layers.push(Layer {
			index: layer.get("ind").and_then(Value::as_i64),
			parent: layer.get("parent").and_then(Value::as_i64),
			kind,
			transform: Placement::parse(layer.get("ks")),
			in_point: number(layer, "ip"),
			out_point: layer
				.get("op")
				.and_then(Value::as_f64)
				.map_or(f32::MAX, |value| value as f32),
			start: number(layer, "st"),
			stretch: if stretch > 0. && stretch.is_finite() {
				stretch
			} else {
				1.
			},
			hidden: flag(layer, "hd"),
			matte_source: layer.get("td").and_then(Value::as_i64).unwrap_or(0) != 0,
		});
	}
	layers
}

enum Item {
	Group(Vec<Item>),
	Shape(ShapeProp),
	Rect {
		position: Prop,
		size: Prop,
		round: Prop,
	},
	Ellipse {
		position: Prop,
		size: Prop,
	},
	Fill {
		look: Look,
		rule: FillRule,
	},
	Stroke {
		look: Look,
		width: Prop,
		cap: LineCap,
		join: LineJoin,
		miter: f32,
	},
	Place(Placement),
	Other,
}

enum Look {
	Solid { color: Prop, opacity: Prop },
	Gradient(Gradient),
}

struct Gradient {
	radial: bool,
	start: Prop,
	end: Prop,
	stops: usize,
	colors: Prop,
	opacity: Prop,
}

fn items(value: Option<&Value>, vertices: &mut usize, depth: usize) -> Vec<Item> {
	let Some(list) = value.and_then(Value::as_array) else {
		return Vec::new();
	};
	list.iter()
		.filter(|item| !flag(item, "hd"))
		.map(
			|item| match item.get("ty").and_then(Value::as_str).unwrap_or_default() {
				"gr" if depth < MAX_DEPTH => Item::Group(items(item.get("it"), vertices, depth + 1)),
				"sh" => Item::Shape(ShapeProp::parse(item.get("ks"), vertices)),
				"rc" => Item::Rect {
					position: Prop::parse(item.get("p")),
					size: Prop::parse(item.get("s")),
					round: Prop::parse(item.get("r")),
				},
				"el" => Item::Ellipse {
					position: Prop::parse(item.get("p")),
					size: Prop::parse(item.get("s")),
				},
				"fl" => Item::Fill {
					look: Look::Solid {
						color: Prop::parse(item.get("c")),
						opacity: Prop::parse(item.get("o")),
					},
					rule: fill_rule(item),
				},
				"gf" => Item::Fill {
					look: Look::Gradient(Gradient::parse(item)),
					rule: fill_rule(item),
				},
				"st" | "gs" => Item::Stroke {
					look: if item.get("ty").and_then(Value::as_str) == Some("gs") {
						Look::Gradient(Gradient::parse(item))
					} else {
						Look::Solid {
							color: Prop::parse(item.get("c")),
							opacity: Prop::parse(item.get("o")),
						}
					},
					width: Prop::parse(item.get("w")),
					cap: match item.get("lc").and_then(Value::as_i64) {
						Some(2) => LineCap::Round,
						Some(3) => LineCap::Square,
						_ => LineCap::Butt,
					},
					join: match item.get("lj").and_then(Value::as_i64) {
						Some(2) => LineJoin::Round,
						Some(3) => LineJoin::Bevel,
						_ => LineJoin::Miter,
					},
					miter: item
						.get("ml")
						.and_then(Value::as_f64)
						.map_or(4., |value| value as f32),
				},
				"tr" => Item::Place(Placement::parse(Some(item))),
				_ => Item::Other,
			},
		)
		.collect()
}

fn fill_rule(item: &Value) -> FillRule {
	if item.get("r").and_then(Value::as_i64) == Some(2) {
		FillRule::EvenOdd
	} else {
		FillRule::Winding
	}
}

impl Gradient {
	fn parse(item: &Value) -> Self {
		let colors = item.get("g");
		Self {
			radial: item.get("t").and_then(Value::as_i64) == Some(2),
			start: Prop::parse(item.get("s")),
			end: Prop::parse(item.get("e")),
			stops: colors
				.and_then(|colors| colors.get("p"))
				.and_then(Value::as_u64)
				.map_or(0, |count| count.min(64) as usize),
			colors: Prop::parse(colors.and_then(|colors| colors.get("k"))),
			opacity: Prop::parse(item.get("o")),
		}
	}

	fn shader(&self, time: f32, opacity: f32) -> Option<Shader<'static>> {
		let values = self.colors.at(time);
		let alpha = opacity * self.opacity.scalar_or(time, 100.) / 100.;
		let stops: Vec<GradientStop> = values
			.as_chunks::<4>()
			.0
			.iter()
			.take(self.stops)
			.filter_map(|stop| {
				let color =
					Color::from_rgba(unit(stop[1]), unit(stop[2]), unit(stop[3]), alpha.clamp(0., 1.))?;
				Some(GradientStop::new(stop[0].clamp(0., 1.), color))
			})
			.collect();
		let start = point(&self.start.at(time));
		let end = point(&self.end.at(time));
		if self.radial {
			let radius = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
			RadialGradient::new(
				start,
				start,
				radius,
				stops,
				SpreadMode::Pad,
				Transform::identity(),
			)
		} else {
			LinearGradient::new(start, end, stops, SpreadMode::Pad, Transform::identity())
		}
	}
}

impl Look {
	fn paint(&self, time: f32, opacity: f32) -> Option<Paint<'static>> {
		let mut paint = Paint {
			anti_alias: true,
			..Paint::default()
		};
		match self {
			Self::Solid { color, opacity: own } => {
				let alpha = opacity * own.scalar_or(time, 100.) / 100.;
				let mut rgba = color.at(time);
				rgba.resize(4, 1.);
				paint.set_color(color_of(&[rgba[0], rgba[1], rgba[2], rgba[3]], alpha)?);
			}
			Self::Gradient(gradient) => paint.shader = gradient.shader(time, opacity)?,
		}
		Some(paint)
	}
}

enum Op {
	Fill {
		path: Path,
		paint: Paint<'static>,
		rule: FillRule,
		world: Transform,
	},
	Stroke {
		path: Path,
		paint: Paint<'static>,
		stroke: Stroke,
		world: Transform,
	},
}

impl Op {
	fn draw(&self, pixmap: &mut Pixmap) {
		match self {
			Self::Fill {
				path,
				paint,
				rule,
				world,
			} => pixmap.fill_path(path, paint, *rule, *world, None),
			Self::Stroke {
				path,
				paint,
				stroke,
				world,
			} => pixmap.stroke_path(path, paint, stroke, *world, None),
		}
	}
}

fn group(items: &[Item], world: Transform, opacity: f32, time: f32, ops: &mut Vec<Op>) -> Vec<Path> {
	let (local, own_opacity) = items
		.iter()
		.rev()
		.find_map(|item| match item {
			Item::Place(placement) => Some((placement.matrix(time), placement.opacity(time))),
			_ => None,
		})
		.unwrap_or((Transform::identity(), 1.));
	let world = world.pre_concat(local);
	let opacity = opacity * own_opacity;
	let mut pending: Vec<Path> = Vec::new();
	for item in items {
		match item {
			Item::Group(children) => pending.extend(group(children, world, opacity, time, ops)),
			Item::Shape(shape) => pending.extend(shape.path(time)),
			Item::Rect {
				position,
				size,
				round,
			} => pending.extend(rect_path(
				&position.at(time),
				&size.at(time),
				round.scalar_or(time, 0.),
			)),
			Item::Ellipse { position, size } => {
				let (centre, size) = (point(&position.at(time)), point(&size.at(time)));
				pending.extend(
					Rect::from_xywh(centre.x - size.x / 2., centre.y - size.y / 2., size.x, size.y)
						.and_then(PathBuilder::from_oval),
				);
			}
			Item::Fill { look, rule } => {
				if let (Some(path), Some(paint)) = (merged(&pending), look.paint(time, opacity)) {
					ops.push(Op::Fill {
						path,
						paint,
						rule: *rule,
						world,
					});
				}
			}
			Item::Stroke {
				look,
				width,
				cap,
				join,
				miter,
			} => {
				let width = width.scalar_or(time, 1.);
				if let (Some(path), Some(paint), true) =
					(merged(&pending), look.paint(time, opacity), width > 0.)
				{
					let stroke = Stroke {
						width,
						line_cap: *cap,
						line_join: *join,
						miter_limit: *miter,
						..Stroke::default()
					};
					ops.push(Op::Stroke {
						path,
						paint,
						stroke,
						world,
					});
				}
			}
			Item::Place(_) | Item::Other => {}
		}
	}
	pending
		.into_iter()
		.filter_map(|path| path.transform(local))
		.collect()
}

fn merged(paths: &[Path]) -> Option<Path> {
	let mut builder = PathBuilder::new();
	for path in paths {
		builder.push_path(path);
	}
	builder.finish()
}

fn rect_path(position: &[f32], size: &[f32], round: f32) -> Option<Path> {
	let (centre, size) = (point(position), point(size));
	let rect = Rect::from_xywh(centre.x - size.x / 2., centre.y - size.y / 2., size.x, size.y)?;
	let radius = round.min(size.x / 2.).min(size.y / 2.).max(0.);
	if radius <= 0. {
		return Some(PathBuilder::from_rect(rect));
	}
	let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
	let handle = radius * ROUND_KAPPA;
	let mut builder = PathBuilder::new();
	builder.move_to(left + radius, top);
	builder.line_to(right - radius, top);
	builder.cubic_to(
		right - radius + handle,
		top,
		right,
		top + radius - handle,
		right,
		top + radius,
	);
	builder.line_to(right, bottom - radius);
	builder.cubic_to(
		right,
		bottom - radius + handle,
		right - radius + handle,
		bottom,
		right - radius,
		bottom,
	);
	builder.line_to(left + radius, bottom);
	builder.cubic_to(
		left + radius - handle,
		bottom,
		left,
		bottom - radius + handle,
		left,
		bottom - radius,
	);
	builder.line_to(left, top + radius);
	builder.cubic_to(
		left,
		top + radius - handle,
		left + radius - handle,
		top,
		left + radius,
		top,
	);
	builder.close();
	builder.finish()
}

struct Placement {
	anchor: Prop,
	position: Option<Prop>,
	split: Option<(Prop, Prop)>,
	scale: Prop,
	rotation: Prop,
	opacity: Prop,
}

impl Placement {
	fn parse(value: Option<&Value>) -> Self {
		let field = |key: &str| Prop::parse(value.and_then(|value| value.get(key)));
		let position = value.and_then(|value| value.get("p"));
		let split = position
			.filter(|position| flag(position, "s"))
			.map(|position| (Prop::parse(position.get("x")), Prop::parse(position.get("y"))));
		Self {
			anchor: field("a"),
			position: split.is_none().then(|| Prop::parse(position)),
			split,
			scale: field("s"),
			rotation: field("r"),
			opacity: field("o"),
		}
	}

	fn matrix(&self, time: f32) -> Transform {
		let anchor = point(&self.anchor.at(time));
		let position = match (&self.split, &self.position) {
			(Some((x, y)), _) => Point::from_xy(x.scalar_or(time, 0.), y.scalar_or(time, 0.)),
			(None, Some(position)) => point(&position.at(time)),
			(None, None) => Point::zero(),
		};
		let mut scale = self.scale.at(time);
		if scale.is_empty() {
			scale = vec![100., 100.];
		}
		let scale_y = scale.get(1).copied().unwrap_or(scale[0]);
		let matrix = Transform::from_translate(position.x, position.y)
			.pre_rotate(self.rotation.scalar_or(time, 0.))
			.pre_scale(scale[0] / 100., scale_y / 100.)
			.pre_translate(-anchor.x, -anchor.y);
		if matrix.is_finite() {
			matrix
		} else {
			Transform::identity()
		}
	}

	fn opacity(&self, time: f32) -> f32 {
		(self.opacity.scalar_or(time, 100.) / 100.).clamp(0., 1.)
	}
}

struct Key {
	time: f32,
	start: Vec<f32>,
	end: Option<Vec<f32>>,
	hold: bool,
	ease_out: (f32, f32),
	ease_in: (f32, f32),
}

enum Prop {
	Fixed(Vec<f32>),
	Keyed(Vec<Key>),
}

impl Prop {
	fn parse(value: Option<&Value>) -> Self {
		let Some(value) = value else {
			return Self::Fixed(Vec::new());
		};
		let inner = value.get("k").unwrap_or(value);
		if let Some(keys) = keyed(inner) {
			let keys: Vec<Key> = keys
				.iter()
				.map(|key| Key {
					time: number(key, "t"),
					start: numbers(key.get("s")),
					end: key.get("e").map(|end| numbers(Some(end))),
					hold: key.get("h").and_then(Value::as_i64) == Some(1),
					ease_out: ease(key.get("o"), (0., 0.)),
					ease_in: ease(key.get("i"), (1., 1.)),
				})
				.collect();
			return Self::Keyed(keys);
		}
		Self::Fixed(numbers(Some(inner)))
	}

	fn at(&self, time: f32) -> Vec<f32> {
		match self {
			Self::Fixed(values) => values.clone(),
			Self::Keyed(keys) => sample(keys, time),
		}
	}

	fn scalar_or(&self, time: f32, fallback: f32) -> f32 {
		self.at(time)
			.first()
			.copied()
			.filter(|value| value.is_finite())
			.unwrap_or(fallback)
	}
}

fn keyed(value: &Value) -> Option<&Vec<Value>> {
	value
		.as_array()
		.filter(|keys| keys.first().is_some_and(|key| key.get("t").is_some()))
}

fn sample(keys: &[Key], time: f32) -> Vec<f32> {
	let Some(first) = keys.first() else {
		return Vec::new();
	};
	if time <= first.time || keys.len() == 1 {
		return first.start.clone();
	}
	for pair in keys.windows(2) {
		let (from, to) = (&pair[0], &pair[1]);
		if time < to.time {
			if from.hold {
				return from.start.clone();
			}
			let span = to.time - from.time;
			let progress = if span > 0. { (time - from.time) / span } else { 1. };
			let amount = bezier(from.ease_out, from.ease_in, progress);
			let target = from.end.as_ref().unwrap_or(&to.start);
			return lerp_all(&from.start, target, amount);
		}
	}
	let last = &keys[keys.len() - 1];
	if last.start.is_empty() {
		keys[keys.len() - 2]
			.end
			.clone()
			.unwrap_or_else(|| keys[keys.len() - 2].start.clone())
	} else {
		last.start.clone()
	}
}

fn bezier(out: (f32, f32), into: (f32, f32), progress: f32) -> f32 {
	let progress = progress.clamp(0., 1.);
	let curve = |a: f32, b: f32, t: f32| {
		let inverse = 1. - t;
		3. * inverse * inverse * t * a + 3. * inverse * t * t * b + t * t * t
	};
	let (mut low, mut high) = (0_f32, 1_f32);
	let mut t = progress;
	for _ in 0..BEZIER_STEPS {
		let x = curve(out.0, into.0, t);
		if (x - progress).abs() < 1e-4 {
			break;
		}
		if x < progress {
			low = t;
		} else {
			high = t;
		}
		t = (low + high) / 2.;
	}
	curve(out.1, into.1, t)
}

fn ease(value: Option<&Value>, fallback: (f32, f32)) -> (f32, f32) {
	let Some(value) = value else {
		return fallback;
	};
	let first = |key: &str| {
		value.get(key).and_then(|component| match component {
			Value::Array(values) => values.first().and_then(Value::as_f64),
			other => other.as_f64(),
		})
	};
	match (first("x"), first("y")) {
		(Some(x), Some(y)) => (x as f32, y as f32),
		_ => fallback,
	}
}

fn lerp_all(from: &[f32], to: &[f32], amount: f32) -> Vec<f32> {
	if from.len() != to.len() {
		return from.to_vec();
	}
	from.iter()
		.zip(to)
		.map(|(from, to)| from + (to - from) * amount)
		.collect()
}

#[derive(Clone, Default)]
struct Outline {
	vertices: Vec<[f32; 2]>,
	ins: Vec<[f32; 2]>,
	outs: Vec<[f32; 2]>,
	closed: bool,
}

impl Outline {
	fn parse(value: &Value, budget: &mut usize) -> Self {
		let value = match value {
			Value::Array(values) => values.first().unwrap_or(&Value::Null),
			other => other,
		};
		let pairs = |key: &str| -> Vec<[f32; 2]> {
			value
				.get(key)
				.and_then(Value::as_array)
				.map(|points| {
					points
						.iter()
						.map(|pair| {
							let pair = numbers(Some(pair));
							[
								pair.first().copied().unwrap_or(0.),
								pair.get(1).copied().unwrap_or(0.),
							]
						})
						.collect()
				})
				.unwrap_or_default()
		};
		let vertices = pairs("v");
		if vertices.len() > *budget {
			return Self::default();
		}
		*budget -= vertices.len();
		Self {
			ins: pairs("i"),
			outs: pairs("o"),
			closed: value.get("c").and_then(Value::as_bool).unwrap_or(false),
			vertices,
		}
	}

	fn blend(&self, other: &Self, amount: f32) -> Self {
		if self.vertices.len() != other.vertices.len()
			|| self.ins.len() != other.ins.len()
			|| self.outs.len() != other.outs.len()
		{
			return self.clone();
		}
		let mix = |from: &[[f32; 2]], to: &[[f32; 2]]| -> Vec<[f32; 2]> {
			from.iter()
				.zip(to)
				.map(|(from, to)| {
					[
						from[0] + (to[0] - from[0]) * amount,
						from[1] + (to[1] - from[1]) * amount,
					]
				})
				.collect()
		};
		Self {
			vertices: mix(&self.vertices, &other.vertices),
			ins: mix(&self.ins, &other.ins),
			outs: mix(&self.outs, &other.outs),
			closed: self.closed,
		}
	}

	fn path(&self) -> Option<Path> {
		let count = self.vertices.len();
		if count < 2 {
			return None;
		}
		let tangent = |list: &[[f32; 2]], index: usize| list.get(index).copied().unwrap_or([0., 0.]);
		let mut builder = PathBuilder::new();
		builder.move_to(self.vertices[0][0], self.vertices[0][1]);
		let segment = |builder: &mut PathBuilder, from: usize, to: usize| {
			let (start, end) = (self.vertices[from], self.vertices[to]);
			let (out, into) = (tangent(&self.outs, from), tangent(&self.ins, to));
			builder.cubic_to(
				start[0] + out[0],
				start[1] + out[1],
				end[0] + into[0],
				end[1] + into[1],
				end[0],
				end[1],
			);
		};
		for index in 1..count {
			segment(&mut builder, index - 1, index);
		}
		if self.closed {
			segment(&mut builder, count - 1, 0);
			builder.close();
		}
		builder.finish()
	}
}

struct OutlineKey {
	key: Key,
	start: Outline,
	end: Option<Outline>,
}

enum ShapeProp {
	Fixed(Outline),
	Keyed(Vec<OutlineKey>),
}

impl ShapeProp {
	fn parse(value: Option<&Value>, budget: &mut usize) -> Self {
		let inner = value.map(|value| value.get("k").unwrap_or(value));
		let Some(inner) = inner else {
			return Self::Fixed(Outline::default());
		};
		if let Some(keys) = keyed(inner) {
			return Self::Keyed(
				keys.iter()
					.map(|key| OutlineKey {
						start: key
							.get("s")
							.map_or_else(Outline::default, |start| Outline::parse(start, budget)),
						end: key.get("e").map(|end| Outline::parse(end, budget)),
						key: Key {
							time: number(key, "t"),
							start: Vec::new(),
							end: None,
							hold: key.get("h").and_then(Value::as_i64) == Some(1),
							ease_out: ease(key.get("o"), (0., 0.)),
							ease_in: ease(key.get("i"), (1., 1.)),
						},
					})
					.collect(),
			);
		}
		Self::Fixed(Outline::parse(inner, budget))
	}

	fn path(&self, time: f32) -> Option<Path> {
		match self {
			Self::Fixed(outline) => outline.path(),
			Self::Keyed(keys) => {
				let first = keys.first()?;
				if time <= first.key.time || keys.len() == 1 {
					return first.start.path();
				}
				for pair in keys.windows(2) {
					let (from, to) = (&pair[0], &pair[1]);
					if time < to.key.time {
						if from.key.hold {
							return from.start.path();
						}
						let span = to.key.time - from.key.time;
						let progress = if span > 0. {
							(time - from.key.time) / span
						} else {
							1.
						};
						let amount = bezier(from.key.ease_out, from.key.ease_in, progress);
						let target = from.end.as_ref().unwrap_or(&to.start);
						return from.start.blend(target, amount).path();
					}
				}
				let last = &keys[keys.len() - 1];
				if last.start.vertices.is_empty() {
					let before = &keys[keys.len() - 2];
					before.end.as_ref().unwrap_or(&before.start).path()
				} else {
					last.start.path()
				}
			}
		}
	}
}

fn number(value: &Value, key: &str) -> f32 {
	value
		.get(key)
		.and_then(Value::as_f64)
		.map_or(0., |value| value as f32)
}

fn numbers(value: Option<&Value>) -> Vec<f32> {
	match value {
		Some(Value::Array(values)) => values
			.iter()
			.filter_map(Value::as_f64)
			.map(|value| value as f32)
			.collect(),
		Some(Value::Number(number)) => number
			.as_f64()
			.map(|value| vec![value as f32])
			.unwrap_or_default(),
		_ => Vec::new(),
	}
}

fn flag(value: &Value, key: &str) -> bool {
	match value.get(key) {
		Some(Value::Bool(flag)) => *flag,
		Some(Value::Number(number)) => number.as_i64() == Some(1),
		_ => false,
	}
}

fn point(values: &[f32]) -> Point {
	Point::from_xy(
		values.first().copied().unwrap_or(0.),
		values.get(1).copied().unwrap_or(0.),
	)
}

fn unit(value: f32) -> f32 {
	if value > 1. { value / 255. } else { value }.clamp(0., 1.)
}

fn color_of(rgba: &[f32; 4], opacity: f32) -> Option<Color> {
	Color::from_rgba(
		unit(rgba[0]),
		unit(rgba[1]),
		unit(rgba[2]),
		(rgba[3].clamp(0., 1.) * opacity).clamp(0., 1.),
	)
}

fn hex_color(text: &str) -> [f32; 4] {
	let digits = text.trim_start_matches('#');
	let channel = |index: usize| {
		digits
			.get(index..index + 2)
			.and_then(|pair| u8::from_str_radix(pair, 16).ok())
			.map_or(0., |value| f32::from(value) / 255.)
	};
	[channel(0), channel(2), channel(4), 1.]
}
