use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::{AzaleaState, TriviaPhase, TriviaRound};

use super::casino::chips_str;

pub const TRIVIA_COMMAND: CommandDefinition = CommandDefinition {
    names: &["trivia"],
    description: "Chip-staked trivia. !trivia <chips> [category] | !trivia join | !trivia categories",
    whitelisted: false,
    execute: execute_trivia,
};

pub const ANSWER_COMMAND: CommandDefinition = CommandDefinition {
    names: &["answer"],
    description: "Answer the active trivia question. Usage: {prefix}answer <A/B/C/D or true/false>",
    whitelisted: false,
    execute: execute_answer,
};

const MIN_BET: i64 = 50;
const JOIN_SECS: u64 = 30;
const ANSWER_SECS: u64 = 45;

fn execute_trivia(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg = ctx.args.first().copied().unwrap_or("").to_lowercase();

        match arg.as_str() {
            "categories" => {
                ctx.whisper(format!("Categories: {CATEGORIES_LIST}"));
                return Ok(());
            }
            "join" => {
                return join_round(&ctx).await;
            }
            _ => {}
        }

        // Start a new round
        {
            let lock = ctx.state.active_trivia.lock().expect("trivia lock");
            if lock.as_ref().is_some_and(|r| r.phase != TriviaPhase::Closed) {
                ctx.whisper("Trivia round already in progress!");
                return Ok(());
            }
        }

        // Syntax: !trivia <chips> OR !trivia <category> <chips>
        let (stake, category, category_arg) = if let Ok(n) = arg.parse::<i64>() {
            (n, None, String::new())
        } else {
            let cat_name = arg.clone();
            let chips_str_arg = ctx.args.get(1).copied().unwrap_or("");
            let n: i64 = match chips_str_arg.parse() {
                Ok(n) => n,
                Err(_) => {
                    ctx.whisper("Usage: !trivia <chips> | !trivia <category> <chips> | !trivia categories");
                    return Ok(());
                }
            };
            match category_id(&cat_name) {
                Some(id) => (n, Some(id), cat_name),
                None => {
                    ctx.whisper(format!("Unknown category '{}'. Try: !trivia categories", cat_name));
                    return Ok(());
                }
            }
        };

        if stake < MIN_BET {
            ctx.whisper(format!("Min wager is {}.", chips_str(MIN_BET)));
            return Ok(());
        }

        // Deduct stake from starter
        match ctx.state.api.casino_adjust(ctx.sender, -stake).await {
            Ok(_) => {}
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper("Casino unavailable.");
                return Ok(());
            }
        }

        // Fetch question now, reveal after join window
        let (round, question_msg) = match fetch_question(category).await {
            Some(r) => r,
            None => {
                // Refund on fetch failure
                let _ = ctx.state.api.casino_adjust(ctx.sender, stake).await;
                ctx.whisper("Failed to fetch trivia question, try again.");
                return Ok(());
            }
        };

        let sender = ctx.sender.to_owned();
        {
            let mut lock = ctx.state.active_trivia.lock().expect("trivia lock");
            let mut r = round;
            r.stake = stake;
            r.participants.insert(sender.clone());
            *lock = Some(r);
        }

        let cat_str = if category_arg.is_empty() { String::new() } else { format!(" [{}]", category_arg) };
        ctx.chat(format!(
            "[Trivia{cat_str}] {sender} started a round! Wager: {}. Type !trivia join within {JOIN_SECS}s to join.",
            chips_str(stake)
        ));

        spawn_join_timer(ctx.state.clone(), question_msg, stake, JOIN_SECS);
        Ok(())
    })
}

async fn join_round(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let sender = ctx.sender.to_owned();

    enum JoinOutcome {
        NoRound,
        NotJoining,
        AlreadyJoined,
        NeedDeduct { stake: i64 },
    }

    let outcome = {
        let lock = ctx.state.active_trivia.lock().expect("trivia lock");
        match lock.as_ref() {
            None => JoinOutcome::NoRound,
            Some(round) if round.phase != TriviaPhase::Joining => JoinOutcome::NotJoining,
            Some(round) if round.participants.contains(&sender) => JoinOutcome::AlreadyJoined,
            Some(round) => JoinOutcome::NeedDeduct { stake: round.stake },
        }
    };

    match outcome {
        JoinOutcome::NoRound => {
            ctx.whisper("No trivia round accepting players right now.");
        }
        JoinOutcome::NotJoining => {
            ctx.whisper("Join window is closed — question is already live.");
        }
        JoinOutcome::AlreadyJoined => {
            ctx.whisper("Already joined this round!");
        }
        JoinOutcome::NeedDeduct { stake } => {
            match ctx.state.api.casino_adjust(ctx.sender, -stake).await {
                Ok(_) => {}
                Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                    ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
                    return Ok(());
                }
                Err(CasinoAdjustErr::NetworkErr) => {
                    ctx.whisper("Casino unavailable.");
                    return Ok(());
                }
            }
            {
                let mut lock = ctx.state.active_trivia.lock().expect("trivia lock");
                if let Some(round) = lock.as_mut() {
                    round.participants.insert(sender.clone());
                }
            }
            ctx.chat(format!("{} joined the trivia round!", sender));
        }
    }

    Ok(())
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
            NotOpen(String),
            NotParticipant,
            AlreadyAnswered,
            Correct,
            Wrong,
        }

        let outcome = {
            let mut lock = ctx.state.active_trivia.lock().expect("trivia lock");
            match lock.as_mut() {
                None => Outcome::NoRound,
                Some(round) if round.phase == TriviaPhase::Joining => {
                    Outcome::NotOpen("Question isn't live yet — wait for the join window to close.".to_owned())
                }
                Some(round) if round.phase == TriviaPhase::Closed => {
                    Outcome::NotOpen(format!("Too late! Answer was: {}", round.correct_answer))
                }
                Some(round) => {
                    if !round.participants.contains(&sender) {
                        Outcome::NotParticipant
                    } else if round.answered.contains(&sender) {
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
            Outcome::NoRound => ctx.whisper("No active trivia round."),
            Outcome::NotOpen(msg) => ctx.whisper(msg),
            Outcome::NotParticipant => ctx.whisper("You didn't join this round. Wait for the next one!"),
            Outcome::AlreadyAnswered => ctx.whisper("Already answered this round!"),
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

// ── Timers ────────────────────────────────────────────────────────────────────

fn spawn_join_timer(state: AzaleaState, question_msg: String, stake: i64, delay_secs: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;

        let participant_count = {
            let mut lock = state.active_trivia.lock().expect("trivia lock");
            let Some(round) = lock.as_mut() else { return; };
            if round.phase != TriviaPhase::Joining { return; }

            if round.participants.is_empty() {
                lock.take();
                drop(lock);
                state.outbound_chat.lock().expect("outbound lock")
                    .push_back("[Trivia] Nobody joined — cancelled.".to_owned());
                return;
            }

            round.phase = TriviaPhase::Open;
            round.participants.len()
        };

        state.outbound_chat.lock().expect("outbound lock")
            .push_back(format!("[Trivia] {participant_count} player(s) — {}", question_msg));

        spawn_answer_timer(state, stake, ANSWER_SECS);
    });
}

fn spawn_answer_timer(state: AzaleaState, stake: i64, delay_secs: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;

        let (summary, correct_players, wrong_players, no_answer_players) = {
            let mut lock = state.active_trivia.lock().expect("trivia lock");
            let Some(round) = lock.as_mut() else { return; };
            if round.phase != TriviaPhase::Open { return; }
            round.phase = TriviaPhase::Closed;
            let summary = build_summary(round);
            let correct = round.correct_players.clone();
            let wrong = round.wrong_players.clone();
            let no_answer: Vec<String> = round.participants.iter()
                .filter(|p| !round.answered.contains(*p))
                .cloned()
                .collect();
            (summary, correct, wrong, no_answer)
        };

        // Refund 2× to correct players
        for player in &correct_players {
            let _ = state.api.casino_adjust(player, stake * 2).await;
        }
        // Wrong + no-answer players' stakes go to jackpot (already deducted at join)
        let forfeited = (wrong_players.len() + no_answer_players.len()) as i64;
        if forfeited > 0 {
            let _ = state.api.casino_jackpot_rake(stake * forfeited).await;
        }

        {
            let mut out = state.outbound_chat.lock().expect("outbound lock");
            out.push_back(summary);
            for player in &correct_players {
                out.push_back(format!("/msg {player} [Trivia] Correct! +{}.", chips_str(stake)));
            }
            for player in &wrong_players {
                out.push_back(format!("/msg {player} [Trivia] Wrong! -{} (to jackpot).", chips_str(stake)));
            }
            for player in &no_answer_players {
                out.push_back(format!("/msg {player} [Trivia] No answer — -{} (to jackpot).", chips_str(stake)));
            }
        }

        state.active_trivia.lock().expect("trivia lock").take();
    });
}

// ── Question fetch ─────────────────────────────────────────────────────────────

async fn fetch_question(category: Option<u32>) -> Option<(TriviaRound, String)> {
    let url = match category {
        Some(id) => format!("https://opentdb.com/api.php?amount=1&encode=url3986&category={id}"),
        None => "https://opentdb.com/api.php?amount=1&encode=url3986".to_owned(),
    };

    let resp: serde_json::Value = reqwest::Client::new()
        .get(&url)
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
        let prefix = "True or False: ";
        let budget = 255usize.saturating_sub(prefix.len() + suffix.len() + 1);
        let question_msg = format!("{prefix}{}?{suffix}", truncate_str(&question, budget));

        let round = TriviaRound {
            correct_answer: correct,
            is_boolean: true,
            letter_map: Vec::new(),
            correct_letter: None,
            question_msg: question_msg.clone(),
            phase: TriviaPhase::Joining,
            stake: 0,
            participants: HashSet::new(),
            correct_players: Vec::new(),
            wrong_players: Vec::new(),
            answered: HashSet::new(),
        };
        Some((round, question_msg))
    } else {
        let incorrect: Vec<String> = item["incorrect_answers"]
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(url_decode))
            .collect();

        let mut answers: Vec<String> = incorrect;
        answers.push(correct.clone());
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as usize;
        shuffle_seeded(&mut answers, seed);

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
        let prefix = "";
        let choices_chars = choices_str.chars().count();
        let q_budget = 255usize.saturating_sub(prefix.len() + 1 + choices_chars + suffix.len());
        let question_msg = format!(
            "{}{}?{choices_str}{suffix}",
            prefix,
            truncate_str(&question, q_budget)
        );

        let round = TriviaRound {
            correct_answer: correct,
            is_boolean: false,
            letter_map,
            correct_letter: Some(correct_letter),
            question_msg: question_msg.clone(),
            phase: TriviaPhase::Joining,
            stake: 0,
            participants: HashSet::new(),
            correct_players: Vec::new(),
            wrong_players: Vec::new(),
            answered: HashSet::new(),
        };
        Some((round, question_msg))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

    let no_answers = round.participants.iter()
        .filter(|p| !round.answered.contains(*p))
        .cloned()
        .collect::<Vec<_>>();

    if round.correct_players.is_empty() && round.wrong_players.is_empty() {
        return truncate_str(
            &format!("[Trivia] Nobody answered! Was: {answer_display}"),
            255,
        );
    }

    let mut msg = String::from("[Trivia]");
    if !round.correct_players.is_empty() {
        msg.push_str(&format!(" ✓ {}", truncate_list(&round.correct_players, 70)));
    }
    if !round.wrong_players.is_empty() {
        msg.push_str(&format!(" ✗ {}", truncate_list(&round.wrong_players, 35)));
    }
    if !no_answers.is_empty() {
        msg.push_str(&format!(" ∅ {}", truncate_list(&no_answers, 30)));
    }
    msg.push_str(&format!(" | Was: {answer_display}"));

    if msg.chars().count() > 255 {
        format!("[Trivia] Was: {answer_display}")
    } else {
        msg
    }
}

fn category_id(name: &str) -> Option<u32> {
    Some(match name {
        "general" | "general knowledge" => 9,
        "books" | "book" => 10,
        "film" | "films" | "movie" | "movies" => 11,
        "music" => 12,
        "musical" | "musicals" | "theatre" | "theater" => 13,
        "tv" | "television" | "shows" => 14,
        "games" | "video games" | "videogames" | "gaming" => 15,
        "board games" | "boardgames" => 16,
        "science" | "nature" => 17,
        "computers" | "computer" | "tech" | "technology" => 18,
        "math" | "maths" | "mathematics" => 19,
        "mythology" | "myth" => 20,
        "sports" | "sport" => 21,
        "geography" | "geo" => 22,
        "history" => 23,
        "politics" | "political" => 24,
        "art" | "arts" => 25,
        "celebrities" | "celeb" | "celebs" => 26,
        "animals" | "animal" => 27,
        "vehicles" | "vehicle" | "cars" => 28,
        "comics" | "comic" => 29,
        "gadgets" | "gadget" => 30,
        "anime" | "manga" => 31,
        "cartoons" | "cartoon" | "animation" => 32,
        _ => return None,
    })
}

const CATEGORIES_LIST: &str = "general, books, film, music, musicals, tv, games, board games, science, computers, math, mythology, sports, geography, history, politics, art, celebrities, animals, vehicles, comics, gadgets, anime, cartoons";

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

fn shuffle_seeded(v: &mut Vec<String>, seed: usize) {
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
