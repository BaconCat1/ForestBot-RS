use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use azalea::prelude::*;
use azalea::WalkDirection;

pub fn spawn_antiafk_loop(bot: Client, active: Arc<AtomicBool>) {
    tokio::spawn(async move {
        let directions = [
            WalkDirection::Forward,
            WalkDirection::Backward,
            WalkDirection::Left,
            WalkDirection::Right,
        ];
        let mut moving = false;

        loop {
            if !active.load(Ordering::Relaxed) {
                bot.walk(WalkDirection::None);
                break;
            }

            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64;

            if moving {
                bot.walk(WalkDirection::None);
                moving = false;
            } else {
                let yaw = pseudo_rand(nanos, 1) as f32 / u64::MAX as f32 * 360.0 - 180.0;
                let pitch = pseudo_rand(nanos, 2) as f32 / u64::MAX as f32 * 180.0 - 90.0;
                bot.set_direction(yaw, pitch);

                let dir_idx = (pseudo_rand(nanos, 3) % 4) as usize;
                bot.walk(directions[dir_idx]);
                moving = true;
            }

            // Random interval 2000–7000ms, matching TS 2–7s range
            let sleep_ms = 2000 + pseudo_rand(nanos, 4) % 5001;
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
    });
}

fn pseudo_rand(nanos: u64, salt: u64) -> u64 {
    nanos
        .wrapping_mul(6364136223846793005_u64)
        .wrapping_add(salt.wrapping_mul(1442695040888963407_u64))
}
