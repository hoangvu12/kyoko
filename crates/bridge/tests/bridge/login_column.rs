use bridge::{Failure, LoginChoice, LoginColumn, Screen, Status};

use super::sign_in::Desk;

async fn submit(desk: &mut Desk, choice: &mut LoginChoice, token: &str) {
	let check = desk
		.flow
		.submit(token.to_owned())
		.expect("the login screen takes the token");
	choice.follow(&desk.flow);
	let checked = check.await;
	desk.flow.checked(checked);
	choice.follow(&desk.flow);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_login_screen_opens_on_discord_com_and_returns_there_after_a_session() {
	let mut desk = Desk::open("login-column").await;
	desk.flow.launch(None);
	desk.acknowledge();
	let mut choice = LoginChoice::default();
	assert_eq!(choice.column(), LoginColumn::Discord);
	assert_eq!(LoginColumn::Discord.name(), "discord");
	for column in [LoginColumn::Discord, LoginColumn::Email, LoginColumn::Token] {
		assert_eq!(LoginColumn::named(column.name()), Some(column));
	}
	assert_eq!(LoginColumn::named("password"), None);

	assert!(choice.choose(&desk.flow, LoginColumn::Email));
	choice.follow(&desk.flow);
	assert_eq!(
		choice.column(),
		LoginColumn::Email,
		"the email link opens the form"
	);
	assert!(choice.choose(&desk.flow, LoginColumn::Discord));
	assert_eq!(
		choice.column(),
		LoginColumn::Discord,
		"the way back returns to discord.com"
	);

	assert!(choice.choose(&desk.flow, LoginColumn::Token));
	submit(&mut desk, &mut choice, "not-the-mock-token").await;
	assert_eq!(
		desk.flow.screen(),
		&Screen::Login(Status::Failed(Failure::Rejected))
	);
	assert_eq!(
		choice.column(),
		LoginColumn::Token,
		"a rejected token stays on the token form"
	);

	let token = desk.server.token().to_owned();
	submit(&mut desk, &mut choice, &token).await;
	assert_eq!(desk.flow.screen(), &Screen::Main);
	assert_eq!(choice.column(), LoginColumn::Discord);
	assert!(
		!choice.choose(&desk.flow, LoginColumn::Email),
		"no column is chosen away from the login screen"
	);
	desk.connected().await;

	let sign_out = desk.flow.sign_out().expect("a session to sign out of");
	tokio::task::spawn_blocking(move || sign_out.finish())
		.await
		.expect("sign-out finishes");
	desk.flow.signed_out();
	choice.follow(&desk.flow);
	assert_eq!(desk.flow.screen(), &Screen::Login(Status::SignedOut));
	assert_eq!(
		choice.column(),
		LoginColumn::Discord,
		"after a sign-out the login screen leads with discord.com again"
	);
	desk.stop().await;
}
