use chrono::{DateTime, Duration, Utc};
use protocol::{EmbedKind, Message, MessageType, PartialEmoji, Permissions, Poll, Snowflake};
use state::State;

pub fn can_create_polls(state: &State, channel_id: Snowflake) -> bool {
	state
		.my_permissions(channel_id)
		.contains(Permissions::SEND_MESSAGES | Permissions::SEND_POLLS)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollSummary {
	pub question: String,
	pub choices: Vec<PollChoice>,
	pub total: i64,
	pub finished: bool,
	pub multi: bool,
	pub left: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollChoice {
	pub id: i64,
	pub text: String,
	pub emoji: Option<PartialEmoji>,
	pub count: i64,
	pub share: f32,
	pub voted: bool,
	pub winner: bool,
}

fn shown_emoji(emoji: Option<&PartialEmoji>) -> Option<PartialEmoji> {
	emoji
		.filter(|emoji| emoji.id.is_some() || emoji.name.as_ref().is_some_and(|name| !name.is_empty()))
		.cloned()
}

impl PollSummary {
	pub fn of(poll: &Poll, now: DateTime<Utc>) -> Self {
		let count_of = |id: i64| {
			poll.results
				.as_ref()
				.and_then(|results| results.answer_counts.iter().find(|entry| entry.id == id))
				.map_or((0, false), |entry| (entry.count.max(0), entry.me_voted))
		};
		let counts: Vec<(i64, bool)> = poll
			.answers
			.iter()
			.map(|answer| count_of(answer.answer_id.unwrap_or_default()))
			.collect();
		let total: i64 = counts.iter().map(|(count, _)| count).sum();
		let best = counts.iter().map(|(count, _)| *count).max().unwrap_or(0);
		let finished = poll.results.as_ref().is_some_and(|results| results.is_finalized)
			|| poll.expiry.is_some_and(|expiry| expiry <= now);
		let choices = poll
			.answers
			.iter()
			.zip(counts)
			.map(|(answer, (count, voted))| PollChoice {
				id: answer.answer_id.unwrap_or_default(),
				text: answer.poll_media.text.clone().unwrap_or_default(),
				emoji: shown_emoji(answer.poll_media.emoji.as_ref()),
				count,
				share: if total > 0 {
					count as f32 / total as f32
				} else {
					0.
				},
				voted,
				winner: finished && best > 0 && count == best,
			})
			.collect();
		Self {
			question: poll.question.text.clone().unwrap_or_default(),
			choices,
			total,
			finished,
			multi: poll.allow_multiselect,
			left: poll.expiry.filter(|_| !finished).map(|expiry| expiry - now),
		}
	}

	pub fn voted(&self) -> Vec<i64> {
		self.choices
			.iter()
			.filter(|choice| choice.voted)
			.map(|choice| choice.id)
			.collect()
	}

	pub fn has_voted(&self) -> bool {
		self.choices.iter().any(|choice| choice.voted)
	}

	pub fn hint(&self) -> &'static str {
		if self.multi {
			"Select one or more answers"
		} else {
			"Select one answer"
		}
	}

	pub fn votes_label(&self) -> String {
		if self.total == 1 {
			"1 vote".to_owned()
		} else {
			format!("{} votes", self.total)
		}
	}

	pub fn footer(&self) -> String {
		let state = match self.left {
			_ if self.finished => "Poll closed".to_owned(),
			Some(left) => time_left(left),
			None => return self.votes_label(),
		};
		format!("{} \u{b7} {state}", self.votes_label())
	}
}

pub fn time_left(left: Duration) -> String {
	let minutes = left.num_minutes().max(1);
	let day = 24 * 60;
	if minutes >= day {
		format!("{}d left", minutes / day)
	} else if minutes >= 60 {
		format!("{}h left", minutes / 60)
	} else {
		format!("{minutes}m left")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollOutcome {
	pub poll_id: Option<Snowflake>,
	pub question: String,
	pub winner: Option<String>,
	pub emoji: Option<PartialEmoji>,
	pub share: f32,
	pub total: i64,
}

impl PollOutcome {
	pub fn of(message: &Message) -> Option<Self> {
		if message.kind != MessageType::PollResult {
			return None;
		}
		let embed = message
			.embeds
			.iter()
			.find(|embed| embed.kind == Some(EmbedKind::PollResult))?;
		let field = |name: &str| {
			embed
				.fields
				.iter()
				.find(|field| field.name == name)
				.map(|field| field.value.trim().to_owned())
				.filter(|value| !value.is_empty())
		};
		let number = |name: &str| field(name).and_then(|value| value.parse::<i64>().ok());
		let total = number("total_votes").unwrap_or(0).max(0);
		let votes = number("victor_answer_votes").unwrap_or(0).max(0);
		let emoji = PartialEmoji {
			id: field("victor_answer_emoji_id")
				.and_then(|value| value.parse::<u64>().ok())
				.map(Snowflake),
			name: field("victor_answer_emoji_name"),
			animated: field("victor_answer_emoji_animated").is_some_and(|value| value == "true"),
		};
		Some(Self {
			poll_id: message
				.message_reference
				.as_ref()
				.and_then(|reference| reference.message_id),
			question: field("poll_question_text").unwrap_or_default(),
			winner: field("victor_answer_text"),
			emoji: shown_emoji(Some(&emoji)),
			share: if total > 0 {
				votes as f32 / total as f32
			} else {
				0.
			},
			total,
		})
	}
}
