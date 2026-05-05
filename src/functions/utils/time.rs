use std::time::Duration;

use chrono::{Datelike, Local, TimeZone, Timelike};

pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

pub fn dhms(milliseconds: u64) -> String {
    let day = 1000 * 60 * 60 * 24;
    let hour = 1000 * 60 * 60;
    let minute = 1000 * 60;

    let days = milliseconds / day;
    let hours = (milliseconds % day) / hour;
    let minutes = (milliseconds % hour) / minute;

    format!("{days} Day(s) {hours} hours {minutes} minutes.")
}

pub fn convert_unix_timestamp(seconds: u64) -> String {
    let Some(date) = Local.timestamp_opt(seconds as i64, 0).single() else {
        return seconds.to_string();
    };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = MONTHS[date.month0() as usize];
    format!(
        "{} {} {} {}:{}:{}",
        month,
        date.day(),
        date.year(),
        date.hour(),
        date.minute(),
        date.second()
    )
}

pub fn time_ago_str(milliseconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();

    if milliseconds >= now {
        return "just now".to_owned();
    }

    let elapsed_seconds = (now - milliseconds) / 1000;
    match elapsed_seconds {
        0..=59 => "just now".to_owned(),
        60..=3_599 => plural(elapsed_seconds / 60, "minute"),
        3_600..=86_399 => plural(elapsed_seconds / 3_600, "hour"),
        86_400..=2_591_999 => plural(elapsed_seconds / 86_400, "day"),
        2_592_000..=31_535_999 => plural(elapsed_seconds / 2_592_000, "month"),
        _ => plural(elapsed_seconds / 31_536_000, "year"),
    }
}

fn plural(value: u64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{value} {unit}s ago")
    }
}
