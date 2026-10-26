use std::time::{Duration, Instant};

use plugins::{Limits, PluginHost, Status};
use protocol::Snowflake;
use serde_json::json;
use session::{DisplayHooks, Formatting, Person, RelativeTime, ShownName};

use crate::support::{approve, config, scratch, write_plugin};

const SHOW_ME_YOUR_NAME: &str = "show-me-your-name";
const DONT_ROUND: &str = "dont-round-my-timestamps";

const BOTH_HOOKS: &str = r#"
return {
	format_name = function(person)
		return { name = person.name .. "!", extra = person.username }
	end,
	format_relative = function(time)
		return time.unit .. " " .. tostring(time.seconds)
	end,
}
"#;

fn host(root: &std::path::Path, limits: Limits, bundled: bool) -> PluginHost {
	PluginHost::new(config(root, limits, bundled))
}

fn nicknamed() -> Person {
	Person {
		user_id: Snowflake(41),
		guild_id: Some(Snowflake(7)),
		username: "quiet_gecko".to_owned(),
		global_name: Some("Quiet Gecko".to_owned()),
		nick: Some("Gecko".to_owned()),
	}
}

fn plain(username: &str, global_name: &str) -> Person {
	Person {
		user_id: Snowflake(42),
		guild_id: None,
		username: username.to_owned(),
		global_name: Some(global_name.to_owned()),
		nick: None,
	}
}

fn shown(name: &str, extra: Option<&str>) -> Option<ShownName> {
	Some(ShownName {
		name: name.to_owned(),
		extra: extra.map(str::to_owned),
	})
}

fn span(seconds: u64, past: bool) -> RelativeTime {
	let units: [(&'static str, u64); 6] = [
		("second", 1),
		("minute", 60),
		("hour", 3600),
		("day", 86_400),
		("month", 30 * 86_400),
		("year", 365 * 86_400),
	];
	let (unit, length) = units
		.iter()
		.rev()
		.find(|(_, length)| seconds >= *length)
		.copied()
		.unwrap_or(units[0]);
	RelativeTime {
		seconds,
		past,
		unit,
		value: seconds as f64 / length as f64,
		text: "kyoko's own text".to_owned(),
	}
}

#[test]
fn display_hooks_run_only_with_their_own_permission() {
	let root = scratch("display-permissions");
	write_plugin(&root, "no-grants", &[], "", BOTH_HOOKS);
	write_plugin(&root, "names-only", &["hook:display_name"], "", BOTH_HOOKS);
	let host = host(&root, Limits::default(), false);
	approve(&host, "no-grants");
	approve(&host, "names-only");

	let names = DisplayHooks::formatters(&host, Formatting::Names);
	assert_eq!(names.len(), 1, "{names:?}");
	assert!(names[0].starts_with("names-only/"), "{names:?}");
	assert!(DisplayHooks::formatters(&host, Formatting::RelativeTimes).is_empty());
	assert_eq!(
		host.format_name(&nicknamed()),
		shown("Gecko!", Some("quiet_gecko"))
	);
	assert_eq!(host.format_relative(&span(90, true)), None);

	write_plugin(&root, "times-only", &["hook:timestamp"], "", BOTH_HOOKS);
	host.scan();
	approve(&host, "times-only");
	assert_eq!(
		host.format_relative(&span(90, true)),
		Some("minute 90".to_owned())
	);
	assert_eq!(
		host.format_name(&nicknamed()),
		shown("Gecko!", Some("quiet_gecko"))
	);
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn show_me_your_name_puts_the_username_beside_or_instead_of_the_name() {
	let root = scratch("show-me-your-name");
	let host = host(&root, Limits::default(), true);
	assert_eq!(host.status(SHOW_ME_YOUR_NAME), Some(Status::Disabled));
	assert_eq!(host.format_name(&nicknamed()), None, "it ships disabled");
	approve(&host, SHOW_ME_YOUR_NAME);

	assert_eq!(
		host.format_name(&nicknamed()),
		shown("Gecko", Some("quiet_gecko"))
	);
	assert_eq!(
		host.format_name(&plain("quiet_gecko", "Quiet Gecko")),
		shown("Quiet Gecko", Some("quiet_gecko"))
	);
	assert_eq!(host.format_name(&plain("gecko", "Gecko")), None);

	host.set_setting(SHOW_ME_YOUR_NAME, "hide_same", &json!(false))
		.expect("hide_same is a setting");
	assert_eq!(
		host.format_name(&plain("gecko", "Gecko")),
		shown("Gecko", Some("gecko"))
	);

	host.set_setting(SHOW_ME_YOUR_NAME, "mode", &json!("user-nick"))
		.expect("mode is a setting");
	assert_eq!(
		host.format_name(&nicknamed()),
		shown("quiet_gecko", Some("Gecko"))
	);

	host.set_setting(SHOW_ME_YOUR_NAME, "mode", &json!("user"))
		.expect("mode is a setting");
	assert_eq!(host.format_name(&nicknamed()), shown("quiet_gecko", None));
	assert_eq!(host.format_name(&plain("gecko", "gecko")), None);

	host.disable(SHOW_ME_YOUR_NAME);
	assert_eq!(host.format_name(&nicknamed()), None);
	assert!(DisplayHooks::formatters(&host, Formatting::Names).is_empty());
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dont_round_my_timestamps_rounds_relative_times_down() {
	let root = scratch("dont-round");
	let host = host(&root, Limits::default(), true);
	assert_eq!(host.format_relative(&span(2 * 86_400 + 23 * 3600, true)), None);
	approve(&host, DONT_ROUND);

	let cases = [
		(2 * 86_400 + 23 * 3600, true, "2 days ago"),
		(59 * 60 + 59, true, "59 minutes ago"),
		(90, false, "in 1 minute"),
		(45, true, "45 seconds ago"),
		(7 * 365 * 86_400 + 200 * 86_400, true, "7 years ago"),
		(11 * 30 * 86_400 + 29 * 86_400, false, "in 11 months"),
	];
	for (seconds, past, expected) in cases {
		assert_eq!(
			host.format_relative(&span(seconds, past)).as_deref(),
			Some(expected),
			"{seconds} seconds"
		);
	}
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failing_or_slow_formatter_falls_back_and_is_suspended() {
	let root = scratch("display-faults");
	write_plugin(
		&root,
		"broken-names",
		&["hook:display_name"],
		"",
		"return { format_name = function(person) error(\"no names today\") end }",
	);
	write_plugin(
		&root,
		"slow-times",
		&["hook:timestamp"],
		"",
		"return { format_relative = function(time) while true do end end }",
	);
	let limits = Limits {
		name_deadline: Duration::from_millis(30),
		time_deadline: Duration::from_millis(30),
		..Limits::default()
	};
	let host = host(&root, limits, false);
	approve(&host, "broken-names");
	approve(&host, "slow-times");

	for _ in 0..3 {
		assert_eq!(host.format_name(&nicknamed()), None);
		let started = Instant::now();
		assert_eq!(host.format_relative(&span(90, true)), None);
		assert!(
			started.elapsed() < Duration::from_secs(1),
			"{:?}",
			started.elapsed()
		);
	}
	assert!(matches!(
		host.status("broken-names"),
		Some(Status::Suspended { .. })
	));
	assert!(matches!(
		host.status("slow-times"),
		Some(Status::Suspended { .. })
	));
	assert!(DisplayHooks::formatters(&host, Formatting::Names).is_empty());
	assert!(DisplayHooks::formatters(&host, Formatting::RelativeTimes).is_empty());
	let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_formatter_list_changes_with_settings_restarts_and_disabling() {
	let root = scratch("display-formatters");
	let host = host(&root, Limits::default(), true);
	approve(&host, SHOW_ME_YOUR_NAME);
	let first = DisplayHooks::formatters(&host, Formatting::Names);
	assert_eq!(first.len(), 1);
	host.set_setting(SHOW_ME_YOUR_NAME, "mode", &json!("user"))
		.expect("mode is a setting");
	let changed = DisplayHooks::formatters(&host, Formatting::Names);
	assert_ne!(first, changed, "a settings change is a new formatter revision");
	host.reload(SHOW_ME_YOUR_NAME);
	let restarted = DisplayHooks::formatters(&host, Formatting::Names);
	assert_ne!(changed, restarted, "a restart is a new formatter generation");
	host.disable(SHOW_ME_YOUR_NAME);
	assert!(DisplayHooks::formatters(&host, Formatting::Names).is_empty());
	let _ = std::fs::remove_dir_all(&root);
}
