use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::mineflayer::bot::{AzaleaState, TriviaRound};

pub const TRIVIA_COMMAND: CommandDefinition = CommandDefinition {
    names: &["trivia"],
    description: "Start a server trivia round — all players can !answer within 15 seconds.",
    whitelisted: false,
    execute: execute_trivia,
};

pub const ANSWER_COMMAND: CommandDefinition = CommandDefinition {
    names: &["answer"],
    description: "Answer the active trivia question. Usage: {prefix}answer <A/B/C/D or true/false>",
    whitelisted: false,
    execute: execute_answer,
};

fn execute_trivia(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        {
            let lock = ctx.state.active_trivia.lock().expect("trivia lock poisoned");
            if lock.as_ref().is_some_and(|r| r.is_open) {
                ctx.whisper("A trivia round is already in progress!");
                return Ok(());
            }
        }

        match fetch_question().await {
            Some((round, public_msg)) => {
                {
                    let mut lock = ctx.state.active_trivia.lock().expect("trivia lock poisoned");
                    *lock = Some(round);
                }
                ctx.chat(public_msg);
                spawn_trivia_timer(ctx.state, 15);
            }
            None => {
                ctx.whisper("Failed to fetch trivia question, try again.");
            }
        }

        Ok(())
    })
}

fn execute_answer(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let answer_raw = ctx.args.first().copied().unwrap_or("").to_lowercase();
        if answer_raw.is_empty() {
            ctx.whisper("Usage: !answer <A/B/C/D> or !answer <true/false>");
            return Ok(());
        }

        let sender = ctx.sender.to_owned();

        enum Outcome {
            NoRound,
            TooLate(String),
            AlreadyAnswered,
            Correct,
            Wrong,
        }

        let outcome = {
            let mut lock = ctx.state.active_trivia.lock().expect("trivia lock poisoned");
            match lock.as_mut() {
                None => Outcome::NoRound,
                Some(round) if !round.is_open => Outcome::TooLate(round.correct_answer.clone()),
                Some(round) => {
                    if round.answered.contains(&sender) {
                        Outcome::AlreadyAnswered
                    } else {
                        round.answered.insert(sender.clone());
                        if check_answer(round, &answer_raw) {
                            round.correct_players.push(sender.clone());
                            Outcome::Correct
                        } else {
                            round.wrong_players.push(sender.clone());
                            Outcome::Wrong
                        }
                    }
                }
            }
        };

        match outcome {
            Outcome::NoRound => ctx.whisper("No active trivia round. Start one with !trivia."),
            Outcome::TooLate(answer) => ctx.whisper(format!("Too late! Answer was: {answer}")),
            Outcome::AlreadyAnswered => ctx.whisper("You already answered this round!"),
            Outcome::Correct | Outcome::Wrong => ctx.whisper("Answer received!"),
        }

        Ok(())
    })
}

fn check_answer(round: &TriviaRound, input: &str) -> bool {
    if round.is_boolean {
        let norm = match input {
            "true" | "t" | "yes" | "y" => "true",
            "false" | "f" | "no" | "n" => "false",
            other => other,
        };
        norm == round.correct_answer.to_lowercase()
    } else {
        input
            .chars()
            .next()
            .map(|c| Some(c.to_ascii_uppercase()) == round.correct_letter)
            .unwrap_or(false)
    }
}

async fn fetch_question() -> Option<(TriviaRound, String)> {
    let resp: serde_json::Value = reqwest::Client::new()
        .get("https://opentdb.com/api.php?amount=1&encode=url3986")
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if resp["response_code"].as_u64() != Some(0) {
        return None;
    }

    let item = resp["results"].get(0)?;
    let question = url_decode(item["question"].as_str()?);
    let correct = url_decode(item["correct_answer"].as_str()?);
    let qtype = item["type"].as_str().unwrap_or("multiple");

    if qtype == "boolean" {
        let suffix = " (!answer true/false)";
        let prefix = "[Trivia] True or False: ";
        let budget = 255usize.saturating_sub(prefix.len() + suffix.len() + 1);
        let public_msg = format!("{prefix}{}?{suffix}", truncate_str(&question, budget));

        let round = TriviaRound {
            correct_answer: correct,
            is_open: true,
            is_boolean: true,
            letter_map: Vec::new(),
            correct_letter: None,
            correct_players: Vec::new(),
            wrong_players: Vec::new(),
            answered: HashSet::new(),
        };
        Some((round, public_msg))
    } else {
        let incorrect: Vec<String> = item["incorrect_answers"]
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(url_decode))
            .collect();

        let mut answers: Vec<String> = incorrect;
        answers.push(correct.clone());
        shuffle_by_time(&mut answers);

        let correct_idx = answers.iter().position(|a| *a == correct)?;
        let correct_letter = (b'A' + correct_idx as u8) as char;

        let letter_map: Vec<(char, String)> = answers
            .iter()
            .enumerate()
            .map(|(i, a)| ((b'A' + i as u8) as char, a.clone()))
            .collect();

        let choices_str: String = letter_map
            .iter()
            .map(|(l, a)| format!(" {l}.{}", truncate_str(a, 18)))
            .collect();

        let suffix = " (!answer A-D)";
        let prefix = "[Trivia] ";
        let choices_chars = choices_str.chars().count();
        let q_budget =
            255usize.saturating_sub(prefix.len() + 1 + choices_chars + suffix.len());
        let public_msg = format!(
            "{prefix}{}?{choices_str}{suffix}",
            truncate_str(&question, q_budget)
        );

        let round = TriviaRound {
            correct_answer: correct,
            is_open: true,
            is_boolean: false,
            letter_map,
            correct_letter: Some(correct_letter),
            correct_players: Vec::new(),
            wrong_players: Vec::new(),
            answered: HashSet::new(),
        };
        Some((round, public_msg))
    }
}

fn spawn_trivia_timer(state: &AzaleaState, delay_secs: u64) {
    let trivia = Arc::clone(&state.active_trivia);
    let outbound = Arc::clone(&state.outbound_chat);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;

        let summary = {
            let mut lock = trivia.lock().expect("trivia lock poisoned");
            let Some(round) = lock.as_mut() else {
                return;
            };
            if !round.is_open {
                return;
            }
            round.is_open = false;
            build_summary(round)
        };

        outbound
            .lock()
            .expect("outbound lock poisoned")
            .push_back(summary);

        // Keep round accessible for 60s so latecomers get the answer via whisper
        let trivia_cleanup = Arc::clone(&trivia);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let mut lock = trivia_cleanup.lock().expect("trivia lock poisoned");
            if lock.as_ref().is_some_and(|r| !r.is_open) {
                lock.take();
            }
        });
    });
}

fn build_summary(round: &TriviaRound) -> String {
    let answer_display = if round.is_boolean {
        round.correct_answer.clone()
    } else {
        match round.correct_letter {
            Some(l) => format!(
                "{l} — {}",
                round
                    .letter_map
                    .iter()
                    .find(|(c, _)| *c == l)
                    .map(|(_, a)| a.as_str())
                    .unwrap_or(&round.correct_answer)
            ),
            None => round.correct_answer.clone(),
        }
    };

    if round.correct_players.is_empty() && round.wrong_players.is_empty() {
        return truncate_str(
            &format!("[Trivia] Nobody answered! Was: {answer_display}"),
            255,
        );
    }

    let mut msg = String::from("[Trivia]");
    if !round.correct_players.is_empty() {
        msg.push_str(&format!(" ✓ {}", truncate_list(&round.correct_players, 80)));
    }
    if !round.wrong_players.is_empty() {
        msg.push_str(&format!(" ✗ {}", truncate_list(&round.wrong_players, 40)));
    }
    msg.push_str(&format!(" | Was: {answer_display}"));

    if msg.chars().count() > 255 {
        format!("[Trivia] Was: {answer_display}")
    } else {
        msg
    }
}

fn truncate_list(names: &[String], max: usize) -> String {
    truncate_str(&names.join(", "), max)
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}...", s.chars().take(max.saturating_sub(3)).collect::<String>())
    }
}

fn shuffle_by_time(v: &mut Vec<String>) {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    let n = v.len();
    for i in (1..n).rev() {
        let j = seed
            .wrapping_mul(i.wrapping_add(0x9e3779b9))
            .wrapping_add(i)
            % (i + 1);
        v.swap(i, j);
    }
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
