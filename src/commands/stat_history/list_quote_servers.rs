use crate::commands::{CommandContext, CommandFuture};

command!(
    LIST_QUOTE_SERVERS_COMMAND,
    &["lq", "listquoteservers"],
    "Lists servers you can quote from. Usage: {prefix}lq",
    list_quote_servers
);

fn list_quote_servers(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let servers = std::iter::once("all")
            .chain(
                crate::constants::quote_servers::QUOTE_SERVERS
                    .iter()
                    .copied(),
            )
            .collect::<Vec<_>>();
        let chunks = quote_server_chunks(&servers);
        if chunks.len() == 1 {
            ctx.chat_success(format!(" {}", chunks[0]));
        } else {
            for chunk in chunks {
                ctx.whisper_success(format!(" {chunk}"));
            }
        }
        Ok(())
    })
}

fn quote_server_chunks(servers: &[&str]) -> Vec<String> {
    const MAX_MESSAGE_LENGTH: usize = 230;
    const CONTINUATION_PREFIX: &str = "More: ";

    let intro = format!("Quotable servers ({}): ", servers.len());
    let mut chunks = Vec::new();
    let mut current = intro;
    let mut has_server_in_chunk = false;

    for server in servers {
        let separator = if has_server_in_chunk { ", " } else { "" };
        let next = format!("{current}{separator}{server}");

        if next.len() > MAX_MESSAGE_LENGTH {
            chunks.push(current);
            current = format!("{CONTINUATION_PREFIX}{server}");
            has_server_in_chunk = true;
            continue;
        }

        current = next;
        has_server_in_chunk = true;
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
