use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use super::Outcome;

const STYLE: &str = "body{background:#1e1f22;color:#dbdee1;font:14px system-ui,sans-serif;margin:24px}\
a{color:#00a8fc}table{border-collapse:collapse;margin-bottom:32px}\
td,th{border-bottom:1px solid #3f4147;padding:6px 12px;text-align:left}\
section{margin-bottom:48px}img{max-width:100%;border:1px solid #3f4147}\
.muted{color:#949ba4}.failed{color:#f23f42}";

pub fn write(
	out: &Path,
	reference: &str,
	outcomes: &[Outcome],
	elapsed: Duration,
	seed: u64,
	anchor: &str,
) -> Result<()> {
	let mut html = String::new();
	let _ = write!(
		html,
		"<!doctype html><html><head><meta charset=\"utf-8\"><title>Kyoko parity: {reference}</title><style>{STYLE}</style></head><body>"
	);
	let _ = write!(
		html,
		"<h1>Parity captures: kyoko against {}</h1><p class=\"muted\">{} screens, seed {seed}, fixture date {}, {:.0}s.</p>",
		escape(reference),
		outcomes.len(),
		escape(anchor),
		elapsed.as_secs_f64()
	);
	let _ = write!(
		html,
		"<table><tr><th>Screen</th><th>Ticket</th><th>{}</th><th>kyoko</th><th>Differing pixels</th></tr>",
		escape(reference)
	);
	for outcome in outcomes {
		let name = escape(&outcome.screen.name);
		let _ = write!(
			html,
			"<tr><td><a href=\"#{name}\">{name}</a></td><td>{}</td><td{}>{}</td><td{}>{}</td><td>{}</td></tr>",
			escape(&outcome.screen.ticket),
			class(outcome.reference.shot.label()),
			outcome.reference.shot.label(),
			class(outcome.kyoko.shot.label()),
			outcome.kyoko.shot.label(),
			outcome
				.differing
				.map(|share| format!("{:.1}%", share * 100.))
				.unwrap_or_else(|| "-".to_owned())
		);
	}
	html.push_str("</table>");
	for outcome in outcomes {
		let name = escape(&outcome.screen.name);
		let _ = write!(
			html,
			"<section id=\"{name}\"><h2>{name}</h2><p class=\"muted\">Ticket {}. {}</p>\
			<p><a href=\"{name}/{reference}.png\">{reference}.png</a> · <a href=\"{name}/kyoko.png\">kyoko.png</a>{}</p>\
			<img src=\"{name}/pair.png\" alt=\"{name}\"></section>",
			escape(&outcome.screen.ticket),
			escape(&outcome.screen.steps),
			if outcome.differing.is_some() {
				format!(" · <a href=\"{name}/diff.png\">diff.png</a>")
			} else {
				String::new()
			}
		);
	}
	html.push_str("</body></html>");
	let path = out.join("index.html");
	std::fs::write(&path, html).with_context(|| format!("write {}", path.display()))
}

fn class(label: &str) -> &'static str {
	match label {
		"failed" => " class=\"failed\"",
		"captured" => "",
		_ => " class=\"muted\"",
	}
}

fn escape(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('`', "")
}
