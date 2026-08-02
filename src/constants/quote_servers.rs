// Cut 2026-08-02 per a real message-count audit (see json/CENSOR_DECISIONS.md's neighbor,
// the ranked-todo done log, for the full numbers): kept only servers clearing 100k real
// (non-bot) messages, plus `simplevanilla` as an explicit exception since it's currently
// live despite low historical volume. Everything cut was confirmed real-but-quiet ("bot
// joined for a few minutes, never got proper support"), not test/dev noise -- the actual
// test servers (`test`/`newtest`/`newtest_new`/`forestbot`) were merged into a single
// `testing_archive` server tag in the DB instead of being listed here at all.
pub const QUOTE_SERVERS: &[&str] = &[
    "aksh",
    "barevanilla",
    "eupvp",
    "eusurvival",
    "mcvpg",
    "refinedvanilla",
    "simplevanilla",
    "simplyanarchy",
    "simplyvanilla",
    "truevanilla",
    "uneasyvanilla",
    "vanillaanarchy",
];

pub fn is_quote_server(server: &str) -> bool {
    QUOTE_SERVERS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(server))
}
