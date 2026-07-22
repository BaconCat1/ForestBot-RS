use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

use super::{chips_str, fmt_time, calc_payout};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["bets", "mybets", "eventbets"],
    description: "List all your open event bets (AQI, launch, gas, sports, kalshi, seismic, floods, etc.) Usage: {prefix}!bets",
    whitelisted: false,
    execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

        let db_rows = ctx.state.api.casino_event_bets_list(&player_uuid).await;

        let gas_bets: Vec<String> = {
            let map = ctx.state.gas_bets.lock().unwrap();
            map.get(&player_uuid).map(|v| v.iter().map(|b| {
                let payout = (b.stake as f64 * 10000.0 / b.price as f64).floor() as i64;
                format!("[GAS] {} {} (base ${:.3}) {} → pays {} | T-{}",
                    b.region, b.side.to_uppercase(), b.baseline as f64 / 1000.0,
                    chips_str(b.stake), chips_str(payout), fmt_time(b.close_time))
            }).collect()).unwrap_or_default()
        };

        let weather_bets: Vec<String> = {
            let map = ctx.state.weather_bets.lock().expect("weather_bets lock");
            map.get(&player_uuid).map(|v| v.iter().map(|b| {
                let type_str = match b.bet_type.as_str() {
                    "rain" => format!("rain {}", b.direction.to_uppercase()),
                    _ => format!("{} {} {:.1}{}",
                        b.bet_type.to_uppercase(), b.direction.to_uppercase(),
                        b.threshold.unwrap_or(0.0),
                        b.unit.as_deref().unwrap_or("")),
                };
                let payout = (b.stake as f64 * b.payout_mult).ceil() as i64;
                format!("[WEATHER] {} {} {} → pays {} | T-{}",
                    b.city, type_str, chips_str(b.stake), chips_str(payout), fmt_time(b.closes_unix))
            }).collect()).unwrap_or_default()
        };

        let total = db_rows.len() + gas_bets.len() + weather_bets.len();
        if total == 0 {
            ctx.whisper_success("No open event bets.");
            return Ok(());
        }

        ctx.whisper_success(format!("{} open event bet{}:", total, if total == 1 { "" } else { "s" }));

        for row in &db_rows {
            let bet_type   = row["bet_type"].as_str().unwrap_or("?");
            let stake      = row["stake"].as_i64().unwrap_or(0);
            let price      = row["price"].as_f64().unwrap_or(1.0);
            let close_time = row["close_time"].as_u64().unwrap_or(0);
            let payout     = calc_payout(stake, price);
            let label      = describe_bet(bet_type, row);
            let bracket = match bet_type {
                "launch"      => "ROCKET".to_owned(),
                "nasa"        => "SPACEWX".to_owned(),
                "join_window" => "JOINS".to_owned(),
                other         => other.to_uppercase(),
            };
            ctx.whisper_success(format!(
                "[{}] {} {} → pays {} | T-{}",
                bracket, label, chips_str(stake), chips_str(payout),
                fmt_time(close_time),
            ));
        }
        for line in gas_bets { ctx.whisper_success(line); }
        for line in weather_bets { ctx.whisper_success(line); }

        Ok(())
    })
}

// `row`'s columns are the polymorphic `casino_event_bets` table's real columns -- most
// types just reuse `train_name`/`location` for their own display name/location fields
// (documented in Hub's betTypeConfig.ts), which is why the same column name means a
// different thing per branch below. `gas` is deliberately never seen here -- excluded
// at the SQL level in getEventBets.ts since it has its own dedicated display path.
fn describe_bet(bet_type: &str, row: &serde_json::Value) -> String {
    let side = row["side"].as_str().unwrap_or("?");
    match bet_type {
        "aqi" => {
            // location = zip, train_name = reporting area
            let zip  = row["location"].as_str().unwrap_or("?");
            let area = row["train_name"].as_str().unwrap_or("");
            format!("{zip}/{area} {}", side.to_uppercase())
        }
        "launch" => {
            // location = launch id (truncated for display), train_name = launch name
            let name = row["train_name"].as_str().unwrap_or("?");
            let short = row["location"].as_str().map(|s| &s[..s.len().min(8)]).unwrap_or("?");
            format!("[{}] {} {}", short, &name[..name.len().min(20)], side.to_uppercase())
        }
        "nasa" => {
            let subtype = row["nasa_subtype"].as_str().unwrap_or("?");
            subtype.to_uppercase()
        }
        "join_window" => {
            // location = subject_uuid, train_name = subject_name. No `side` -- single outcome.
            let subject = row["train_name"].as_str().unwrap_or("?");
            format!("{subject} logs in")
        }
        _ => {
            // train_name = display name for every other type (train, faa, noaa, kalshi,
            // quake, volcano, sports) -- see betTypeConfig.ts's getFieldMapping per type.
            let name = row["train_name"].as_str().unwrap_or(bet_type);
            format!("{} {}", &name[..name.len().min(25)], side.to_uppercase())
        }
    }
}
