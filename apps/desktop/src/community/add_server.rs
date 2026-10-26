use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use image::imageops::FilterType;
use protocol::Snowflake;
use session::{Command, InviteSource};
use zgx::compose::prelude::*;
use zgx::image::ImageData;
use zgx::platform::{FilePicker, PlatformService};
use zgx::reactive::Signal;
use zgx::style::ObjectFit;

use super::CommunityLayer;
use super::parts::{self, Tone, body, enabled_when, field, footer, hint, input, labelled_button, on_enter};
use crate::kit;
use crate::timeline::{TypeSize, icon, tok};

const WIDTH: f32 = 440.;
const JOIN: usize = 0;
const CREATE: usize = 1;
const START: usize = 2;
const OPTION_ICON: f32 = 40.;
const NAME_LIMIT: usize = 100;
const ICON_SIDE: u32 = 512;
const CARD_ICON: f32 = 56.;
const TILE: f32 = 96.;
const STAT_DOT: f32 = 8.;
const IMAGE_TYPES: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

#[derive(Clone)]
struct Picked {
	png: Rc<Vec<u8>>,
	image: Arc<ImageData>,
}

impl PartialEq for Picked {
	fn eq(&self, other: &Self) -> bool {
		Rc::ptr_eq(&self.png, &other.png)
	}
}

#[derive(Debug, Clone, PartialEq)]
struct Preview {
	guild_id: Option<Snowflake>,
	name: String,
	icon: Option<String>,
	online: i64,
	members: i64,
	joined: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Lookup {
	Nothing,
	Typed(String),
	Unknown(String),
	Found(String, Preview),
}

fn lookup(layer: &CommunityLayer, typed: &str) -> Lookup {
	let Some(code) = bridge::invite_code(typed) else {
		return Lookup::Nothing;
	};
	layer
		.read(|state| {
			if let Some(preview) = state.invite_preview(&code) {
				let joined = preview.guild_id.is_some_and(|id| state.guild(id).is_some());
				return Lookup::Found(
					code.clone(),
					Preview {
						guild_id: preview.guild_id,
						name: preview.guild_name.clone(),
						icon: preview.guild_icon.clone(),
						online: preview.online_count,
						members: preview.member_count,
						joined,
					},
				);
			}
			if state.is_invite_unknown(&code) {
				return Lookup::Unknown(code.clone());
			}
			Lookup::Typed(code.clone())
		})
		.unwrap_or(Lookup::Nothing)
}

fn square_png(bytes: &[u8]) -> Option<Picked> {
	let decoded = image::load_from_memory(bytes).ok()?;
	let resized = decoded.resize_to_fill(ICON_SIDE, ICON_SIDE, FilterType::Lanczos3);
	let mut encoded = std::io::Cursor::new(Vec::new());
	resized.write_to(&mut encoded, image::ImageFormat::Png).ok()?;
	let rgba = resized.to_rgba8();
	let image = ImageData::new(rgba.width(), rgba.height(), rgba.into_raw()).ok()?;
	Some(Picked {
		png: Rc::new(encoded.into_inner()),
		image: Arc::new(image),
	})
}

fn read_icon(path: &Path) -> Option<Picked> {
	square_png(&std::fs::read(path).ok()?)
}

pub fn view(layer: &Rc<CommunityLayer>) -> View {
	let runtime = &layer.runtime;
	let path = runtime.signal(START);
	let code = runtime.signal(String::new());
	let owner = layer
		.read(|state| state.current_user_id().map(|me| state.display_name(None, me)))
		.flatten()
		.unwrap_or_default();
	let name = runtime.signal(format!("{owner}'s server"));
	let picked: Signal<Option<Picked>> = runtime.signal(None);
	let submit = {
		let (layer, path, code, name, picked) = (
			layer.clone(),
			path.clone(),
			code.clone(),
			name.clone(),
			picked.clone(),
		);
		Rc::new(move || {
			if path.with_untracked(|path| *path) == JOIN {
				join(&layer, &code.with_untracked(Clone::clone));
			} else {
				create(&layer, &name, &picked);
			}
		})
	};
	let joining = path.clone();
	let creating = path.clone();
	let join_view = join_body(layer, &code, submit.clone()).reactive_style(move || {
		if joining.get() == 0 {
			Styles::new().flex()
		} else {
			Styles::new().hidden()
		}
	});
	let create_view = create_body(&name, &picked, submit.clone()).reactive_style(move || {
		if creating.get() == 1 {
			Styles::new().flex()
		} else {
			Styles::new().hidden()
		}
	});
	let (close, cancel) = (layer.clone(), layer.clone());
	let (titled, described, started, finishing) = (path.clone(), path.clone(), path.clone(), path.clone());
	column()
		.w(WIDTH)
		.overflow_hidden()
		.rounded(tok("radius_lg"))
		.border(tok("border_thickness"))
		.border_color(tok("border"))
		.bg(tok("bg_4"))
		.text_color(tok("text_3"))
		.child(kit::header_with(
			kit::title_view(text_signal(move || {
				match titled.get() {
					JOIN => "Join a Server",
					CREATE => "Customize Your Server",
					_ => "Create Your Server",
				}
				.to_owned()
			})),
			div()
				.w_full()
				.reactive_style(move || {
					if described.get() == START {
						Styles::new().flex()
					} else {
						Styles::new().hidden()
					}
				})
				.child(kit::note(
					"Your server is where you and your friends hang out. Make yours and start talking.",
				)),
			move || close.close(),
		))
		.child(
			body()
				.child(start_body(&path).reactive_style(move || {
					if started.get() == START {
						Styles::new().flex()
					} else {
						Styles::new().hidden()
					}
				}))
				.child(join_view)
				.child(create_view)
				.child(
					footer("Cancel", confirm(layer, &path, &code, &name, submit), move || {
						cancel.close()
					})
					.reactive_style(move || {
						if finishing.get() == START {
							Styles::new().hidden()
						} else {
							Styles::new().flex()
						}
					}),
				),
		)
}

fn start_body(path: &Signal<usize>) -> View {
	let (create, join) = (path.clone(), path.clone());
	column()
		.w_full()
		.gap(tok("space_16"))
		.child(
			button()
				.focusable(false)
				.w_full()
				.items_center()
				.justify_start()
				.gap(tok("space_12"))
				.px(tok("space_12"))
				.py(tok("space_8"))
				.rounded(tok("radius_sm"))
				.border(tok("border_thickness"))
				.border_color(tok("border"))
				.bg(tok("bg_3"))
				.hover(|style| style.bg(tok("bg_2")))
				.cursor(Cursor::Pointer)
				.on_click(move || {
					create.set(CREATE);
				})
				.child(
					div()
						.size(OPTION_ICON, OPTION_ICON)
						.shrink_0()
						.items_center()
						.justify_center()
						.rounded(tok("radius_sm"))
						.bg(tok("accent_3"))
						.child(icon("plus", 20., "text_0")),
				)
				.child(
					text("Create My Own")
						.grow()
						.text_size(tok("font_size_md"))
						.line_height(tok("line_height_label"))
						.font_weight(600)
						.text_color(tok("text_2")),
				)
				.child(kit::menu_icon("chevron_right", false)),
		)
		.child(kit::footer_band(
			column()
				.w_full()
				.items_center()
				.gap(tok("space_8"))
				.child(
					text("Have an invite already?")
						.text_size(tok("font_size_lg"))
						.line_height(tok("line_height_lg"))
						.font_weight(600)
						.text_color(tok("text_2")),
				)
				.child(
					kit::action("Join a Server", kit::Variant::Secondary, move || {
						join.set(JOIN);
					})
					.focusable(false)
					.w_full(),
				),
		))
}

fn join(layer: &CommunityLayer, typed: &str) {
	match lookup(layer, typed) {
		Lookup::Nothing => {}
		Lookup::Typed(code) | Lookup::Unknown(code) => layer.send(Command::ResolveInvite { code }),
		Lookup::Found(_, preview) if preview.joined => {}
		Lookup::Found(code, _) => {
			layer.send(Command::AcceptInvite {
				code,
				from: InviteSource::AddServer,
			});
			layer.close();
		}
	}
}

fn create(layer: &CommunityLayer, name: &Signal<String>, picked: &Signal<Option<Picked>>) {
	let name: String = name.with_untracked(|name| name.trim().chars().take(NAME_LIMIT).collect());
	if name.is_empty() {
		return;
	}
	let icon = picked.with_untracked(|picked| {
		picked
			.as_ref()
			.map(|picked| rest::api::image_data_uri("image/png", &picked.png))
	});
	layer.send(Command::CreateGuild { name, icon });
	layer.close();
}

fn confirm(
	layer: &Rc<CommunityLayer>,
	path: &Signal<usize>,
	code: &Signal<String>,
	name: &Signal<String>,
	submit: Rc<dyn Fn()>,
) -> View {
	let state = {
		let (layer, path, code, name) = (layer.clone(), path.clone(), code.clone(), name.clone());
		let revision = layer.revision.clone();
		move || -> (&'static str, bool) {
			revision.get();
			if path.get() == CREATE {
				return ("Create Server", name.with(|name| !name.trim().is_empty()));
			}
			match lookup(&layer, &code.get()) {
				Lookup::Nothing => ("Find Server", false),
				Lookup::Typed(_) | Lookup::Unknown(_) => ("Find Server", true),
				Lookup::Found(_, preview) if preview.joined => ("Joined", false),
				Lookup::Found(..) => ("Join Server", true),
			}
		}
	};
	let label = {
		let state = state.clone();
		text_signal(move || state().0.to_owned())
	};
	let enabled = {
		let state = state.clone();
		move || state().1
	};
	let can = state;
	let button = labelled_button(label, Tone::Primary, move || {
		if can().1 {
			submit();
		}
	});
	enabled_when(button, enabled)
}

fn join_body(layer: &Rc<CommunityLayer>, code: &Signal<String>, submit: Rc<dyn Fn()>) -> View {
	let found = {
		let (layer, code) = (layer.clone(), code.clone());
		let revision = layer.revision.clone();
		move || {
			revision.get();
			lookup(&layer, &code.get())
		}
	};
	let (this, shown) = (layer.clone(), layer.clone());
	column()
		.w_full()
		.gap(16.)
		.child(field(
			"Invite link or code",
			on_enter(
				input("Invite link or code", code.clone(), "https://discord.gg/hTKzmak"),
				move || submit(),
			),
		))
		.child(hint("Paste an invite link or type the invite code."))
		.child_when(
			move || match found() {
				Lookup::Found(_, preview) => Some(Some(preview)),
				Lookup::Unknown(_) => Some(None),
				_ => {
					this.set_images(Vec::new());
					None
				}
			},
			move |preview: Option<Preview>, _| match preview {
				Some(preview) => card(&shown, &preview),
				None => {
					shown.set_images(Vec::new());
					text("This invite is invalid or has expired.".to_owned())
						.type_size("font_sm")
						.text_color(tok("danger"))
				}
			},
		)
}

fn card(layer: &Rc<CommunityLayer>, preview: &Preview) -> View {
	let picture = preview
		.guild_id
		.zip(preview.icon.as_deref())
		.map(|(guild_id, hash)| layer.pictures.guild_icon(guild_id, hash, CARD_ICON));
	layer.set_images(picture.iter().cloned().collect());
	let initials: String = preview
		.name
		.split_whitespace()
		.filter_map(|word| word.chars().next())
		.take(3)
		.collect();
	let avatar = div()
		.size(CARD_ICON, CARD_ICON)
		.shrink_0()
		.rounded(CARD_ICON / 2.)
		.overflow_hidden()
		.items_center()
		.justify_center()
		.bg(tok("brand"))
		.child(
			text(initials)
				.text_size(CARD_ICON * 0.36)
				.font_weight(600)
				.text_color(tok("text_on_brand")),
		);
	let avatar = match picture {
		Some(name) => {
			let shown = layer.pictures.picture(&name);
			avatar.reactive_style(move || match shown.get() {
				Some(image) => Styles::new().bg_image(image, ObjectFit::Cover),
				None => Styles::new(),
			})
		}
		None => avatar,
	};
	row()
		.w_full()
		.items_center()
		.gap(16.)
		.p(16.)
		.rounded(tok("radius_md"))
		.border(1.)
		.border_color(tok("divider"))
		.bg(tok("background_secondary"))
		.child(avatar)
		.child(
			column()
				.min_w(0.)
				.gap(4.)
				.child(
					text(preview.name.clone())
						.truncate()
						.type_size("font_md")
						.font_weight(700)
						.text_color(tok("header_primary")),
				)
				.child(
					row()
						.items_center()
						.gap(12.)
						.child(stat("status_online", format!("{} Online", preview.online)))
						.child(stat("status_offline", format!("{} Members", preview.members))),
				),
		)
}

fn stat(color: &str, label: String) -> View {
	row()
		.items_center()
		.gap(4.)
		.child(div().size(STAT_DOT, STAT_DOT).rounded(STAT_DOT).bg(tok(color)))
		.child(text(label).type_size("font_sm").text_color(tok("text_muted")))
}

fn create_body(name: &Signal<String>, picked: &Signal<Option<Picked>>, submit: Rc<dyn Fn()>) -> View {
	let initial = {
		let name = name.clone();
		move || {
			name.with(|name| {
				name.split_whitespace()
					.filter_map(|word| word.chars().next())
					.take(3)
					.collect::<String>()
			})
		}
	};
	let shown = picked.clone();
	let tile = div()
		.size(TILE, TILE)
		.shrink_0()
		.rounded(TILE / 2.)
		.overflow_hidden()
		.items_center()
		.justify_center()
		.bg(tok("brand"))
		.reactive_style(move || match shown.get() {
			Some(picked) => Styles::new().bg_image(picked.image, ObjectFit::Cover),
			None => Styles::new(),
		})
		.child({
			let hidden = picked.clone();
			text_signal(initial)
				.text_size(TILE * 0.36)
				.font_weight(600)
				.text_color(tok("text_on_brand"))
				.reactive_style(move || {
					if hidden.with(Option::is_some) {
						Styles::new().invisible()
					} else {
						Styles::new().visible()
					}
				})
		});
	column()
		.w_full()
		.gap(16.)
		.child(
			row().items_center().gap(16.).child(tile).child(
				column()
					.items_start()
					.gap(8.)
					.child(upload_button(picked.clone()))
					.child(hint("Minimum 128 by 128. A square image works best.")),
			),
		)
		.child(field(
			"Server name",
			on_enter(input("Server name", name.clone(), ""), move || submit()),
		))
}

fn upload_button(picked: Signal<Option<Picked>>) -> View {
	component(move |cx| {
		let platform = cx.try_service::<PlatformService>();
		let tasks = cx.tasks();
		let picked = picked.clone();
		button()
			.h(parts::BUTTON_HEIGHT)
			.px(12.)
			.gap(8.)
			.items_center()
			.border(1.)
			.border_color(tok("divider"))
			.rounded(tok("radius_sm"))
			.bg(parts::clear())
			.hover(|style| style.bg(tok("modifier_hover")))
			.cursor(Cursor::Pointer)
			.text_size(tok("font_sm"))
			.font_weight(500)
			.text_color(tok("text_normal"))
			.child(icon("upload", 16., "interactive_normal"))
			.child(text("Upload Icon".to_owned()))
			.on_click(move || {
				let Some(platform) = platform.clone() else {
					return;
				};
				let pending = platform.0.open_files(FilePicker {
					title: "Upload Icon".to_owned(),
					directory: None,
					file_name: None,
					filters: vec![(
						"Images".to_owned(),
						IMAGE_TYPES.iter().map(|kind| (*kind).to_owned()).collect(),
					)],
					multiple: false,
				});
				let picked = picked.clone();
				tasks.spawn(async move {
					let Ok(paths) = pending.await else {
						return;
					};
					if let Some(icon) = paths.first().and_then(|path| read_icon(path)) {
						picked.set(Some(icon));
					}
				});
			})
	})
}
