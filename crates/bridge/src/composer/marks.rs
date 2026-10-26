use std::ops::Range;

const FENCE: &str = "```";
const QUOTE_BLOCK: &str = ">>> ";
const QUOTE: &str = "> ";
const PAIRS: [&str; 6] = ["**", "__", "~~", "||", "*", "_"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
	Syntax,
	Label,
	Mention,
	Code,
	Spoiler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
	pub range: Range<usize>,
	pub mark: Mark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineState {
	#[default]
	Text,
	Code,
}

pub fn line_marks(line: &str, state: LineState) -> (Vec<Marked>, LineState) {
	let mut marks = Vec::new();
	if let Some(rest) = line.trim_start().strip_prefix(FENCE) {
		let start = line.len() - line.trim_start().len();
		marks.push(marked(start..start + FENCE.len(), Mark::Syntax));
		let label = rest.trim_end();
		let next = match state {
			LineState::Text if !rest.contains(FENCE) => {
				if !label.is_empty() {
					let from = start + FENCE.len();
					marks.push(marked(from..from + label.len(), Mark::Label));
				}
				LineState::Code
			}
			_ => LineState::Text,
		};
		return (marks, next);
	}
	if state == LineState::Code {
		return (marks, state);
	}
	let mut from = 0;
	for quote in [QUOTE_BLOCK, QUOTE] {
		if line.starts_with(quote) {
			marks.push(marked(0..quote.len() - 1, Mark::Syntax));
			from = quote.len();
			break;
		}
	}
	inline(line, from..line.len(), &mut marks);
	(marks, LineState::Text)
}

pub fn text_marks(text: &str) -> Vec<Marked> {
	let mut all = Vec::new();
	let mut state = LineState::Text;
	let mut offset = 0;
	for line in text.split('\n') {
		let (marks, next) = line_marks(line, state);
		all.extend(marks.into_iter().map(|mark| Marked {
			range: mark.range.start + offset..mark.range.end + offset,
			mark: mark.mark,
		}));
		state = next;
		offset += line.len() + 1;
	}
	all
}

fn marked(range: Range<usize>, mark: Mark) -> Marked {
	Marked { range, mark }
}

fn inline(line: &str, span: Range<usize>, marks: &mut Vec<Marked>) {
	let mut at = span.start;
	while at < span.end {
		let rest = &line[at..span.end];
		if let Some(end) = code_span(rest) {
			marks.push(marked(at..at + 1, Mark::Syntax));
			marks.push(marked(at + 1..at + end - 1, Mark::Code));
			marks.push(marked(at + end - 1..at + end, Mark::Syntax));
			at += end;
			continue;
		}
		if let Some(found) = angle(rest, at, marks) {
			at += found;
			continue;
		}
		if let Some((delimiter, inner_end)) = paired(line, at, span.end) {
			let width = delimiter.len();
			marks.push(marked(at..at + width, Mark::Syntax));
			if delimiter == "||" {
				marks.push(marked(at + width..inner_end, Mark::Spoiler));
			}
			inline(line, at + width..inner_end, marks);
			marks.push(marked(inner_end..inner_end + width, Mark::Syntax));
			at = inner_end + width;
			continue;
		}
		at += rest.chars().next().map_or(1, char::len_utf8);
	}
}

fn code_span(rest: &str) -> Option<usize> {
	let inner = rest.strip_prefix('`')?;
	let close = inner.find('`')?;
	(close > 0).then_some(close + 2)
}

fn paired(line: &str, at: usize, end: usize) -> Option<(&'static str, usize)> {
	let rest = &line[at..end];
	PAIRS.into_iter().find_map(|delimiter| {
		let inner = rest.strip_prefix(delimiter)?;
		if inner.starts_with(char::is_whitespace) || inner.starts_with(delimiter) {
			return None;
		}
		if delimiter == "_" && line[..at].chars().next_back().is_some_and(char::is_alphanumeric) {
			return None;
		}
		let close = closing(inner, delimiter)?;
		let inner_end = at + delimiter.len() + close;
		let after = &line[inner_end + delimiter.len()..];
		if delimiter == "_" && after.chars().next().is_some_and(char::is_alphanumeric) {
			return None;
		}
		Some((delimiter, inner_end))
	})
}

fn closing(inner: &str, delimiter: &str) -> Option<usize> {
	let mut from = 0;
	while let Some(found) = inner[from..].find(delimiter) {
		let close = from + found;
		let doubled = delimiter.len() == 1 && inner[close + 1..].starts_with(delimiter);
		if close > 0 && !inner[..close].ends_with(char::is_whitespace) && !doubled {
			return Some(close);
		}
		from = close + delimiter.len() + usize::from(doubled);
	}
	None
}

fn angle(rest: &str, at: usize, marks: &mut Vec<Marked>) -> Option<usize> {
	let inner = rest.strip_prefix('<')?;
	let close = inner.find('>')?;
	let body = &inner[..close];
	let width = close + 2;
	let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
	let mention = ["@!", "@&", "@", "#"]
		.into_iter()
		.find_map(|prefix| body.strip_prefix(prefix))
		.is_some_and(digits);
	if mention {
		marks.push(marked(at..at + width, Mark::Mention));
		return Some(width);
	}
	let emoji = body.strip_prefix("a:").or_else(|| body.strip_prefix(':'))?;
	let (name, id) = emoji.split_once(':')?;
	let named = !name.is_empty()
		&& name
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
	if !named || !digits(id) {
		return None;
	}
	let name_from = at + 1 + body.len() - emoji.len();
	marks.push(marked(at..name_from, Mark::Syntax));
	marks.push(marked(name_from + name.len()..at + width, Mark::Syntax));
	Some(width)
}
