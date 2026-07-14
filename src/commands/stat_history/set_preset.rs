use crate::commands::{CommandContext, CommandFuture, enqueue_chat};

command!(SET_PRESET_COMMAND, &["setpreset"], "Sets the namechalk preset, only on refinedvanilla. Usage: {prefix}setpreset <preset>", set_preset);

fn set_preset(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(preset) = ctx.args.first() else {
            return Ok(());
        };
        enqueue_chat(&ctx.state, format!("/nc preset {preset}"));
        ctx.chat(format!(" Set the preset {preset} successfully!"));
        Ok(())
    })
}
