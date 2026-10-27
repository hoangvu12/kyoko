#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameCase {
	Exact,
	Ignored,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderDifference {
	pub unexpected: Vec<String>,
	pub chrome: Vec<String>,
	pub kyoko: Vec<String>,
}

impl OrderDifference {
	pub fn is_empty(&self) -> bool {
		self.unexpected.is_empty() && self.chrome == self.kyoko
	}
}

pub fn header_names(headers: &[(String, String)], case: NameCase) -> Vec<String> {
	headers
		.iter()
		.map(|(name, _)| name.as_str())
		.filter(|name| !name.starts_with(':'))
		.map(|name| match case {
			NameCase::Exact => name.to_owned(),
			NameCase::Ignored => name.to_ascii_lowercase(),
		})
		.collect()
}

pub fn order_difference(
	chrome: &[(String, String)],
	kyoko: &[(String, String)],
	case: NameCase,
) -> OrderDifference {
	let chrome = header_names(chrome, case);
	let kyoko = header_names(kyoko, case);
	OrderDifference {
		unexpected: kyoko
			.iter()
			.filter(|name| !chrome.contains(name))
			.cloned()
			.collect(),
		chrome: chrome
			.iter()
			.filter(|name| kyoko.contains(name))
			.cloned()
			.collect(),
		kyoko: kyoko
			.iter()
			.filter(|name| chrome.contains(name))
			.cloned()
			.collect(),
	}
}

pub fn missing_names(chrome: &[(String, String)], kyoko: &[(String, String)], case: NameCase) -> Vec<String> {
	let kyoko = header_names(kyoko, case);
	let mut missing: Vec<String> = header_names(chrome, case)
		.into_iter()
		.filter(|name| !kyoko.contains(name))
		.collect();
	missing.dedup();
	missing
}
