use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::logger;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["calc", "wa", "wolframalpha"],
    description: "Query Wolfram|Alpha. Usage: {prefix}calc <query>",
    whitelisted: false,
    execute: execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}calc <query>", ctx.runtime.prefix));
            return Ok(());
        }
        let app_id = ctx.runtime.wolfram_app_id.clone();
        if app_id.is_empty() {
            ctx.whisper("Wolfram|Alpha is not configured.".to_owned());
            return Ok(());
        }
        let query = ctx.args.join(" ");
        match wolfram_query(&app_id, &query).await {
            Some(result) => ctx.chat(result),
            None => ctx.chat(format!("No result for: {query}")),
        }
        Ok(())
    })
}

async fn wolfram_query(app_id: &str, query: &str) -> Option<String> {
    let url = format!(
        "https://www.wolframalpha.com/api/v1/llm-api?input={}&appid={}",
        percent_encode(query),
        app_id,
    );

    let text = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    if text.trim().is_empty() {
        return None;
    }

    logger::debug_cat("calc", format!("[calc] WA raw response:\n{text}"));

    // WA LLM API: "Label:\nvalue\nvalue2\n\nNextLabel:\n..." structure
    // Collect each section's values (all lines until the next label), join with " | "
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut sections: Vec<(&str, String)> = Vec::new();
    let mut cur_label: Option<&str> = None;
    let mut cur_values: Vec<&str> = Vec::new();

    for line in &lines {
        if line.ends_with(':') {
            if let Some(label) = cur_label.take() {
                if !cur_values.is_empty() {
                    sections.push((label, cur_values.join(" | ")));
                    cur_values.clear();
                }
            }
            cur_label = Some(line.trim_end_matches(':'));
        } else if cur_label.is_some() {
            cur_values.push(line);
        }
    }
    if let Some(label) = cur_label {
        if !cur_values.is_empty() {
            sections.push((label, cur_values.join(" | ")));
        }
    }

    // Preferred result sections in priority order
    const PRIORITY: &[&str] = &[
        "Result",
        "Results",
        "Exact result",
        "Solution",
        "Solutions",
        "Real solution",
        "Real solutions",
        "Complex solution",
        "Complex solutions",
        "Derivative",
        "Definite integral",
        "Indefinite integral",
        "Infinite sum",
        "Sum",
        "Limit",
        "Decimal approximation",
        "Approximate form",
        "Approximate decimal result",
        "Answer",
        "Value",
        "Output",
        "Property",
    ];

    // Decorative/boilerplate sections to skip in fallback
    const SKIP_PREFIXES: &[&str] = &[
        "Query",
        "Input",
        "Assumption",
        "Number name",
        "Number line",
        "Alternate form",
        "Alternative representation",
        "Additional conversion",
        "Comparison",
        "Wolfram Language",
        "Wolfram|Alpha",
        "Plot",
        "Graph",
        "Visual",
        "Image",
    ];

    let answer = PRIORITY
        .iter()
        .find_map(|p| {
            sections
                .iter()
                .find(|(label, _)| label.eq_ignore_ascii_case(p))
                .map(|(_, v)| v.as_str())
        })
        .or_else(|| {
            sections
                .iter()
                .find(|(label, _)| {
                    SKIP_PREFIXES
                        .iter()
                        .all(|skip| !label.starts_with(skip))
                })
                .map(|(_, v)| v.as_str())
        })?;

    // Use ": " separator for equations (query already contains "=") to avoid "a = b = c = d"
    let sep = if query.contains('=') { ": " } else { " = " };
    let display = format!("{query}{sep}{answer}");
    if display.chars().count() > 220 {
        Some(format!("{}...", display.chars().take(217).collect::<String>()))
    } else {
        Some(display)
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
