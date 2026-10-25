use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};

pub fn clock_text(at: DateTime<Utc>) -> String {
	clock_in(at, &Local)
}

pub fn message_time(at: DateTime<Utc>, today: NaiveDate) -> String {
	message_time_in(at, today, &Local)
}

pub fn full_time(at: DateTime<Utc>) -> String {
	at.with_timezone(&Local)
		.format("%A, %B %-d, %Y %-I:%M %p")
		.to_string()
}

pub fn day_label(day: NaiveDate) -> String {
	day.format("%B %-d, %Y").to_string()
}

pub fn today() -> NaiveDate {
	Local::now().date_naive()
}

fn clock_in<Z: TimeZone>(at: DateTime<Utc>, zone: &Z) -> String
where
	Z::Offset: std::fmt::Display,
{
	at.with_timezone(zone).format("%-I:%M %p").to_string()
}

fn message_time_in<Z: TimeZone>(at: DateTime<Utc>, today: NaiveDate, zone: &Z) -> String
where
	Z::Offset: std::fmt::Display,
{
	let day = at.with_timezone(zone).date_naive();
	let clock = clock_in(at, zone);
	if day == today {
		format!("Today at {clock}")
	} else if today.pred_opt() == Some(day) {
		format!("Yesterday at {clock}")
	} else {
		format!("{:02}/{:02}/{}", day.month(), day.day(), day.year())
	}
}
