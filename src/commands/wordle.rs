use rand::Rng;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::casino::chips_str;

fn circled(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' => char::from_u32(0x24B6 + c as u32 - 'A' as u32).unwrap_or(c), // Ⓐ–Ⓩ, 9px uniform accented bitmap
        'a'..='z' => char::from_u32(0x24D0 + c as u32 - 'a' as u32).unwrap_or(c), // ⓐ–ⓩ, 9px uniform
        _ => c,
    }).collect()
}

fn render_matches(matches: &cl_wordle::Matches) -> String {
    matches.iter().map(|m| match m {
        cl_wordle::Match::Exact => '\u{25A3}', // ▣ correct (3.5px unifont)
        cl_wordle::Match::Close => '\u{25C8}', // ◈ wrong spot (3.5px unifont)
        cl_wordle::Match::Wrong => '\u{25A2}', // ▢ not in word (3.5px unifont)
    }).collect::<String>()
        .chars()
        .flat_map(|c| [c, ' '])
        .collect::<String>()
        .trim_end()
        .to_owned()
}

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["wordle"],
    description: "Chip-staked Wordle. !wordle <chips> [hard] | !wordle <word> | !wordle board | !wordle quit",
    whitelisted: false,
    execute,
};

const MAX_GUESSES: usize = 6;
const MIN_STAKE: i64 = 50;

// Win multipliers indexed by guess number (1-based: index 0 = win on guess 1)
const WIN_MULTIPLIERS: [f64; 6] = [8.0, 5.0, 3.0, 2.0, 1.5, 1.2];

pub struct WordleSession {
    pub game: cl_wordle::game::Game,
    pub stake: i64,
    pub hard_mode: bool,
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();

        match arg.as_str() {
            "" => {
                if has_active_game(ctx.state, ctx.sender) {
                    show_board(&ctx);
                } else {
                    ctx.whisper_success("Usage: !wordle <chips> [hard] | guess: !wordle <word> | !wordle board | !wordle quit");
                }
            }
            "board" => show_board(&ctx),
            "quit" | "forfeit" => quit_game(&ctx).await?,
            "help" => {
                ctx.whisper_success("!wordle <chips> [hard] — start | !wordle <word> — guess | !wordle board — show board | !wordle quit — forfeit");
            }
            // 5-letter alpha word → guess
            w if w.len() == 5 && w.chars().all(|c| c.is_ascii_alphabetic()) => {
                submit_guess(&ctx, w).await?;
            }
            // Numeric → start game
            _ => {
                let chips: i64 = match arg.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        ctx.whisper_success("Unknown command. Try !wordle help.");
                        return Ok(());
                    }
                };
                let hard = ctx.args.get(1).map(|s| s.eq_ignore_ascii_case("hard")).unwrap_or(false);
                start_game(&ctx, chips, hard).await?;
            }
        }
        Ok(())
    })
}

// ── Start ─────────────────────────────────────────────────────────────────────

async fn start_game(ctx: &CommandContext<'_>, stake: i64, hard_mode: bool) -> anyhow::Result<()> {
    if stake < MIN_STAKE {
        ctx.whisper_success(format!("Min stake is {}.", chips_str(MIN_STAKE)));
        return Ok(());
    }

    if has_active_game(ctx.state, ctx.sender) {
        ctx.whisper_success("Already have an active game. !wordle board to see it, !wordle quit to forfeit.");
        return Ok(());
    }

    match ctx.state.api.casino_adjust(ctx.sender, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper_success("Casino unavailable.");
            return Ok(());
        }
    }

    let word_set = cl_wordle::words::NYTIMES;
    let day = rand::thread_rng().gen_range(0..word_set.solutions.len());
    let mut game = cl_wordle::game::Game::from_day(day, word_set);
    if hard_mode {
        game.hard_mode();
    }

    {
        let mut games = ctx.state.wordle_games.lock().expect("wordle lock");
        games.insert(ctx.sender.to_owned(), WordleSession { game, stake, hard_mode });
    }

    let mode_str = if hard_mode { " [hard mode]" } else { "" };
    ctx.whisper_success(format!(
        "Wordle started{mode_str}! {} staked. {} guesses. \u{25A3}=correct \u{25C8}=wrong spot \u{25A2}=not in word",
        chips_str(stake), MAX_GUESSES
    ));
    ctx.whisper_success("Win multipliers: 8x/5x/3x/2x/1.5x/1.2x — guess with !wordle <word>");
    Ok(())
}

// ── Guess ─────────────────────────────────────────────────────────────────────

async fn submit_guess(ctx: &CommandContext<'_>, word: &str) -> anyhow::Result<()> {
    // Phase 1: game logic under lock — collect everything needed before unlocking
    enum GuessOutcome {
        Continue { board: Vec<String>, guesses_used: usize },
        Win { board: Vec<String>, guesses_used: usize, stake: i64 },
        Lose { board: Vec<String>, solution: String, stake: i64 },
        NotInWordList,
        HardModeViolation(usize),
        #[allow(dead_code)]
        NoGame,
    }

    let outcome = {
        let mut games = ctx.state.wordle_games.lock().expect("wordle lock");
        let session = match games.get_mut(ctx.sender) {
            Some(s) => s,
            None => {
                drop(games);
                return {
                    ctx.whisper_success("No active game. Start one with !wordle <chips>.");
                    Ok(())
                };
            }
        };

        use cl_wordle::state::GuessError;
        match session.game.guess(word) {
            Err(GuessError::NotInWordList) => GuessOutcome::NotInWordList,
            Err(GuessError::MissingExactValues(i)) => GuessOutcome::HardModeViolation(i),
            Ok(_) => {
                let guesses_used = session.game.guesses().count();
                let board: Vec<String> = session.game.guesses()
                    .map(|(w, m)| format!("{}: {}", circled(&w.to_uppercase()), render_matches(&m)))
                    .collect();

                use cl_wordle::state::GameOver;
                match session.game.game_over() {
                    Some(GameOver::Win) => {
                        let stake = session.stake;
                        games.remove(ctx.sender);
                        GuessOutcome::Win { board, guesses_used, stake }
                    }
                    Some(GameOver::Lose) => {
                        let solution = session.game.solution().to_owned();
                        let stake = session.stake;
                        games.remove(ctx.sender);
                        GuessOutcome::Lose { board, solution, stake }
                    }
                    None => GuessOutcome::Continue { board, guesses_used },
                }
            }
        }
    };

    // Phase 2: respond + API calls (no lock held)
    match outcome {
        GuessOutcome::NotInWordList => {
            ctx.whisper_error(format!("'{}' isn't in the word list.", word.to_uppercase()));
        }
        GuessOutcome::HardModeViolation(i) => {
            ctx.whisper_success(format!("Hard mode: position {} must stay fixed.", i + 1));
        }
        GuessOutcome::Continue { board, guesses_used } => {
            ctx.whisper_board(&board).await;
            ctx.whisper_success(format!("Guess {}/{}.", guesses_used, MAX_GUESSES));
        }
        GuessOutcome::Win { board, guesses_used, stake } => {
            let mult = WIN_MULTIPLIERS[guesses_used.saturating_sub(1).min(5)];
            let payout = (stake as f64 * mult).ceil() as i64;
            let net = payout - stake;
            let win = ctx.state.api.casino_win(ctx.sender, payout).await.unwrap_or_default();
            let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
            ctx.whisper_board(&board).await;
            ctx.whisper_success(format!(
                "You got it in {}/{}! {}x payout — +{} chips ({}){alimony_note}",
                guesses_used, MAX_GUESSES, mult, chips_str(net), chips_str(payout)
            ));
        }
        GuessOutcome::Lose { board, solution, stake } => {
            let _ = ctx.state.api.casino_jackpot_rake(stake).await;
            ctx.whisper_board(&board).await;
            ctx.whisper_success(format!(
                "The word was {}. -{} chips (to jackpot).",
                solution.to_uppercase(), chips_str(stake)
            ));
        }
        GuessOutcome::NoGame => {
            ctx.whisper_success("No active game. Start one with !wordle <chips>.");
        }
    }
    Ok(())
}

// ── Board ─────────────────────────────────────────────────────────────────────

fn show_board(ctx: &CommandContext) {
    let games = ctx.state.wordle_games.lock().expect("wordle lock");
    let session = match games.get(ctx.sender) {
        Some(s) => s,
        None => {
            ctx.whisper_success("No active game. Start one with !wordle <chips>.");
            return;
        }
    };

    let guesses_used = session.game.guesses().count();
    let mode_str = if session.hard_mode { " [hard]" } else { "" };
    ctx.whisper_success(format!(
        "Wordle{} — {}/{} guesses — {} staked",
        mode_str, guesses_used, MAX_GUESSES, chips_str(session.stake)
    ));
    for (word, matches) in session.game.guesses() {
        ctx.whisper_success(format!("{}: {}", circled(&word.to_uppercase()), render_matches(&matches)));
    }
}

// ── Quit ─────────────────────────────────────────────────────────────────────

async fn quit_game(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let removed = {
        let mut games = ctx.state.wordle_games.lock().expect("wordle lock");
        games.remove(ctx.sender)
    };
    match removed {
        None => ctx.whisper_success("No active game."),
        Some(session) => {
            let solution = session.game.solution().to_owned();
            let _ = ctx.state.api.casino_jackpot_rake(session.stake).await;
            ctx.whisper_success(format!(
                "Forfeited. The word was {}. -{} chips (to jackpot).",
                solution.to_uppercase(), chips_str(session.stake)
            ));
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn has_active_game(state: &crate::structure::mineflayer::bot::AzaleaState, player: &str) -> bool {
    state.wordle_games.lock().expect("wordle lock").contains_key(player)
}
