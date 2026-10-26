use chrono::{Duration, Utc};
use protocol::{
	Embed, EmbedField, EmbedKind, LenientVec, Message, MessageReference, MessageReferenceKind, MessageType,
	Poll, PollAnswer, PollCreate, PollLayout, PollMedia, PollResults, Snowflake,
};
use serde_json::{Value, json};

use crate::error::ApiError;
use crate::fixtures::CURRENT_USER_ID;
use crate::json::to_json;
use crate::state::Shared;

const QUESTION_MAX: usize = 300;
const ANSWER_MAX: usize = 55;
const ANSWERS_MAX: usize = 10;
const MAX_HOURS: u32 = 32 * 24;

pub fn created(create: &PollCreate) -> Result<Poll, ApiError> {
	let question = create.question.text.trim();
	if question.is_empty() || question.chars().count() > QUESTION_MAX {
		return Err(ApiError::invalid_body("Invalid poll question"));
	}
	if create.answers.is_empty() || create.answers.len() > ANSWERS_MAX {
		return Err(ApiError::invalid_body("Invalid poll answers"));
	}
	let bad_answer = create.answers.iter().any(|answer| {
		let text = answer.poll_media.text.trim();
		text.is_empty() || text.chars().count() > ANSWER_MAX
	});
	if bad_answer {
		return Err(ApiError::invalid_body("Invalid poll answer"));
	}
	if create.duration == 0 || create.duration > MAX_HOURS {
		return Err(ApiError::invalid_body("Invalid poll duration"));
	}
	let answers: Vec<PollAnswer> = create
		.answers
		.iter()
		.enumerate()
		.map(|(index, answer)| PollAnswer {
			answer_id: Some(index as i64 + 1),
			poll_media: PollMedia {
				text: Some(answer.poll_media.text.trim().to_owned()),
				emoji: answer
					.poll_media
					.emoji
					.as_ref()
					.map(|emoji| protocol::PartialEmoji {
						id: emoji.id,
						name: emoji.name.clone(),
						animated: false,
					}),
			},
		})
		.collect();
	Ok(Poll {
		question: PollMedia {
			text: Some(question.to_owned()),
			emoji: None,
		},
		answers: LenientVec::from(answers),
		expiry: Some(Utc::now() + Duration::hours(i64::from(create.duration))),
		allow_multiselect: create.allow_multiselect,
		layout_type: Some(PollLayout::Default),
		results: Some(PollResults::default()),
	})
}

pub fn result_embed(poll: &Poll) -> Embed {
	let counts = poll
		.results
		.as_ref()
		.map(|results| results.answer_counts.to_vec())
		.unwrap_or_default();
	let total: i64 = counts.iter().map(|count| count.count).sum();
	let mut fields = vec![
		field(
			"poll_question_text",
			poll.question.text.clone().unwrap_or_default(),
		),
		field("total_votes", total.to_string()),
	];
	let best = counts.iter().map(|count| count.count).max().unwrap_or(0);
	let leaders: Vec<_> = counts.iter().filter(|count| count.count == best).collect();
	if best > 0
		&& let [leader] = leaders.as_slice()
		&& let Some(answer) = poll
			.answers
			.iter()
			.find(|answer| answer.answer_id == Some(leader.id))
	{
		fields.push(field("victor_answer_id", leader.id.to_string()));
		fields.push(field(
			"victor_answer_text",
			answer.poll_media.text.clone().unwrap_or_default(),
		));
		fields.push(field("victor_answer_votes", leader.count.to_string()));
		if let Some(emoji) = &answer.poll_media.emoji {
			if let Some(id) = emoji.id {
				fields.push(field("victor_answer_emoji_id", id.to_string()));
			}
			if let Some(name) = &emoji.name {
				fields.push(field("victor_answer_emoji_name", name.clone()));
			}
		}
	}
	Embed {
		kind: Some(EmbedKind::PollResult),
		fields: LenientVec::from(fields),
		..Embed::default()
	}
}

fn field(name: &str, value: String) -> EmbedField {
	EmbedField {
		name: name.to_owned(),
		value,
		inline: false,
	}
}

pub fn result_message(id: u64, poll_message: &Message, poll: &Poll) -> Message {
	Message {
		id: Snowflake(id),
		channel_id: poll_message.channel_id,
		guild_id: poll_message.guild_id,
		author: poll_message.author.clone(),
		timestamp: Some(Utc::now()),
		kind: MessageType::PollResult,
		embeds: LenientVec::from(vec![result_embed(poll)]),
		message_reference: Some(MessageReference {
			kind: Some(MessageReferenceKind::Default),
			message_id: Some(poll_message.id),
			channel_id: Some(poll_message.channel_id),
			guild_id: poll_message.guild_id,
			fail_if_not_exists: Some(false),
		}),
		..Message::default()
	}
}

fn counts_of(message: &mut Value) -> Result<&mut Vec<Value>, ApiError> {
	if message["poll"].is_null() {
		return Err(ApiError::invalid_body("This message has no poll"));
	}
	let results = &mut message["poll"]["results"];
	if !results.is_object() || !results["answer_counts"].is_array() {
		*results = json!({ "is_finalized": false, "answer_counts": [] });
	}
	if results["is_finalized"].as_bool() == Some(true) {
		return Err(ApiError::invalid_body("This poll has ended"));
	}
	results["answer_counts"]
		.as_array_mut()
		.ok_or_else(ApiError::unknown_message)
}

fn bump(counts: &mut Vec<Value>, answer: i64, delta: i64, mine: Option<bool>) {
	let index = match counts
		.iter()
		.position(|count| count["id"].as_i64() == Some(answer))
	{
		Some(index) => index,
		None => {
			counts.push(json!({ "id": answer, "count": 0, "me_voted": false }));
			counts.len() - 1
		}
	};
	let count = &mut counts[index];
	count["count"] = json!((count["count"].as_i64().unwrap_or(0) + delta).max(0));
	if let Some(mine) = mine {
		count["me_voted"] = json!(mine);
	}
}

impl Shared {
	fn broadcast_votes(&self, channel_id: u64, message_id: u64, user_id: u64, changes: &[(i64, bool)]) {
		let guild_id = self
			.store
			.lock()
			.channels
			.get(&channel_id)
			.and_then(|channel| channel.guild_id);
		for (answer, added) in changes {
			let event = if *added {
				"MESSAGE_POLL_VOTE_ADD"
			} else {
				"MESSAGE_POLL_VOTE_REMOVE"
			};
			self.hub.broadcast(
				event,
				&json!({
					"user_id": user_id.to_string(),
					"channel_id": channel_id.to_string(),
					"message_id": message_id.to_string(),
					"guild_id": guild_id.map(|id| id.to_string()),
					"answer_id": answer,
				}),
			);
		}
	}

	pub fn vote_poll(&self, channel_id: u64, message_id: u64, answers: &[i64]) -> Result<(), ApiError> {
		let changes = {
			let mut store = self.store.lock();
			let message = store
				.message_mut(channel_id, message_id)
				.ok_or_else(ApiError::unknown_message)?;
			let multi = message["poll"]["allow_multiselect"].as_bool() == Some(true);
			let known: Vec<i64> = message["poll"]["answers"]
				.as_array()
				.map(|answers| {
					answers
						.iter()
						.filter_map(|answer| answer["answer_id"].as_i64())
						.collect()
				})
				.unwrap_or_default();
			if answers.iter().any(|answer| !known.contains(answer)) || (!multi && answers.len() > 1) {
				return Err(ApiError::invalid_body("Invalid poll answer"));
			}
			let counts = counts_of(message)?;
			let before: Vec<i64> = counts
				.iter()
				.filter(|count| count["me_voted"].as_bool() == Some(true))
				.filter_map(|count| count["id"].as_i64())
				.collect();
			let mut changes = Vec::new();
			for answer in before.iter().filter(|answer| !answers.contains(answer)) {
				bump(counts, *answer, -1, Some(false));
				changes.push((*answer, false));
			}
			for answer in answers.iter().filter(|answer| !before.contains(answer)) {
				bump(counts, *answer, 1, Some(true));
				changes.push((*answer, true));
			}
			changes
		};
		self.broadcast_votes(channel_id, message_id, CURRENT_USER_ID, &changes);
		Ok(())
	}

	pub fn poll_vote_as(
		&self,
		channel_id: u64,
		message_id: u64,
		user_id: u64,
		answer: i64,
		added: bool,
	) -> Result<(), ApiError> {
		{
			let mut store = self.store.lock();
			let message = store
				.message_mut(channel_id, message_id)
				.ok_or_else(ApiError::unknown_message)?;
			let counts = counts_of(message)?;
			bump(counts, answer, if added { 1 } else { -1 }, None);
		}
		self.broadcast_votes(channel_id, message_id, user_id, &[(answer, added)]);
		Ok(())
	}

	pub fn expire_poll(&self, channel_id: u64, message_id: u64) -> Result<Value, ApiError> {
		let mut store = self.store.lock();
		let message = store
			.message_mut(channel_id, message_id)
			.ok_or_else(ApiError::unknown_message)?;
		if message["author"]["id"].as_str() != Some(&CURRENT_USER_ID.to_string()) {
			return Err(ApiError::not_author());
		}
		counts_of(message)?;
		message["poll"]["results"]["is_finalized"] = json!(true);
		message["poll"]["expiry"] = json!(crate::ops::now_iso());
		let value = message.clone();
		let gateway = self.gateway_message(&store, &value);
		let result = serde_json::from_value::<Message>(value.clone())
			.ok()
			.and_then(|poll_message| {
				let poll = poll_message.poll.clone()?;
				let id = store.next_snowflake();
				Some(to_json(&result_message(id, &poll_message, &poll)))
			});
		if let Some(result) = &result
			&& let Some(id) = crate::json::parse_id(&result["id"])
		{
			store
				.messages
				.entry(channel_id)
				.or_default()
				.insert(id, result.clone());
			store.refresh_last_message(channel_id);
		}
		let result_gateway = result.as_ref().map(|result| self.gateway_message(&store, result));
		drop(store);
		self.hub.broadcast("MESSAGE_UPDATE", &gateway);
		if let Some(result_gateway) = result_gateway {
			self.hub.broadcast("MESSAGE_CREATE", &result_gateway);
		}
		Ok(value)
	}
}
