pub mod stats_target;

use crate::structure::{
    endpoints::endpoints::ContentFlaggedData,
    logger,
    mineflayer::{bot::AzaleaState, utils::profanity_filter::is_severely_flagged},
};

// Spawns off the handler's call stack to avoid stack overflow in Azalea's event loop.
pub fn flag_content_if_needed(state: &AzaleaState, username: &str, command: &str, content: &str) {
    logger::debug_cat("content_flag","flag_content_if_needed: entered");
    let username = username.to_owned();
    logger::debug_cat("content_flag","flag_content_if_needed: cloned username");
    let command = command.to_owned();
    let content = content.to_owned();
    let mc_server = state.mc_server.clone();
    logger::debug_cat("content_flag","flag_content_if_needed: cloned mc_server");
    let ws = state.api.websocket.clone();
    let trie = state.profanity_trie.clone();
    logger::debug_cat("content_flag","flag_content_if_needed: cloned ws, about to spawn");
    tokio::spawn(async move {
        logger::debug_cat("content_flag","flag_content_if_needed: inside spawn, checking trie");
        let Some(trie) = *trie.read().expect("profanity_trie read") else {
            logger::debug_cat("content_flag","flag_content_if_needed: trie not loaded yet, skipping");
            return;
        };
        if !is_severely_flagged(trie, &content) {
            logger::debug_cat("content_flag","flag_content_if_needed: not severely flagged, returning");
            return;
        }
        logger::debug_cat("content_flag","flag_content_if_needed: severe content found, sending WS event");
        let Some(ws) = ws else {
            logger::debug_cat("content_flag","flag_content_if_needed: no WS client, aborting");
            return;
        };
        if let Err(e) = ws.send_content_flagged(ContentFlaggedData {
            username,
            mc_server,
            command,
            content,
        }).await {
            logger::warn(format!("content_flagged send failed: {e}"));
        } else {
            logger::debug_cat("content_flag","flag_content_if_needed: WS event sent OK");
        }
    });
    logger::debug_cat("content_flag","flag_content_if_needed: spawn returned, exiting");
}
