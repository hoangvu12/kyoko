use chrono::{DateTime, Duration, Utc};
use protocol::{
	LenientVec, PartialEmoji, Poll, PollAnswer, PollCreate, PollCreateAnswer, PollCreateMedia, PollEmoji,
	PollLayout, PollMedia, PollResults,
};

pub const POLL_QUESTION_MAX: usize = 300;
pub const POLL_ANSWER_MAX: usize = 55;
pub const POLL_ANSWERS_MAX: usize = 10;
pub const POLL_DEFAULT_HOURS: u32 = 24;
pub const POLL_DURATIONS: [(u32, &str); 7] = [
	(1, "1 hour"),
	(4, "4 hours"),
	(8, "8 hours"),
	(24, "24 hours"),
	(72, "3 days"),
	(168, "1 week"),
	(336, "2 weeks"),
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PollDraft {
	pub question: String,
	pub answers: Vec<PollDraftAnswer>,
	pub duration_hours: u32,
	pub multiple: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PollDraftAnswer {
	pub text: String,
	pub emoji: Option<PartialEmoji>,
}

pub fn poll_length(text: &str) -> usize {
	text.trim().chars().count()
}

impl PollDraft {
	fn filled(&self) -> impl Iterator<Item = &PollDraftAnswer> {
		self.answers.iter().filter(|answer| poll_length(&answer.text) > 0)
	}

	pub fn problem(&self) -> Option<&'static str> {
		let question = poll_length(&self.question);
		if question == 0 {
			return Some("A poll needs a question");
		}
		if question > POLL_QUESTION_MAX {
			return Some("The question can be at most 300 characters");
		}
		if self.answers.len() > POLL_ANSWERS_MAX {
			return Some("A poll can have at most 10 answers");
		}
		if self.filled().next().is_none() {
			return Some("A poll needs at least one answer");
		}
		if self
			.answers
			.iter()
			.any(|answer| poll_length(&answer.text) > POLL_ANSWER_MAX)
		{
			return Some("An answer can be at most 55 characters");
		}
		if !POLL_DURATIONS
			.iter()
			.any(|(hours, _)| *hours == self.duration_hours)
		{
			return Some("Pick one of the poll durations");
		}
		None
	}

	pub fn body(&self) -> PollCreate {
		PollCreate {
			question: PollCreateMedia {
				text: self.question.trim().to_owned(),
				emoji: None,
			},
			answers: self
				.filled()
				.map(|answer| PollCreateAnswer {
					poll_media: PollCreateMedia {
						text: answer.text.trim().to_owned(),
						emoji: answer.emoji.as_ref().and_then(poll_emoji),
					},
				})
				.collect(),
			allow_multiselect: self.multiple,
			duration: self.duration_hours,
			layout_type: PollLayout::Default,
		}
	}

	pub fn preview(&self, now: DateTime<Utc>) -> Poll {
		Poll {
			question: PollMedia {
				text: Some(self.question.trim().to_owned()),
				emoji: None,
			},
			answers: LenientVec::from(
				self.filled()
					.enumerate()
					.map(|(index, answer)| PollAnswer {
						answer_id: Some(index as i64 + 1),
						poll_media: PollMedia {
							text: Some(answer.text.trim().to_owned()),
							emoji: answer.emoji.clone(),
						},
					})
					.collect::<Vec<_>>(),
			),
			expiry: Some(now + Duration::hours(i64::from(self.duration_hours))),
			allow_multiselect: self.multiple,
			layout_type: Some(PollLayout::Default),
			results: Some(PollResults::default()),
		}
	}
}

fn poll_emoji(emoji: &PartialEmoji) -> Option<PollEmoji> {
	match (emoji.id, emoji.name.as_ref()) {
		(Some(id), _) => Some(PollEmoji {
			id: Some(id),
			name: None,
		}),
		(None, Some(name)) if !name.is_empty() => Some(PollEmoji {
			id: None,
			name: Some(name.clone()),
		}),
		_ => None,
	}
}
