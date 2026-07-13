pub mod stats_target;

use crate::{
    config::load_word_list,
    structure::{
        endpoints::endpoints::ContentFlaggedData,
        logger,
        mineflayer::{bot::AzaleaState, utils::profanity_filter::contains_flagged_word},
    },
};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";

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
    logger::debug_cat("content_flag","flag_content_if_needed: cloned ws, about to spawn");
    tokio::spawn(async move {
        logger::debug_cat("content_flag","flag_content_if_needed: inside spawn, loading word list");
        let bad_words = load_word_list(BAD_WORDS_PATH).await.unwrap_or_default();
        logger::debug_cat("content_flag",format!("flag_content_if_needed: loaded {} words, checking content", bad_words.len()));
        if !contains_flagged_word(&content, &bad_words) {
            logger::debug_cat("content_flag","flag_content_if_needed: no flagged words, returning");
            return;
        }
        logger::debug_cat("content_flag","flag_content_if_needed: flagged word found, sending WS event");
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
