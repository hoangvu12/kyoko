use bridge::{Area, DEFAULT_EMOJI_NOTE, PopoutSubject, expression_details, place_popout};
use protocol::{Snowflake, StickerFormat};
use state::Expression;

use crate::harness::Running;

const CHAT: (f32, f32) = (900., 640.);
const GAP: f32 = 8.;
const MARGIN: f32 = 8.;

#[test]
fn a_popout_never_covers_its_item_and_stays_inside_the_chat() {
	let sizes = [(300., 90.), (300., 191.), (300., 400.)];
	let items = [(22., 22.), (48., 48.), (160., 160.)];
	for (width, height) in sizes {
		for (item_width, item_height) in items {
			let mut x = 0.;
			while x + item_width <= CHAT.0 {
				let mut y = 0.;
				while y + item_height <= CHAT.1 {
					let item = Area {
						x,
						y,
						width: item_width,
						height: item_height,
					};
					let (left, top) = place_popout(item, (width, height), CHAT, GAP, MARGIN);
					let card = Area {
						x: left,
						y: top,
						width,
						height,
					};
					assert!(!card.overlaps(&item), "{card:?} covers {item:?}");
					assert!(
						card.x >= MARGIN && card.right() <= CHAT.0 - MARGIN,
						"{card:?} leaves the chat sideways for {item:?}"
					);
					let beside = card.x >= item.right() || card.right() <= item.x;
					if beside {
						assert!(
							card.y >= MARGIN && card.bottom() <= CHAT.1 - MARGIN,
							"{card:?} leaves the chat for {item:?}"
						);
					}
					y += 37.;
				}
				x += 41.;
			}
		}
	}
	let item = Area {
		x: 100.,
		y: 500.,
		width: 22.,
		height: 22.,
	};
	let (left, top) = place_popout(item, (300., 191.), CHAT, GAP, MARGIN);
	assert_eq!(left, item.right() + GAP, "beside the item, on its right");
	assert_eq!(top + 191., item.bottom(), "bottom aligned with the item");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emoji_and_sticker_popouts_name_the_expression_and_its_server() {
	let running = Running::start("popouts").await;
	let guild_id = Snowflake(running.server.fixtures().guilds[0].id);
	let shared = running.session().state().clone();
	named(&shared.read(), guild_id);
	running.stop().await;
}

fn named(state: &state::State, guild_id: Snowflake) {
	let guild = state.guild(guild_id).expect("the first guild");
	let emoji = guild
		.emojis
		.iter()
		.find(|emoji| emoji.id.is_some() && emoji.name.is_some())
		.expect("a custom emoji");
	let custom = PopoutSubject::Custom(Expression::Emoji {
		id: emoji.id.expect("an id"),
		name: emoji.name.clone().expect("a name"),
		animated: false,
	});
	let details = expression_details(state, &custom);
	assert_eq!(
		details.title,
		format!(":{}:", emoji.name.as_deref().unwrap_or_default())
	);
	assert_eq!(
		details.description.as_deref(),
		Some(format!("This emoji is from {}.", guild.name).as_str())
	);
	assert_eq!(
		details.server.as_ref().map(|server| server.guild_id),
		Some(guild_id)
	);
	assert_eq!(details.source_heading, Some("This emoji is from"));

	let foreign = PopoutSubject::Custom(Expression::Emoji {
		id: Snowflake(1),
		name: "elsewhere".to_owned(),
		animated: false,
	});
	let details = expression_details(state, &foreign);
	assert!(details.server.is_none(), "no server kyoko cannot name");

	let sob = expression_details(state, &PopoutSubject::Unicode("\u{1f62d}".to_owned()));
	assert_eq!(sob.title, ":sob:");
	assert_eq!(sob.description.as_deref(), Some(DEFAULT_EMOJI_NOTE));
	assert!(sob.server.is_none());

	if let Some(sticker) = state.guild_stickers(guild_id).and_then(|stickers| {
		stickers
			.iter()
			.find(|sticker| sticker.format_type == Some(StickerFormat::Png))
	}) {
		let subject = PopoutSubject::Custom(Expression::Sticker {
			id: sticker.id,
			name: sticker.name.clone(),
			format: StickerFormat::Png,
		});
		let details = expression_details(state, &subject);
		assert_eq!(details.title, sticker.name);
		assert_eq!(details.source_heading, Some("This sticker is from"));
		assert_eq!(
			details.server.as_ref().map(|server| server.guild_id),
			Some(guild_id)
		);
	}
}
