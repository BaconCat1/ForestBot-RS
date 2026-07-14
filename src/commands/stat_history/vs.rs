use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(VS_COMMAND, &["vs"], "Head-to-head stat comparison between two players. Usage: {prefix}vs <player1> <player2>", vs);

fn vs(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let [name1, name2] = match ctx.args.as_slice() {
            [name1, name2] => [*name1, *name2],
            _ => {
                whisper(&ctx, &format!(" Usage: {}vs <player1> <player2>", ctx.runtime.prefix));
                return Ok(());
            }
        };
        let (uuid1, uuid2) = tokio::join!(
            ctx.state.api.convert_username_to_uuid(name1),
            ctx.state.api.convert_username_to_uuid(name2)
        );
        let (Some(uuid1), Some(uuid2)) = (uuid1, uuid2) else {
            whisper(&ctx, " Could not resolve one or both usernames.");
            return Ok(());
        };
        let (kd1, kd2, pt1, pt2, mc1, mc2) = tokio::join!(
            ctx.state.api.get_kd(&uuid1, &ctx.state.mc_server),
            ctx.state.api.get_kd(&uuid2, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid1, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid2, &ctx.state.mc_server),
            ctx.state.api.get_message_count(name1, &ctx.state.mc_server),
            ctx.state.api.get_message_count(name2, &ctx.state.mc_server)
        );
        let (kills1, deaths1) = kd1.map(|kd| (kd.kills, kd.deaths)).unwrap_or_default();
        let (kills2, deaths2) = kd2.map(|kd| (kd.kills, kd.deaths)).unwrap_or_default();
        let kdr1 = if deaths1 > 0 {
            kills1 as f64 / deaths1 as f64
        } else {
            kills1 as f64
        };
        let kdr2 = if deaths2 > 0 {
            kills2 as f64 / deaths2 as f64
        } else {
            kills2 as f64
        };
        let pt_days1 = pt1.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let pt_days2 = pt2.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let msgs1 = mc1.map(|mc| mc.message_count).unwrap_or_default();
        let msgs2 = mc2.map(|mc| mc.message_count).unwrap_or_default();
        ctx.chat(format!(
            " [VS] {name1} vs {name2} | K: {kills1} {} {kills2} | D: {deaths1} {} {deaths2} | KD: {kdr1:.2} {} {kdr2:.2} | PT: {pt_days1}d {} {pt_days2}d | Msgs: {msgs1} {} {msgs2}",
            compare_u64(kills1, kills2),
            compare_u64(deaths2, deaths1),
            compare_f64(kdr1, kdr2),
            compare_u64(pt_days1, pt_days2),
            compare_u64(msgs1, msgs2),
        ));
        Ok(())
    })
}

fn compare_u64(left: u64, right: u64) -> &'static str {
    match left.cmp(&right) {
        std::cmp::Ordering::Greater => ">",
        std::cmp::Ordering::Less => "<",
        std::cmp::Ordering::Equal => "=",
    }
}

fn compare_f64(left: f64, right: f64) -> &'static str {
    if left > right {
        ">"
    } else if left < right {
        "<"
    } else {
        "="
    }
}
