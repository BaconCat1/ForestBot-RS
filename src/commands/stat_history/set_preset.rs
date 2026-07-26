use crate::commands::{CommandContext, CommandFuture, enqueue_chat};

command!(SET_PRESET_COMMAND, &["setpreset"], "Sets the namechalk preset, only on refinedvanilla. Usage: {prefix}setpreset <preset>", set_preset);

fn set_preset(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(preset) = ctx.args.first() else {
            return Ok(());
        };
        enqueue_chat(&ctx.state, format!("/nc preset {preset}"));
        // Doesn't echo the preset back -- no validation against NameChalk's own preset
        // list here (invalid presets are on the player to get right), so this used to
        // blindly repeat whatever they typed into public chat and claim success either
        // way. Generic confirmation instead, censorship toggled off since there's
        // nothing user-authored left to censor.
        ctx.chat_success(" Preset request sent.".to_owned());
        Ok(())
    })
}
