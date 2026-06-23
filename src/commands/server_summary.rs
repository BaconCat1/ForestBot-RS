use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use chrono::{TimeZone, Utc};

pub const SERVER_SUMMARY_COMMAND: CommandDefinition = CommandDefinition {
    names: &["serversummary", "ssummary"],
    description: "Server stats. Usage: {prefix}ssummary <server>",
    whitelisted: false,
    execute: execute_server_summary,
};

pub const COMPARE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["compare"],
    description: "Compare two servers side by side. Usage: {prefix}compare <serverA> <serverB>",
    whitelisted: false,
    execute: execute_compare,
};

fn execute_server_summary(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}ssummary <server>", ctx.runtime.prefix));
            return Ok(());
        }
        let server = ctx.args[0];
        match ctx.state.api.get_server_summary(server).await {
            Some(data) => ctx.chat(format_summary(&data)),
            None => ctx.chat(format!("No data found for server: {server}")),
        }
        Ok(())
    })
}

fn execute_compare(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.len() < 2 {
            ctx.whisper(format!(
                "Usage: {}compare <serverA> <serverB>",
                ctx.runtime.prefix
            ));
            return Ok(());
        }
        let server_a = ctx.args[0].to_owned();
        let server_b = ctx.args[1].to_owned();

        let (data_a, data_b) = tokio::join!(
            ctx.state.api.get_server_summary(&server_a),
            ctx.state.api.get_server_summary(&server_b),
        );

        match (data_a, data_b) {
            (Some(a), Some(b)) => ctx.chat(format_compare(&a, &b)),
            (None, _) => ctx.chat(format!("No data found for server: {server_a}")),
            (_, None) => ctx.chat(format!("No data found for server: {server_b}")),
        }
        Ok(())
    })
}

struct ServerStats {
    server: String,
    players: u64,
    msgs: u64,
    kills: u64,
    deaths: u64,
    hrs: u64,
    top: String,
    top_count: u64,
    since: String,
}

impl ServerStats {
    fn from_json(data: &serde_json::Value) -> Self {
        Self {
            server: data["server"].as_str().unwrap_or("?").to_owned(),
            players: data["total_players"].as_u64().unwrap_or(0),
            msgs: data["total_messages"].as_u64().unwrap_or(0),
            kills: data["total_kills"].as_u64().unwrap_or(0),
            deaths: data["total_deaths"].as_u64().unwrap_or(0),
            hrs: data["total_playtime_ms"].as_u64().unwrap_or(0) / 3_600_000,
            top: data["top_chatter"]["name"].as_str().unwrap_or("?").to_owned(),
            top_count: data["top_chatter"]["count"].as_u64().unwrap_or(0),
            since: parse_ms(&data["tracking_since"])
                .map(format_month_year)
                .unwrap_or_else(|| "?".to_owned()),
        }
    }
}

fn format_summary(data: &serde_json::Value) -> String {
    let s = ServerStats::from_json(data);
    format!(
        "[{}] {} players | {} msgs | {} kills | {} deaths | {} hrs | top msgs: {} ({}) | since {}",
        s.server,
        fmt_num(s.players),
        fmt_num(s.msgs),
        fmt_num(s.kills),
        fmt_num(s.deaths),
        fmt_num(s.hrs),
        s.top,
        fmt_num(s.top_count),
        s.since,
    )
}

fn format_compare(a: &serde_json::Value, b: &serde_json::Value) -> String {
    let a = ServerStats::from_json(a);
    let b = ServerStats::from_json(b);
    format!(
        "{} vs {} | {}/{} players | {}/{} msgs | {}/{} kills | {}/{} deaths | {}/{} hrs | top msgs: {}/{} | since {}/{}",
        a.server, b.server,
        fmt_num(a.players), fmt_num(b.players),
        fmt_num(a.msgs),    fmt_num(b.msgs),
        fmt_num(a.kills),   fmt_num(b.kills),
        fmt_num(a.deaths),  fmt_num(b.deaths),
        fmt_num(a.hrs),     fmt_num(b.hrs),
        a.top, b.top,
        a.since, b.since,
    )
}

fn fmt_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 100_000 {
        format!("{}K", n / 1_000)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn parse_ms(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse().ok())
}

fn format_month_year(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    match Utc.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%b '%y").to_string(),
        None => "?".to_owned(),
    }
}
