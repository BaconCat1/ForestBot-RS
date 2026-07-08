use anyhow::Result;

use crate::commands::stat_history;
use crate::structure::mineflayer::bot::Bot;

#[allow(dead_code)]
pub async fn handle(_bot: &mut Bot, username: &str) -> Result<()> {
    stat_history::clear_delete_faq_pending(username);
    Ok(())
}
