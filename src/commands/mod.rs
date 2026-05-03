use std::{future::Future, pin::Pin};

use azalea::prelude::Client;

pub mod discord;
pub mod help;
pub mod joins;
pub mod lastseen;
pub mod msgcount;
pub mod ping;
pub mod playtime;
pub mod quote;
pub mod reload;
pub mod utils;

use crate::structure::mineflayer::bot::{AzaleaState, RuntimeConfig};

pub type CommandFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
pub type CommandExecutor = for<'a> fn(CommandContext<'a>) -> CommandFuture<'a>;

#[derive(Clone, Copy)]
pub struct CommandDefinition {
    pub names: &'static [&'static str],
    pub whitelisted: bool,
    pub execute: CommandExecutor,
}

pub struct CommandContext<'a> {
    pub bot: &'a Client,
    pub state: &'a AzaleaState,
    pub runtime: &'a RuntimeConfig,
    pub sender: &'a str,
    pub args: Vec<&'a str>,
}

pub fn registry() -> &'static [CommandDefinition] {
    &[
        ping::COMMAND,
        help::COMMAND,
        discord::COMMAND,
        reload::COMMAND,
        lastseen::COMMAND,
        msgcount::COMMAND,
        playtime::COMMAND,
        joins::COMMAND,
        quote::COMMAND,
    ]
}

pub fn find(command_name: &str) -> Option<&'static CommandDefinition> {
    registry().iter().find(|command| {
        command
            .names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(command_name))
    })
}
