use bridge::{
	LinkAction, Names, PollSummary, ReplyPreview, Row, RowKey, SystemIcon, UNKNOWN_ROLE, UNKNOWN_USER,
};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use markdown::TimestampStyle;
use protocol::{
	LenientVec, Message, MessageType, Poll, PollAnswer, PollAnswerCount, PollMedia, PollResults, Snowflake,
	User,
};
use serde_json::json;
use state::ReplyTarget;

use crate::harness::{Running, View};

fn at(text: &str) -> DateTime<Utc> {
	DateTime::parse_from_rfc3339(text)
		.expect("a timestamp")
		.with_timezone(&Utc)
}

#[test]
fn timestamps_format_in_the_local_zone() {
	let zone = FixedOffset::east_opt(7 * 3600).expect("an offset");
	let unix = at("2026-10-01T12:05:09Z").timestamp();
	let now = at("2026-10-01T12:00:00Z");
	let text = |style| bridge::timestamp_in(unix, style, now, &zone);
	assert_eq!(text(TimestampStyle::ShortTime), "7:05 PM");
	assert_eq!(text(TimestampStyle::LongTime), "7:05:09 PM");
	assert_eq!(text(TimestampStyle::ShortDate), "10/01/2026");
	assert_eq!(text(TimestampStyle::LongDate), "October 1, 2026");
	assert_eq!(text(TimestampStyle::ShortDateTime), "October 1, 2026 7:05 PM");
	assert_eq!(
		text(TimestampStyle::LongDateTime),
		"Thursday, October 1, 2026 7:05 PM"
	);
	assert_eq!(text(TimestampStyle::Relative), "in 5 minutes");
	let later = now + Duration::days(3);
	assert_eq!(
		bridge::timestamp_in(unix, TimestampStyle::Relative, later, &zone),
		"2 days ago"
	);
	assert_eq!(
		bridge::timestamp_in(now.timestamp() - 1, TimestampStyle::Relative, now, &zone),
		"1 second ago"
	);
}

#[test]
fn mentions_read_as_names_or_placeholders() {
	let document = markdown::parse("<@1> <@2> <@&3> <@&4> <#5> <#6>");
	let names = Names::collect(
		[&document],
		|id| (id == 1).then(|| "Quiet Gecko".to_owned()),
		|id| (id == 3).then(|| ("Members".to_owned(), Some(0x6666ff))),
		|id| (id == 5).then(|| "general".to_owned()),
	);
	assert_eq!(names.user(1), "@Quiet Gecko");
	assert_eq!(names.user(2), UNKNOWN_USER);
	assert_eq!(names.role(3), ("@Members".to_owned(), Some(0x6666ff)));
	assert_eq!(names.role(4).0, UNKNOWN_ROLE);
	assert_eq!(names.channel(5), "#general");
	assert_eq!(names.channel(6), "#unknown");
	assert_eq!(names.unresolved_users().collect::<Vec<_>>(), vec![2]);
	assert_eq!(
		bridge::snippet(&document, &names, 120),
		"@Quiet Gecko @unknown-user @Members @deleted-role #general #unknown"
	);
}

fn message(id: u64, author: &str, content: &str) -> Message {
	Message {
		id: Snowflake(id),
		author: User {
			id: Snowflake(id + 100),
			username: author.to_owned(),
			..User::default()
		},
		content: content.to_owned(),
		..Message::default()
	}
}

fn quote(original: &Message) -> bridge::Quote {
	bridge::Quote {
		id: original.id,
		author: original.author.clone(),
		name: original.author.username.clone(),
		text: original.content.clone(),
	}
}

#[test]
fn reply_targets_become_previews() {
	let original = message(1, "gecko", "can someone review this?");
	let found = ReplyPreview::from_target(ReplyTarget::Message(&original), true, quote);
	assert!(matches!(&found, ReplyPreview::Message(q) if q.name == "gecko" && !q.attachment_only()));
	let deleted = ReplyPreview::from_target(ReplyTarget::Deleted(&original), true, quote);
	assert!(matches!(deleted, ReplyPreview::Deleted(_)));
	assert_eq!(
		ReplyPreview::from_target(ReplyTarget::Unavailable, true, quote),
		ReplyPreview::Unavailable
	);
	assert_eq!(
		ReplyPreview::from_target(ReplyTarget::Loading, true, quote),
		ReplyPreview::Loading
	);
	assert_eq!(
		ReplyPreview::from_target(ReplyTarget::Loading, false, quote),
		ReplyPreview::Unavailable,
		"without Read Message History a reply never loads, so it reads as gone"
	);
	let image_only = message(2, "koala", "");
	let preview = ReplyPreview::from_target(ReplyTarget::Message(&image_only), true, quote);
	assert!(matches!(preview, ReplyPreview::Message(q) if q.attachment_only()));
}

#[test]
fn media_fits_its_box_without_upscaling() {
	assert_eq!(
		bridge::fit_box(Some(1100), Some(700), bridge::ATTACHMENT_BOX),
		(550., 350.)
	);
	assert_eq!(
		bridge::fit_box(Some(300), Some(450), bridge::ATTACHMENT_BOX),
		(233., 350.)
	);
	assert_eq!(
		bridge::fit_box(Some(240), Some(135), bridge::ATTACHMENT_BOX),
		(240., 135.)
	);
	assert_eq!(
		bridge::fit_box(Some(1280), Some(720), bridge::EMBED_MEDIA_BOX),
		(400., 225.)
	);
	assert_eq!(
		bridge::fit_box(None, Some(720), bridge::ATTACHMENT_BOX),
		(300., 200.)
	);
	assert_eq!(
		bridge::fit_box(Some(0), Some(0), bridge::EMBED_MEDIA_BOX),
		(300., 200.)
	);
}

#[test]
fn custom_emoji_map_to_cdn_images() {
	let still = bridge::emoji_request("https://cdn.example", Snowflake(42), false, 22., 1.5);
	assert_eq!(
		still.url,
		"https://cdn.example/emojis/42.webp?size=64&quality=lossless"
	);
	let moving = bridge::emoji_request("https://cdn.example", Snowflake(42), true, 48., 1.);
	assert_eq!(
		moving.url,
		"https://cdn.example/emojis/42.webp?size=64&quality=lossless&animated=true"
	);
	let document = markdown::parse("<:pepe:7> <a:parrot:8> \u{1f389}");
	assert!(
		document.is_only_emoji(),
		"custom and unicode emoji alone render jumbo"
	);
	assert_eq!(
		bridge::reaction_key(&protocol::PartialEmoji {
			id: Some(Snowflake(7)),
			name: Some("pepe".to_owned()),
			animated: false,
		}),
		"pepe:7"
	);
}

#[test]
fn mock_media_stays_on_the_mock_server() {
	let origin = Some("http://127.0.0.1:8485");
	let foreign = bridge::local_media(
		"https://media.discordapp.net/attachments/1/2/a.png?width=10",
		origin,
	);
	assert!(foreign.starts_with("http://127.0.0.1:8485/media/"));
	assert!(foreign.ends_with(".png"));
	assert_eq!(
		foreign,
		bridge::local_media("https://cdn.discordapp.com/attachments/1/2/a.png", origin),
		"the name depends on the path only, as oxidcord's mock resolver does"
	);
	let local = "http://127.0.0.1:8485/cdn/emojis/1.webp";
	assert_eq!(bridge::local_media(local, origin), local);
	let real = "https://cdn.discordapp.com/emojis/1.webp";
	assert_eq!(bridge::local_media(real, None), real);
}

#[test]
fn links_open_directly_only_for_trusted_hosts() {
	assert_eq!(bridge::link_action("https://github.com/x", &[]), LinkAction::Open);
	assert_eq!(
		bridge::link_action("https://www.youtube.com/watch", &[]),
		LinkAction::Open
	);
	assert_eq!(
		bridge::link_action("https://example.com/masked", &[]),
		LinkAction::Confirm {
			host: "example.com".to_owned()
		}
	);
	assert_eq!(
		bridge::link_action("https://docs.example.com/", &["example.com".to_owned()]),
		LinkAction::Open
	);
	assert_eq!(
		bridge::link_action("https://notexample.com/", &["example.com".to_owned()]),
		LinkAction::Confirm {
			host: "notexample.com".to_owned()
		}
	);
	assert_eq!(bridge::link_action("mailto:a@b.c", &[]), LinkAction::Open);
	assert_eq!(bridge::link_action("file:///c:/x", &[]), LinkAction::Ignore);
	assert_eq!(bridge::link_action("not a url", &[]), LinkAction::Ignore);
}

#[test]
fn polls_sum_votes_and_close_at_expiry() {
	let now = at("2026-10-01T12:00:00Z");
	let poll = Poll {
		question: PollMedia {
			text: Some("Next tool?".to_owned()),
			emoji: None,
		},
		answers: LenientVec::from(
			["Rust", "Zig"]
				.iter()
				.enumerate()
				.map(|(index, text)| PollAnswer {
					answer_id: Some(index as i64 + 1),
					poll_media: PollMedia {
						text: Some((*text).to_owned()),
						emoji: None,
					},
				})
				.collect::<Vec<_>>(),
		),
		expiry: Some(now + Duration::hours(1)),
		allow_multiselect: true,
		layout_type: None,
		results: Some(PollResults {
			is_finalized: false,
			answer_counts: LenientVec::from(vec![
				PollAnswerCount {
					id: 1,
					count: 3,
					me_voted: true,
				},
				PollAnswerCount {
					id: 2,
					count: 1,
					me_voted: false,
				},
			]),
		}),
	};
	let open = PollSummary::of(&poll, now);
	assert_eq!(open.total, 4);
	assert_eq!(bridge::percent(open.choices[0].share), 75);
	assert_eq!(open.voted(), vec![1]);
	assert_eq!(open.hint(), "Select one or more answers");
	assert_eq!(open.votes_label(), "4 votes");
	let closed = PollSummary::of(&poll, now + Duration::hours(2));
	assert!(closed.finished);
	assert_eq!(closed.hint(), "Poll closed");
}

#[test]
fn system_messages_read_as_oxidcord_writes_them() {
	let join = bridge::system_line(MessageType::UserJoin, "Gecko", "", "");
	assert_eq!(join.icon, SystemIcon::Join);
	assert_eq!(join.text, "Gecko joined the server.");
	let tier = bridge::system_line(MessageType::PremiumGuildSubscriptionTier2, "Koala", "", "");
	assert_eq!(tier.icon, SystemIcon::Boost);
	assert_eq!(
		tier.text,
		"Koala just boosted the server! It has reached Level 2."
	);
	let added = bridge::system_line(MessageType::RecipientAdd, "Lynx", "Gecko", "");
	assert_eq!(added.text, "Lynx added Gecko to the group.");
	let unknown = bridge::system_line(MessageType::Unknown(999), "Lynx", "", "");
	assert_eq!(unknown.icon, SystemIcon::Other);
	assert_eq!(bridge::file_size(1_572_864), "1.5 MB");
	assert_eq!(bridge::file_size(900), "900 B");
}

fn entry(view: &View, id: Snowflake) -> Option<&bridge::MessageEntry> {
	view.timeline.messages().find(|entry| entry.id == id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_row_updates_when_its_reply_target_and_mention_resolve() {
	let mut running = Running::start("message-resolve").await;
	let channel_id = running
		.view
		.sidebar
		.channels()
		.find(|channel| channel.kind == bridge::ChannelKind::Text)
		.map(|channel| channel.id)
		.expect("a text channel");
	let command = running.open_channel(channel_id).expect("a select command");
	running.session().send(command);
	running
		.until("the channel history to load", |view| {
			view.timeline.messages().next().is_some()
		})
		.await;
	running.view.timeline.take_delta();
	let fixtures = running.server.fixtures();
	let history = &fixtures.messages[&channel_id.0];
	let oldest = *history.keys().next().expect("a fixture message");
	let loaded: Vec<Snowflake> = running.view.timeline.messages().map(|entry| entry.id).collect();
	assert!(
		!loaded.contains(&Snowflake(oldest)),
		"the original is outside the window"
	);
	let state = running.session().state().clone();
	let stranger = fixtures
		.users
		.values()
		.find(|user| state.read().user(user.id).is_none())
		.cloned()
		.expect("a user the client has not seen");
	let author = loaded
		.last()
		.and_then(|id| entry(&running.view, *id))
		.map(|entry| entry.message.author.clone())
		.expect("an author");
	let id = Snowflake(loaded.last().expect("a message").0 + (1 << 22));
	running.server.control().dispatch(
		"MESSAGE_CREATE",
		&json!({
			"id": id.to_string(),
			"channel_id": channel_id.to_string(),
			"guild_id": running.view.sidebar.selected().map(|guild| guild.to_string()),
			"author": serde_json::to_value(&author).expect("an author value"),
			"content": format!("<@{}> have a look", stranger.id),
			"type": 19,
			"timestamp": Utc::now().to_rfc3339(),
			"message_reference": {
				"message_id": oldest.to_string(),
				"channel_id": channel_id.to_string(),
			},
		}),
	);
	running
		.until("the reply to arrive unresolved", |view| {
			entry(view, id).is_some_and(|entry| entry.reply.is_some())
		})
		.await;
	running.view.timeline.take_delta();
	running
		.until("the original and the mention to resolve", |view| {
			entry(view, id).is_some_and(|entry| {
				matches!(&entry.reply, Some(ReplyPreview::Message(quote)) if quote.id.0 == oldest)
					&& entry.names.user(stranger.id.0) != UNKNOWN_USER
			})
		})
		.await;
	let resolved = entry(&running.view, id).expect("the reply").clone();
	assert_eq!(
		resolved.names.user(stranger.id.0),
		format!("@{}", bridge::user_display(&stranger))
	);
	let delta = running
		.view
		.timeline
		.take_delta()
		.expect("the resolution is published");
	assert!(
		delta.updated.contains(&RowKey::Message(id)),
		"the row is redrawn in place: {delta:?}"
	);
	assert!(matches!(
		running
			.view
			.timeline
			.rows()
			.iter()
			.find(|row| row.key == RowKey::Message(id))
			.map(|row| &row.row),
		Some(Row::Message(_))
	));
	running.stop().await;
}
