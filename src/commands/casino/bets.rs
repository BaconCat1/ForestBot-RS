use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

use super::{chips_str, fmt_time};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["bets", "mybets", "eventbets"],
    description: "List all your open event bets (AQI, launch, gas, sports, kalshi, seismic, floods, etc.)",
    whitelisted: false,
    execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper("Could not resolve your UUID.");
            return Ok(());
        };

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

        let total = db_rows.len() + gas_bets.len();
        if total == 0 {
            ctx.whisper("No open event bets.");
            return Ok(());
        }

        ctx.whisper(format!("{} open event bet{}:", total, if total == 1 { "" } else { "s" }));

        for row in &db_rows {
            let bet_type   = row["bet_type"].as_str().unwrap_or("?");
            let stake      = row["stake"].as_i64().unwrap_or(0);
            let price      = row["price"].as_f64().unwrap_or(1.0);
            let close_time = row["close_time"].as_u64().unwrap_or(0);
            let payout     = (stake as f64 / price).floor() as i64;
            let label      = describe_bet(bet_type, row);
            let bracket = if bet_type == "launch" { "ROCKET".to_owned() } else { bet_type.to_uppercase() };
            ctx.whisper(format!(
                "[{}] {} {} → pays {} | T-{}",
                bracket, label, chips_str(stake), chips_str(payout),
                fmt_time(close_time),
            ));
        }
        for line in gas_bets { ctx.whisper(line); }

        Ok(())
    })
}

fn describe_bet(bet_type: &str, row: &serde_json::Value) -> String {
    let side = row["side"].as_str().unwrap_or("?");
    match bet_type {
        "aqi" => {
            let zip  = row["location"].as_str().unwrap_or("?");
            let area = row["train_name"].as_str().unwrap_or("");
            format!("{zip}/{area} {}", side.to_uppercase())
        }
        "launch" => {
            let name = row["train_name"].as_str().unwrap_or("?");
            let short = row["location"].as_str().map(|s| &s[..s.len().min(8)]).unwrap_or("?");
            format!("[{}] {} {}", short, &name[..name.len().min(20)], side.to_uppercase())
        }
        "gas" => {
            let region = row["train_name"].as_str().unwrap_or("?");
            let baseline = row["latitude"].as_f64().unwrap_or(0.0);
            format!("{region} {} (base ${:.3})", side.to_uppercase(), baseline)
        }
        _ => {
            let name = row["train_name"].as_str().unwrap_or(bet_type);
            format!("{} {}", &name[..name.len().min(25)], side.to_uppercase())
        }
    }
}
