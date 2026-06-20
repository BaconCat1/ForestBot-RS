use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::commands;
use crate::structure::logger;
use crate::structure::mineflayer::bot::AzaleaState;

pub fn spawn_announce_loop(state: AzaleaState, active: Arc<AtomicBool>) {
    tokio::spawn(async move {
        // TS: getRandomInterval() = floor(random * 1_800_001) + 900_000 ms (15–45 min, chosen once)
        // In debug mode, fire every 10s for easy testing
        let interval_ms = if std::env::var("ANNOUNCE_FAST").is_ok() {
            10_000
        } else {
            900_000 + pseudo_rand(now_nanos(), 0) % 1_800_001
        };
        let mut used_indices: Vec<usize> = Vec::new();

        loop {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;

            if !active.load(Ordering::Relaxed) {
                break;
            }

            let (prefix, command_toggles) = {
                let runtime = state.runtime.read().expect("runtime config lock poisoned");
                (runtime.prefix.clone(), runtime.command_toggles.clone())
            };

            let candidates: Vec<usize> = commands::registry()
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    if cmd.whitelisted || cmd.description.is_empty() {
                        return false;
                    }
                    !cmd.names
                        .iter()
                        .any(|n| command_toggles.get(*n).copied() == Some(false))
                })
                .map(|(i, _)| i)
                .collect();

            if candidates.is_empty() {
                continue;
            }

            // Cycle through all before repeating (matches TS usedIndices behavior)
            used_indices.retain(|i| candidates.contains(i));
            if used_indices.len() >= candidates.len() {
                used_indices.clear();
            }

            let remaining: Vec<usize> = candidates
                .iter()
                .filter(|i| !used_indices.contains(i))
                .copied()
                .collect();

            let pick = pseudo_rand(now_nanos(), 1) as usize % remaining.len();
            let chosen_idx = remaining[pick];
            used_indices.push(chosen_idx);

            let cmd = &commands::registry()[chosen_idx];
            let description = cmd.description.replace("{prefix}", &prefix);
            logger::info(format!("Announce: {description}"));

            state
                .outbound_chat
                .lock()
                .expect("outbound chat queue lock poisoned")
                .push_back(description);
        }
    });
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
}

fn pseudo_rand(nanos: u64, salt: u64) -> u64 {
    nanos
        .wrapping_mul(6364136223846793005_u64)
        .wrapping_add(salt.wrapping_mul(1442695040888963407_u64))
}
