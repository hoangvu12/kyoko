use crate::sign_in::{Flow, Screen};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginColumn {
	#[default]
	Discord,
	Email,
	Token,
}

impl LoginColumn {
	const ALL: [LoginColumn; 3] = [LoginColumn::Discord, LoginColumn::Email, LoginColumn::Token];

	pub fn name(self) -> &'static str {
		match self {
			LoginColumn::Discord => "discord",
			LoginColumn::Email => "email",
			LoginColumn::Token => "token",
		}
	}

	pub fn named(name: &str) -> Option<LoginColumn> {
		Self::ALL.into_iter().find(|column| column.name() == name)
	}
}

#[derive(Debug, Default)]
pub struct LoginChoice {
	column: LoginColumn,
}

impl LoginChoice {
	pub fn column(&self) -> LoginColumn {
		self.column
	}

	pub fn choose(&mut self, flow: &Flow, column: LoginColumn) -> bool {
		let shown = matches!(flow.screen(), Screen::Login(_));
		if shown {
			self.column = column;
		}
		shown
	}

	pub fn follow(&mut self, flow: &Flow) {
		if !matches!(flow.screen(), Screen::Login(_)) {
			self.column = LoginColumn::default();
		}
	}
}
