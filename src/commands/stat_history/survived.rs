use super::helpers::{epoch_ms_from_string, now_millis, whisper, whisper_no_record};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;

command!(SURVIVED_COMMAND, &["survived"], "Shows how long since a user's last death. Usage: {prefix}survived <username>", survived);

fn survived(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper_no_record(&ctx, search, "deaths");
            return Ok(());
        };
        let death = ctx
            .state
            .api
            .get_deaths(&uuid, &ctx.state.mc_server, 1, "DESC", "all")
            .await
            .and_then(|mut rows| rows.pop());
        let Some(death) = death else {
            whisper_no_record(&ctx, search, "deaths");
            return Ok(());
        };
        let Some(death_ms) = epoch_ms_from_string(&death.time.to_string()) else {
            whisper(&ctx, &format!(" Unable to determine last death time for {search}."));
            return Ok(());
        };
        let survived = time::dhms(now_millis().saturating_sub(death_ms))
            .trim_end_matches('.')
            .to_owned();
        ctx.chat(format!(
            " {search} has survived for {survived} since their last death."
        ));
        Ok(())
    })
}
