use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["poll"],
    description: "Create a poll or vote. !poll <question?> opt1, opt2 [, opt3]  |  !poll <N> to vote",
    whitelisted: false,
    execute,
};

pub struct PollState {
    pub question: String,
    pub options: Vec<String>,
    // 0-based option index → voter UUIDs (or username fallback)
    pub tally: HashMap<usize, Vec<String>>,
    pub ends_at: Instant,
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper("Usage: !poll <question?> opt1, opt2 [, opt3 ...]  |  !poll <N> to vote");
            return Ok(());
        }

        let raw = ctx.args.join(" ");

        if ctx.args.len() == 1 {
            if let Ok(n) = raw.trim().parse::<usize>() {
                return vote(&ctx, n).await;
            }
        }

        create(&ctx, &raw).await
    })
}

async fn create(ctx: &CommandContext<'_>, raw: &str) -> anyhow::Result<()> {
    {
        let guard = ctx.state.active_poll.lock().expect("active_poll lock");
        if guard.is_some() {
            ctx.whisper("Poll already active. Use !poll <N> to vote.");
            return Ok(());
        }
    }

    let Some(q_end) = raw.find('?') else {
        ctx.whisper("Usage: !poll <question?> opt1, opt2 [, opt3 ...]  |  !poll <N> to vote");
        return Ok(());
    };

    let question = raw[..=q_end].trim().to_string();
    let options_raw = raw[q_end + 1..].trim();

    let options: Vec<String> = options_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if options.len() < 2 {
        ctx.whisper("Need at least 2 options after the question.");
        return Ok(());
    }

    if options.len() > 9 {
        ctx.whisper("Max 9 options.");
        return Ok(());
    }

    let duration = Duration::from_millis(ctx.runtime.poll_duration_ms);
    let ends_at = Instant::now() + duration;

    *ctx.state.active_poll.lock().expect("active_poll lock") = Some(PollState {
        question: question.clone(),
        options: options.clone(),
        tally: HashMap::new(),
        ends_at,
    });

    let opts_display: Vec<String> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| format!("{}) {}", i + 1, opt))
        .collect();

    enqueue_chat(
        ctx.state,
        format!(
            "Poll: {}  {}  — !poll <N> to vote (2 min)",
            question,
            opts_display.join("  ")
        ),
    );

    let state = ctx.state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        let poll = {
            let mut guard = state.active_poll.lock().expect("active_poll lock");
            if guard.as_ref().map_or(false, |p| p.ends_at == ends_at) {
                guard.take()
            } else {
                None
            }
        };
        if let Some(poll) = poll {
            enqueue_chat(&state, build_results(&poll));
        }
    });

    Ok(())
}

async fn vote(ctx: &CommandContext<'_>, n: usize) -> anyhow::Result<()> {
    let uuid = ctx
        .state
        .players
        .read()
        .expect("players lock")
        .get(ctx.sender)
        .map(|p| p.uuid.clone())
        .unwrap_or_else(|| ctx.sender.to_string());

    let mut guard = ctx.state.active_poll.lock().expect("active_poll lock");
    let Some(poll) = guard.as_mut() else {
        ctx.whisper("No active poll.");
        return Ok(());
    };

    if n == 0 || n > poll.options.len() {
        ctx.whisper(format!("Invalid option. Vote 1–{}.", poll.options.len()));
        return Ok(());
    }

    let idx = n - 1;
    let mut old_idx: Option<usize> = None;

    for (opt_idx, voters) in poll.tally.iter_mut() {
        if let Some(pos) = voters.iter().position(|v| v == &uuid) {
            if *opt_idx == idx {
                ctx.whisper(format!("Already voted for: {}", poll.options[idx]));
                return Ok(());
            }
            voters.remove(pos);
            old_idx = Some(*opt_idx);
            break;
        }
    }

    poll.tally.entry(idx).or_default().push(uuid);

    if let Some(old) = old_idx {
        ctx.whisper(format!(
            "Changed vote: {} → {}",
            poll.options[old], poll.options[idx]
        ));
    } else {
        ctx.whisper(format!("Voted: {}", poll.options[idx]));
    }

    Ok(())
}

fn build_results(poll: &PollState) -> String {
    let mut entries: Vec<(&String, usize)> = poll
        .options
        .iter()
        .enumerate()
        .map(|(i, opt)| (opt, poll.tally.get(&i).map_or(0, |v| v.len())))
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    if entries.iter().all(|(_, c)| *c == 0) {
        return format!("Poll closed: {}  No one voted.", poll.question);
    }

    let parts: Vec<String> = entries
        .iter()
        .map(|(opt, count)| format!("{}: {}", opt, count))
        .collect();

    format!("Poll closed: {}  {}", poll.question, parts.join("  |  "))
}
