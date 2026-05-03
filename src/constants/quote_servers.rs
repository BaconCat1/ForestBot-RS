pub const QUOTE_SERVERS: &[&str] = &[
    "aksh",
    "anarchynetwork",
    "andromeda",
    "barevanilla",
    "deepnilla",
    "eupvp",
    "eusurvival",
    "exiledanarchy",
    "forestbot",
    "freedomvanilla",
    "mcvpg",
    "north-american-vanilla",
    "novaanarchy",
    "p-anarchy",
    "playanarchy",
    "purityvanilla",
    "refinedvanilla",
    "simpcraft",
    "simplyanarchy",
    "simplyvanilla",
    "straightupminecraft",
    "truevanilla",
    "uneasyevent",
    "uneasyvanilla",
    "vanillaanarchy",
    "vanillasteal",
];

pub fn is_quote_server(server: &str) -> bool {
    QUOTE_SERVERS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(server))
}
