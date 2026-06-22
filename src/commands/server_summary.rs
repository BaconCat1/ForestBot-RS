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

fn format_summary(data: &serde_json::Value) -> String {
    let server = data["server"].as_str().unwrap_or("?");
    let players = data["total_players"].as_u64().unwrap_or(0);
    let msgs = data["total_messages"].as_u64().unwrap_or(0);
    let kills = data["total_kills"].as_u64().unwrap_or(0);
    let deaths = data["total_deaths"].as_u64().unwrap_or(0);
    let hrs = data["total_playtime_ms"].as_u64().unwrap_or(0) / 3_600_000;
    let top = data["top_chatter"]["name"].as_str().unwrap_or("?");
    let top_count = data["top_chatter"]["count"].as_u64().unwrap_or(0);
    let since = parse_ms(&data["tracking_since"])
        .map(format_month_year)
        .unwrap_or_else(|| "?".to_owned());

    format!(
        "[{server}] {} players | {} msgs | {} kills | {} deaths | {} hrs | top msgs: {} ({}) | since {}",
        fmt_num(players),
        fmt_num(msgs),
        fmt_num(kills),
        fmt_num(deaths),
        fmt_num(hrs),
        top,
        fmt_num(top_count),
        since,
    )
}

fn format_compare(a: &serde_json::Value, b: &serde_json::Value) -> String {
    let sa = a["server"].as_str().unwrap_or("?");
    let sb = b["server"].as_str().unwrap_or("?");

    let players_a = a["total_players"].as_u64().unwrap_or(0);
    let players_b = b["total_players"].as_u64().unwrap_or(0);
    let msgs_a = a["total_messages"].as_u64().unwrap_or(0);
    let msgs_b = b["total_messages"].as_u64().unwrap_or(0);
    let kills_a = a["total_kills"].as_u64().unwrap_or(0);
    let kills_b = b["total_kills"].as_u64().unwrap_or(0);
    let deaths_a = a["total_deaths"].as_u64().unwrap_or(0);
    let deaths_b = b["total_deaths"].as_u64().unwrap_or(0);
    let hrs_a = a["total_playtime_ms"].as_u64().unwrap_or(0) / 3_600_000;
    let hrs_b = b["total_playtime_ms"].as_u64().unwrap_or(0) / 3_600_000;
    let top_a = a["top_chatter"]["name"].as_str().unwrap_or("?");
    let top_b = b["top_chatter"]["name"].as_str().unwrap_or("?");
    let since_a = parse_ms(&a["tracking_since"])
        .map(format_month_year)
        .unwrap_or_else(|| "?".to_owned());
    let since_b = parse_ms(&b["tracking_since"])
        .map(format_month_year)
        .unwrap_or_else(|| "?".to_owned());

    format!(
        "{sa} vs {sb} | {}/{} players | {}/{} msgs | {}/{} kills | {}/{} deaths | {}/{} hrs | top msgs: {top_a}/{top_b} | since {since_a}/{since_b}",
        fmt_num(players_a), fmt_num(players_b),
        fmt_num(msgs_a),    fmt_num(msgs_b),
        fmt_num(kills_a),   fmt_num(kills_b),
        fmt_num(deaths_a),  fmt_num(deaths_b),
        fmt_num(hrs_a),     fmt_num(hrs_b),
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
