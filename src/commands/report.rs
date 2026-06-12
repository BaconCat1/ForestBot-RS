use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const REPORT_COMMAND: CommandDefinition = CommandDefinition {
    names: &["report"],
    description: "Reports a scammer. Usage: {prefix}report <player> <reason>",
    whitelisted: false,
    execute: execute_report,
};

pub fn execute_report(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(&format!("Usage: {}report <player> [reason]", ctx.runtime.prefix));
            return Ok(());
        }

        let target_name = ctx.args[0];
        let reason = if ctx.args.len() > 1 {
            ctx.args[1..].join(" ")
        } else {
            "No reason provided".to_owned()
        };

        let reporter_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            players.get(ctx.sender).map(|p| p.uuid.clone())
        };
        let reporter_uuid = match reporter_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
                Some(u) => u,
                None => {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                }
            },
        };

        let reported_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            players.get(target_name).map(|p| p.uuid.clone())
        };
        let reported_uuid = match reported_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(target_name).await {
                Some(u) => u,
                None => {
                    ctx.whisper(format!("Could not find player: {target_name}"));
                    return Ok(());
                }
            },
        };

        if reporter_uuid == reported_uuid {
            ctx.whisper("You cannot report yourself.");
            return Ok(());
        }

        let server = ctx.state.mc_server.clone();
        if ctx.state.api.tradebot_report_user(&reporter_uuid, &reported_uuid, &reason, &server).await {
            ctx.whisper(format!(
                "Report submitted against {target_name}. A moderator will review it shortly."
            ));
        } else {
            ctx.whisper("Failed to submit report. Try again later.");
        }

        Ok(())
    })
}
