use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["askgod", "agod"],
    description: "Consult the divine oracle. Usage: {prefix}askgod <god>",
    whitelisted: false,
    execute,
};

pub const LISTGODS_COMMAND: CommandDefinition = CommandDefinition {
    names: &["listgods", "gods"],
    description: "Lists available gods for {prefix}askgod. Usage: {prefix}listgods",
    whitelisted: false,
    execute: listgods,
};

pub const SEARCHGOD_COMMAND: CommandDefinition = CommandDefinition {
    names: &["searchgod", "godsearch", "sgod"],
    description: "Search sacred texts for a keyword or phrase. Usage: {prefix}searchgod <words>",
    whitelisted: false,
    execute: searchgod,
};

pub const GODVERSE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["godverse", "verse", "vgod"],
    description: "Look up a verse by reference. Usage: {prefix}godverse <reference>",
    whitelisted: false,
    execute: godverse,
};

pub const GODSTATS_COMMAND: CommandDefinition = CommandDefinition {
    names: &["godstats"],
    description: "Shows stats for the {prefix}askgod corpora. Usage: {prefix}godstats",
    whitelisted: false,
    execute: godstats,
};

fn godstats(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match GOD_STATS.get() {
            Some(s) => {
                let mb = s.total_bytes as f64 / 1_048_576.0;
                let compressed_mb = s.total_compressed_bytes as f64 / 1_048_576.0;
                ctx.chat(format!(
                    "God Stats: {} Corpora, {:.1} MB ({:.1} MB on disk), {} verses, loaded in {:.2}s, Known Gods: {}",
                    s.corpora_loaded, mb, compressed_mb, s.total_verses, s.elapsed.as_secs_f64(), KNOWN_GODS_COUNT
                ));
            }
            None => {
                ctx.whisper("Stats not ready yet, corpora still loading.");
            }
        }
        Ok(())
    })
}

fn searchgod(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper("Usage: !searchgod <words>");
            return Ok(());
        }
        let kw = ctx.args.join(" ").to_lowercase();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs();
        let nanos = now.subsec_nanos();
        let hits = search_corpora(&kw);
        if hits.is_empty() {
            ctx.chat("The light of the Oracle fades, your word is not that of the Gods.".to_string());
            return Ok(());
        }
        let h = secs.wrapping_mul(2654435761).wrapping_add(nanos as u64);
        let verse = hits[(h as usize) % hits.len()];
        ctx.chat(make_output_with_keyword(&verse.reference, &verse.text, &kw));
        Ok(())
    })
}

fn listgods(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        const GODS: &[&str] = &[
            "god", "allah", "mormon", "bahaullah", "jah", "hayyi", "mani",
            "moon", "noi", "sophia", "eddy", "krishna", "buddha", "waheguru", "tao",
            "confucius", "amaterasu", "caodai", "zoroaster", "osiris", "odin", "zeus",
            "hurakan", "hammurabi", "huitzilopochtli", "hermetic", "crowley", "eris",
            "kardec", "tenrikyo", "falun", "rael", "vivec", "dobbs", "bokonon", "tolkien", "shaker", "swedenborg", "canaan", "moorish", "setian", "urantia", "heavensgate", "process", "andraste", "orpheus", "plotinus", "zohar", "sumerian", "lavey", "cathar", "caine", "olamina", "mahavira", "pariacaca", "iching", "kebra", "rasta", "jedi", "qumran", "enoch", "acim", "faithism", "aquarian", "lawofone", "iammovement", "acadfuturesci", "unarius", "aetherius", "anthroposophy", "mahikari", "radhasoami", "hawaii", "alicebailey", "tibetan", "baba",
        ];
        const MAX: usize = 220;
        let mut line = format!("!askgod <god> -- {} corpora, one per corpus: ", GODS.len());
        let mut started = false;
        for &god in GODS {
            if !started {
                line.push_str(god);
                started = true;
            } else {
                let candidate = format!("{line}, {god}");
                if candidate.len() > MAX {
                    ctx.whisper(&line);
                    line = god.to_owned();
                } else {
                    line = candidate;
                }
            }
        }
        if started {
            ctx.whisper(&line);
        }
        Ok(())
    })
}

struct Verse {
    reference: String,
    text: String,
}

// One OnceLock per corpus — populated lazily on first use.
static KJV_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static KORAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MORMON_MERGED_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static BAHAI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static RASTA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MANI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static UNIFICATION_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static NOI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static GNOSTIC_MERGED_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static CS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static HINDU_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static BUDDHISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SIKHISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static TAOISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static CONFUCIANISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SHINTO_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static CAO_DAI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ZOROASTRIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static EGYPTIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static NORSE_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static OLYMPIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MAYAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static BABYLONIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static AZTEC_MERGED_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static HERMETIC_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static THELEMA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static DISCORDIA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SPIRITISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static TENRIKYO_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static FALUNDAFA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static RAELISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ELDERSCROLLS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SUBGENIUS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static BOKONON_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static TOLKIEN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SHAKER_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SWEDENBORG_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static CANAAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MOORISH_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static TEMPLEOFSET_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static URANTIA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static HEAVENSGATE_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static PROCESSCHURCH_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ANDRASTIANISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ORPHIC_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static NEOPLATONISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static KABBALAH_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static SUMERIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static LAVEYANISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static CATHARISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static NODDISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static EARTHSEED_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static JAINISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static INCAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ICHING_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static JEDI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static DSS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static DEUTEROCANON_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ACIM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MANDAEAN_MERGED_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static FAITHISM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static AQUARIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static LAWOFONE_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static IAMMOVEMENT_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ACADFUTURESCI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static UNARIUS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static AETHERIUS_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static ANTHROPOSOPHY_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MAHIKARI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static RADHASOAMI_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static HAWAIIAN_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static COMMOFCHR_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static STRANGITE_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static AGELESSWISDOM_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();
static MEHERBABA_CORPUS: OnceLock<Vec<Verse>> = OnceLock::new();

type CorpusEntry = (&'static OnceLock<Vec<Verse>>, &'static str, fn(&str) -> anyhow::Result<Vec<Verse>>);

fn all_corpora() -> [CorpusEntry; 75] {
    [
        (&KJV_CORPUS, "godtexts/kjv.txt.zst", parse_kjv),
        (&KORAN_CORPUS, "godtexts/koran.txt.zst", parse_koran),
        (&MORMON_MERGED_CORPUS, "godtexts/mormon.txt.zst", parse_merged_mormon),
        (&BAHAI_CORPUS, "godtexts/bahai.txt.zst", parse_bahai),
        (&RASTA_CORPUS, "godtexts/rastafarianism.txt.zst", parse_bahai),
        (&MANDAEAN_MERGED_CORPUS, "godtexts/mandaeanism.txt.zst", parse_merged_mandaean),
        (&MANI_CORPUS, "godtexts/manichaeanism.txt.zst", parse_bahai),
        (&UNIFICATION_CORPUS, "godtexts/unificationchurch.txt.zst", parse_bahai),
        (&NOI_CORPUS, "godtexts/noi.txt.zst", parse_bahai),
        (&GNOSTIC_MERGED_CORPUS, "godtexts/gnosticism.txt.zst", parse_merged_gnostic),
        (&CS_CORPUS, "godtexts/christianscience.txt.zst", parse_bahai),
        (&HINDU_CORPUS, "godtexts/hinduism.txt.zst", parse_bahai),
        (&BUDDHISM_CORPUS, "godtexts/buddhism.txt.zst", parse_bahai),
        (&SIKHISM_CORPUS, "godtexts/sikhism.txt.zst", parse_bahai),
        (&TAOISM_CORPUS, "godtexts/taoism.txt.zst", parse_bahai),
        (&CONFUCIANISM_CORPUS, "godtexts/confucianism.txt.zst", parse_bahai),
        (&SHINTO_CORPUS, "godtexts/shinto.txt.zst", parse_bahai),
        (&CAO_DAI_CORPUS, "godtexts/cao_dai.txt.zst", parse_bahai),
        (&ZOROASTRIAN_CORPUS, "godtexts/zoroastrianism.txt.zst", parse_bahai),
        (&EGYPTIAN_CORPUS, "godtexts/egyptian.txt.zst", parse_bahai),
        (&NORSE_CORPUS, "godtexts/norse.txt.zst", parse_bahai),
        (&OLYMPIAN_CORPUS, "godtexts/olympian.txt.zst", parse_bahai),
        (&MAYAN_CORPUS, "godtexts/mayan.txt.zst", parse_bahai),
        (&BABYLONIAN_CORPUS, "godtexts/babylonian.txt.zst", parse_bahai),
        (&AZTEC_MERGED_CORPUS, "godtexts/aztec.txt.zst", parse_merged_aztec),
        (&HERMETIC_CORPUS, "godtexts/hermeticism.txt.zst", parse_bahai),
        (&THELEMA_CORPUS, "godtexts/thelema.txt.zst", parse_bahai),
        (&DISCORDIA_CORPUS, "godtexts/discordianism.txt.zst", parse_bahai),
        (&SPIRITISM_CORPUS, "godtexts/spiritism.txt.zst", parse_bahai),
        (&TENRIKYO_CORPUS, "godtexts/ofudesaki.txt.zst", parse_bahai),
        (&FALUNDAFA_CORPUS, "godtexts/falungong.txt.zst", parse_bahai),
        (&RAELISM_CORPUS, "godtexts/raelism.txt.zst", parse_bahai),
        (&ELDERSCROLLS_CORPUS, "godtexts/elderscrolls.txt.zst", parse_bahai),
        (&SUBGENIUS_CORPUS, "godtexts/subgenius.txt.zst", parse_bahai),
        (&BOKONON_CORPUS, "godtexts/bokononism.txt.zst", parse_bahai),
        (&TOLKIEN_CORPUS, "godtexts/tolkien.txt.zst", parse_bahai),
        (&SHAKER_CORPUS, "godtexts/shaker.txt.zst", parse_bahai),
        (&SWEDENBORG_CORPUS, "godtexts/swedenborg.txt.zst", parse_bahai),
        (&CANAAN_CORPUS, "godtexts/canaan.txt.zst", parse_bahai),
        (&MOORISH_CORPUS, "godtexts/moorishscience.txt.zst", parse_bahai),
        (&TEMPLEOFSET_CORPUS, "godtexts/templeofset.txt.zst", parse_bahai),
        (&URANTIA_CORPUS, "godtexts/urantia.txt.zst", parse_bahai),
        (&HEAVENSGATE_CORPUS, "godtexts/heavensgate.txt.zst", parse_bahai),
        (&PROCESSCHURCH_CORPUS, "godtexts/processchurch.txt.zst", parse_bahai),
        (&ANDRASTIANISM_CORPUS, "godtexts/andrastianism.txt.zst", parse_bahai),
        (&ORPHIC_CORPUS, "godtexts/orphic.txt.zst", parse_bahai),
        (&NEOPLATONISM_CORPUS, "godtexts/neoplatonism.txt.zst", parse_bahai),
        (&KABBALAH_CORPUS, "godtexts/kabbalah.txt.zst", parse_bahai),
        (&SUMERIAN_CORPUS, "godtexts/sumerian.txt.zst", parse_bahai),
        (&LAVEYANISM_CORPUS, "godtexts/laveyanism.txt.zst", parse_bahai),
        (&CATHARISM_CORPUS, "godtexts/catharism.txt.zst", parse_bahai),
        (&NODDISM_CORPUS, "godtexts/noddism.txt.zst", parse_bahai),
        (&EARTHSEED_CORPUS, "godtexts/earthseed.txt.zst", parse_bahai),
        (&JAINISM_CORPUS, "godtexts/jainism.txt.zst", parse_bahai),
        (&INCAN_CORPUS, "godtexts/incan.txt.zst", parse_bahai),
        (&ICHING_CORPUS, "godtexts/iching.txt.zst", parse_bahai),
        (&JEDI_CORPUS, "godtexts/jedi.txt.zst", parse_bahai),
        (&DSS_CORPUS, "godtexts/deadseascrolls.txt.zst", parse_bahai),
        (&DEUTEROCANON_CORPUS, "godtexts/deuterocanon.txt.zst", parse_bahai),
        (&ACIM_CORPUS, "godtexts/acim.txt.zst", parse_bahai),
        (&FAITHISM_CORPUS, "godtexts/faithism.txt.zst", parse_bahai),
        (&AQUARIAN_CORPUS, "godtexts/aquarian.txt.zst", parse_bahai),
        (&LAWOFONE_CORPUS, "godtexts/lawofone.txt.zst", parse_bahai),
        (&IAMMOVEMENT_CORPUS, "godtexts/iammovement.txt.zst", parse_bahai),
        (&ACADFUTURESCI_CORPUS, "godtexts/acadfuturesci.txt.zst", parse_bahai),
        (&UNARIUS_CORPUS, "godtexts/unarius.txt.zst", parse_bahai),
        (&AETHERIUS_CORPUS, "godtexts/aetherius.txt.zst", parse_bahai),
        (&ANTHROPOSOPHY_CORPUS, "godtexts/anthroposophy.txt.zst", parse_bahai),
        (&MAHIKARI_CORPUS, "godtexts/mahikari.txt.zst", parse_bahai),
        (&RADHASOAMI_CORPUS, "godtexts/radhasoami.txt.zst", parse_bahai),
        (&HAWAIIAN_CORPUS, "godtexts/hawaiian.txt.zst", parse_bahai),
        (&COMMOFCHR_CORPUS, "godtexts/commofchr.txt.zst", parse_bahai),
        (&STRANGITE_CORPUS, "godtexts/strangite.txt.zst", parse_bahai),
        (&AGELESSWISDOM_CORPUS, "godtexts/agelesswisdom.txt.zst", parse_bahai),
        (&MEHERBABA_CORPUS, "godtexts/meherbaba.txt.zst", parse_bahai),
    ]
}

const OUT_TO_LUNCH: &[&str] = &[
    "currently smiting someone else",
    "on sabbatical",
    "unavailable due to a crisis of faith (theirs)",
    "between miracles",
    "off answering prayers in a higher priority queue",
    "temporarily mortal",
    "experiencing an existential crisis",
    "resting on the seventh day (again)",
    "stuck in traffic on the astral plane",
    "in a meeting with the other gods",
    "currently deprecated",
    "not accepting new believers at this time",
    "currently on the Great Journey",
    "ascending to the Undying Lands",
    "busy with the Music of the Ainur",
    "trapped in the underworld with Inanna",
    "attending a Bokononist ritual",
    "on a karass mission",
    "busy fighting the Battle of Red Mountain",
    "currently being worshipped by the Tribunal",
    "taking a granfalloon meeting",
    "in communion with the Peacock Angel",
    "slack deficient",
    "experiencing a foma-related delay",
    "currently ascending with the Thetans",
    "in the fifth era",
    "on the Great Journey (not that one, the other one)",
    "helping Satan bury dinosaur bones in the earth",
    "selling mantracks in Texas",
    "tallying the census in Numbers",
    "drawing something better than realism, an elephant with blue eyes",
    "lost in thought about foreskins",
    "burning Mayan codices with conquistadors",
    "writing apologetics",
    "joy riding in the pope-mobile",
    "riding a chariot of the gods",
    "counting the rings on Methuselah's birthday cake",
    "waiting for the next comet",
    "auditing someone",
    "adding another planet to the cosmology",
    "arguing about whether beer is allowed",
    "calculating the exact date of the apocalypse again",
    "updating the dietary restrictions",
    "debating how many angels fit on a pinhead",
    "on the 8th day creating something God forgot",
    "filing a noise complaint against the muezzin",
    "deciding which books didn't make the canon cut",
    "commissioning another ceiling painting",
    "negotiating with the Council of Nicaea",
    "reincarnating, back in a moment",
    "currently between prophets",
    "selling crack to Netherwhal",
];

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        // secs for corpus selection: alternates every second, avoids Windows clock
        // resolution issue (subsec_nanos is always a multiple of ~15.6M on Windows,
        // which is even → nanos % N is biased toward index 0).
        let secs = now.as_secs();
        let nanos = now.subsec_nanos();

        let god_arg = ctx.args.first().map(|s| s.to_lowercase());
        let _keyword_was_given = god_arg.is_some();

        // Multi-word arg is treated as a question to the oracle, not a god name
        if ctx.args.len() >= 2 {
            let (corpus_cell, path, parser) = pick_random_available_corpus(secs);
            let corpus = match get_corpus(corpus_cell, path, parser).await {
                Ok(c) => c,
                Err(e) => {
                    ctx.whisper(format!("Oracle unavailable: {e}"));
                    return Ok(());
                }
            };
            if corpus.is_empty() {
                ctx.whisper("The oracle is silent.");
                return Ok(());
            }
            let idx = (nanos >> 4) as usize % corpus.len();
            let verse = &corpus[idx];
            let full = format!("[{}] {}", verse.reference, verse.text);
            let out = if full.chars().count() > 200 {
                format!("{}...", full.chars().take(197).collect::<String>())
            } else {
                full
            };
            ctx.chat(format!("The Gods have heard you, and they send you their divine wisdom: {out}"));
            return Ok(());
        }

        let (corpus_cell, path, parser): CorpusEntry =
            match god_arg.as_deref() {
                Some("allah") | Some("quran") | Some("koran") | Some("islam") | Some("muslim") | Some("muhammad") => {
                    (&KORAN_CORPUS, "godtexts/koran.txt.zst", parse_koran)
                }
                Some("moroni") | Some("nephi") | Some("mormon") | Some("joseph") | Some("lds") | Some("bom") | Some("dnc") | Some("doctrine") | Some("covenants") | Some("pogp") | Some("kimball") | Some("brigham") | Some("latterday") | Some("od1") | Some("od2") => {
                    (&MORMON_MERGED_CORPUS, "godtexts/mormon.txt.zst", parse_merged_mormon)
                }
                Some("bahai") | Some("baha") | Some("bahaullah") | Some("aqdas") => {
                    (&BAHAI_CORPUS, "godtexts/bahai.txt.zst", parse_bahai)
                }
                Some("piby") | Some("rastafari") | Some("rasta") | Some("athlyi") | Some("rogers") | Some("jah") | Some("rastafarianism") | Some("kebra") | Some("nagast") | Some("kebraNagast") | Some("selassie") | Some("haile") | Some("zion") | Some("babylon") | Some("makeda") | Some("solomonic") => {
                    (&RASTA_CORPUS, "godtexts/rastafarianism.txt.zst", parse_bahai)
                }
                Some("mandaean") | Some("mandaeanism") | Some("ginza") | Some("manda") | Some("nasoraean") | Some("nasorean") | Some("hayyi") | Some("hiia") | Some("mbofjohn") | Some("mandaeanbookofjohn") | Some("bookofkings") => {
                    (&MANDAEAN_MERGED_CORPUS, "godtexts/mandaeanism.txt.zst", parse_merged_mandaean)
                }
                Some("mani") | Some("manichean") | Some("manichaean") | Some("manichaeism") | Some("manicheanism") => {
                    (&MANI_CORPUS, "godtexts/manichaeanism.txt.zst", parse_bahai)
                }
                Some("moon") | Some("unification") | Some("moonies") | Some("divine") | Some("principle") | Some("divineprincipal") => {
                    (&UNIFICATION_CORPUS, "godtexts/unificationchurch.txt.zst", parse_bahai)
                }
                Some("noi") | Some("nation") | Some("blackman") | Some("yakub") | Some("yakoub") => {
                    (&NOI_CORPUS, "godtexts/noi.txt.zst", parse_bahai)
                }
                Some("gnostic") | Some("gnosticism") | Some("nag") | Some("hammadi") | Some("sophia") | Some("pleroma") | Some("demiurge") | Some("pistissophia") | Some("brucejeu") | Some("jeu") | Some("askcodex") => {
                    (&GNOSTIC_MERGED_CORPUS, "godtexts/gnosticism.txt.zst", parse_merged_gnostic)
                }
                Some("eddy") | Some("christianscience") | Some("marybakeddy") | Some("scienceandhealth") => {
                    (&CS_CORPUS, "godtexts/christianscience.txt.zst", parse_bahai)
                }
                Some("brahma") | Some("vishnu") | Some("shiva") | Some("krishna") | Some("indra") | Some("hindu") | Some("hinduism") | Some("veda") | Some("vedic") | Some("mahabharata") | Some("gita") => {
                    (&HINDU_CORPUS, "godtexts/hinduism.txt.zst", parse_bahai)
                }
                Some("buddha") | Some("buddhism") | Some("pali") | Some("dharma") | Some("nirvana") | Some("tipitaka") | Some("gautama") | Some("theravada") | Some("sangha") => {
                    (&BUDDHISM_CORPUS, "godtexts/buddhism.txt.zst", parse_bahai)
                }
                Some("waheguru") | Some("nanak") | Some("sikh") | Some("sikhism") | Some("granth") | Some("ggs") | Some("guru") => {
                    (&SIKHISM_CORPUS, "godtexts/sikhism.txt.zst", parse_bahai)
                }
                Some("tao") | Some("taoist") | Some("taoism") | Some("laozi") | Some("laotzu") | Some("zhuangzi") | Some("chuangtzu") | Some("ttc") => {
                    (&TAOISM_CORPUS, "godtexts/taoism.txt.zst", parse_bahai)
                }
                Some("confucius") | Some("confucianism") | Some("analects") | Some("kongzi") | Some("lunyu") | Some("zhongyong") => {
                    (&CONFUCIANISM_CORPUS, "godtexts/confucianism.txt.zst", parse_bahai)
                }
                Some("shinto") | Some("kami") | Some("amaterasu") | Some("izanagi") | Some("izanami") | Some("kojiki") | Some("norito") | Some("nihongi") => {
                    (&SHINTO_CORPUS, "godtexts/shinto.txt.zst", parse_bahai)
                }
                Some("caodai") | Some("cao_dai") | Some("jade") | Some("jadeemperor") | Some("caoism") | Some("thanshinh") => {
                    (&CAO_DAI_CORPUS, "godtexts/cao_dai.txt.zst", parse_bahai)
                }
                Some("zoroaster") | Some("zoroastrian") | Some("zoroastrianism") | Some("ahura") | Some("mazda") | Some("zarathustra") | Some("avesta") | Some("parsi") | Some("zend") => {
                    (&ZOROASTRIAN_CORPUS, "godtexts/zoroastrianism.txt.zst", parse_bahai)
                }
                Some("egypt") | Some("egyptian") | Some("ra") | Some("osiris") | Some("isis") | Some("horus") | Some("anubis") | Some("thoth") | Some("amon") | Some("amun") | Some("aten") => {
                    (&EGYPTIAN_CORPUS, "godtexts/egyptian.txt.zst", parse_bahai)
                }
                Some("norse") | Some("odin") | Some("thor") | Some("loki") | Some("freyr") | Some("freyja") | Some("edda") | Some("valhalla") | Some("asgard") | Some("yggdrasil") | Some("ragnarok") => {
                    (&NORSE_CORPUS, "godtexts/norse.txt.zst", parse_bahai)
                }
                Some("greek") | Some("olympian") | Some("zeus") | Some("athena") | Some("apollo") | Some("poseidon") | Some("hera") | Some("ares") | Some("hermes") | Some("artemis") | Some("iliad") | Some("odyssey") | Some("homer") => {
                    (&OLYMPIAN_CORPUS, "godtexts/olympian.txt.zst", parse_bahai)
                }
                Some("mayan") | Some("maya") | Some("kiché") | Some("kiche") | Some("hurakan") | Some("popolvuh") | Some("xibalba") | Some("quetzalcoatl") | Some("itzamna") | Some("kukulkan") => {
                    (&MAYAN_CORPUS, "godtexts/mayan.txt.zst", parse_bahai)
                }
                Some("babylonian") | Some("hammurabi") | Some("marduk") | Some("shamash") | Some("ishtar") | Some("akkad") | Some("akkadian") | Some("mesopotamia") => {
                    (&BABYLONIAN_CORPUS, "godtexts/babylonian.txt.zst", parse_bahai)
                }
                Some("sumerian") | Some("sumer") | Some("gilgamesh") | Some("enkidu") | Some("enumaelish") | Some("tiamat") | Some("apsu") | Some("anunnaki") | Some("enlil") | Some("enki") | Some("inanna") | Some("nanna") | Some("utu") => {
                    (&SUMERIAN_CORPUS, "godtexts/sumerian.txt.zst", parse_bahai)
                }
                Some("lavey") | Some("laveyanism") | Some("satanic") | Some("churchofsatan") | Some("blackpope") | Some("satanbible") | Some("nineantsatanicstatements") => {
                    (&LAVEYANISM_CORPUS, "godtexts/laveyanism.txt.zst", parse_bahai)
                }
                Some("cathar") | Some("catharism") | Some("cathari") | Some("albigensian") | Some("albigenses") | Some("parfait") | Some("consolamentum") | Some("bogomil") | Some("bogomilism") | Some("secretsupper") | Some("interrogatio") => {
                    (&CATHARISM_CORPUS, "godtexts/catharism.txt.zst", parse_bahai)
                }
                Some("caine") | Some("noddism") | Some("kindred") | Some("vampire") | Some("masquerade") | Some("gehenna") | Some("jyhad") | Some("sabbat") | Some("camarilla") | Some("antediluvian") | Some("bookofnod") | Some("vtm") | Some("worldofdarkness") => {
                    (&NODDISM_CORPUS, "godtexts/noddism.txt.zst", parse_bahai)
                }
                Some("earthseed") | Some("olamina") | Some("godischange") | Some("godseed") => {
                    (&EARTHSEED_CORPUS, "godtexts/earthseed.txt.zst", parse_bahai)
                }
                Some("jain") | Some("jainism") | Some("mahavira") | Some("gaina") | Some("tirthankara") | Some("akaranga") | Some("vardhamana") => {
                    (&JAINISM_CORPUS, "godtexts/jainism.txt.zst", parse_bahai)
                }
                Some("incan") | Some("inca") | Some("huarochiri") | Some("pariacaca") | Some("paria") | Some("quechua") | Some("andean") | Some("huallallo") | Some("viracocha") => {
                    (&INCAN_CORPUS, "godtexts/incan.txt.zst", parse_bahai)
                }
                Some("iching") | Some("yijing") | Some("yiching") | Some("yi") | Some("zhouyi") | Some("hexagram") | Some("legge") | Some("khien") | Some("bagua") | Some("trigram") => {
                    (&ICHING_CORPUS, "godtexts/iching.txt.zst", parse_bahai)
                }
                Some("jedi") | Some("jedipath") | Some("theforce") | Some("force") | Some("yoda") | Some("skywalker") | Some("anakin") | Some("luke") | Some("obi") | Some("kenobi") | Some("mace") | Some("windu") | Some("sith") | Some("midichlorian") => {
                    (&JEDI_CORPUS, "godtexts/jedi.txt.zst", parse_bahai)
                }
                Some("aztec") | Some("azteca") | Some("mexica") | Some("nahua") | Some("nahuatl") | Some("huitzilopochtli") | Some("tlaloc") | Some("tezcatlipoca") | Some("xipe") | Some("coatlicue") | Some("tonatiuh") | Some("chalchiuhtlicue") => {
                    (&AZTEC_MERGED_CORPUS, "godtexts/aztec.txt.zst", parse_merged_aztec)
                }
                Some("hermetic") | Some("hermeticism") | Some("trismegistus") | Some("poemandres") | Some("corpus") | Some("emerald") | Some("kybalion") => {
                    (&HERMETIC_CORPUS, "godtexts/hermeticism.txt.zst", parse_bahai)
                }
                Some("thelema") | Some("crowley") | Some("aleister") | Some("liber") | Some("beast") | Some("nuit") | Some("hadit") | Some("hoor") | Some("aiwass") | Some("therion") => {
                    (&THELEMA_CORPUS, "godtexts/thelema.txt.zst", parse_bahai)
                }
                Some("eris") | Some("discordia") | Some("discordian") | Some("discordianism") | Some("principia") | Some("fnord") | Some("kallisti") | Some("malaclypse") | Some("chaos") => {
                    (&DISCORDIA_CORPUS, "godtexts/discordianism.txt.zst", parse_bahai)
                }
                Some("spiritism") | Some("spiritist") | Some("kardec") | Some("allankardec") | Some("medium") | Some("spirits") | Some("spiritsbook") => {
                    (&SPIRITISM_CORPUS, "godtexts/spiritism.txt.zst", parse_bahai)
                }
                Some("tenrikyo") | Some("ofudesaki") | Some("oyasama") | Some("tsukihi") | Some("miki") | Some("nakayama") | Some("jiba") => {
                    (&TENRIKYO_CORPUS, "godtexts/ofudesaki.txt.zst", parse_bahai)
                }
                Some("falun") | Some("falundafa") | Some("falungong") | Some("zhuanfalun") | Some("dafa") | Some("lihongzhi") | Some("shifu") => {
                    (&FALUNDAFA_CORPUS, "godtexts/falungong.txt.zst", parse_bahai)
                }
                Some("rael") | Some("raelian") | Some("raelism") | Some("elohim") | Some("vorilhon") | Some("clonaid") => {
                    (&RAELISM_CORPUS, "godtexts/raelism.txt.zst", parse_bahai)
                }
                Some("elderscrolls") | Some("vivec") | Some("tamriel") | Some("daedra") | Some("aedra") | Some("aurbis") | Some("nirn") | Some("nerevarine") | Some("monomyth") | Some("veloth") | Some("dunmer") | Some("morrowind") | Some("khajiit") | Some("alduin") | Some("talos") | Some("shor") | Some("lorkhan") => {
                    (&ELDERSCROLLS_CORPUS, "godtexts/elderscrolls.txt.zst", parse_bahai)
                }
                Some("subgenius") | Some("dobbs") | Some("slack") | Some("xist") | Some("bulldada") | Some("jhvh") | Some("stang") => {
                    (&SUBGENIUS_CORPUS, "godtexts/subgenius.txt.zst", parse_bahai)
                }
                Some("bokonon") | Some("bokononism") | Some("foma") | Some("karass") | Some("granfalloon") | Some("wampeter") | Some("duprass") | Some("vonnegut") | Some("catscradle") | Some("calypso") => {
                    (&BOKONON_CORPUS, "godtexts/bokononism.txt.zst", parse_bahai)
                }
                Some("tolkien") | Some("silmarillion") | Some("ainulindale") | Some("valaquenta") | Some("akallabeth") | Some("numenor") | Some("valar") | Some("maiar") | Some("eru") | Some("iluvatar") | Some("morgoth") | Some("melkor") | Some("arda") | Some("valinor") | Some("feanor") | Some("eldar") | Some("ainur") | Some("middleearth") => {
                    (&TOLKIEN_CORPUS, "godtexts/tolkien.txt.zst", parse_bahai)
                }
                Some("shaker") | Some("shakers") | Some("annlee") | Some("secondappearing") | Some("millennial") | Some("youngs") => {
                    (&SHAKER_CORPUS, "godtexts/shaker.txt.zst", parse_bahai)
                }
                Some("swedenborg") | Some("newchurch") | Some("newjerusalem") | Some("arcana") | Some("coelestia") | Some("conjugial") | Some("influx") | Some("correspondences") | Some("spiritualworld") => {
                    (&SWEDENBORG_CORPUS, "godtexts/swedenborg.txt.zst", parse_bahai)
                }
                Some("canaan") | Some("canaanite") | Some("ugarit") | Some("ugaritic") | Some("baal") | Some("anat") | Some("asherah") | Some("astarte") | Some("aqhat") | Some("kirta") | Some("rephaim") | Some("mot") | Some("yamm") | Some("kothar") => {
                    (&CANAAN_CORPUS, "godtexts/canaan.txt.zst", parse_bahai)
                }
                Some("moorish") | Some("moorishscience") | Some("drewali") | Some("circle7") | Some("noblepath") | Some("moor") | Some("asiatic") => {
                    (&MOORISH_CORPUS, "godtexts/moorishscience.txt.zst", parse_bahai)
                }
                Some("setian") | Some("templeofset") | Some("xeper") | Some("harwer") | Some("aquino") | Some("bookofthenight") | Some("setianblackflame") => {
                    (&TEMPLEOFSET_CORPUS, "godtexts/templeofset.txt.zst", parse_bahai)
                }
                Some("urantia") | Some("urantiabook") | Some("urantian") | Some("orvonton") | Some("nebadon") | Some("havona") | Some("forsocia") | Some("thoughtadjuster") | Some("finaliter") | Some("uversa") | Some("salvington") => {
                    (&URANTIA_CORPUS, "godtexts/urantia.txt.zst", parse_bahai)
                }
                Some("heavensgate") | Some("telah") | Some("tido") | Some("applewhite") | Some("nettles") | Some("nextlevel") | Some("hale-bopp") | Some("halebopp") => {
                    (&HEAVENSGATE_CORPUS, "godtexts/heavensgate.txt.zst", parse_bahai)
                }
                Some("process") | Some("processchurch") | Some("processian") | Some("jehovah") | Some("lucifer") | Some("satan") | Some("devilworship") | Some("robertdevegrimston") | Some("maryannmaclean") => {
                    (&PROCESSCHURCH_CORPUS, "godtexts/processchurch.txt.zst", parse_bahai)
                }
                Some("andraste") | Some("andrastianism") | Some("maker") | Some("chantoflight") | Some("thedas") | Some("ferelden") | Some("orlais") | Some("dragonage") | Some("chantry") => {
                    (&ANDRASTIANISM_CORPUS, "godtexts/andrastianism.txt.zst", parse_bahai)
                }
                Some("orphic") | Some("orpheus") | Some("orphism") | Some("dionysus") | Some("persephone") | Some("hecate") | Some("protogonus") | Some("phanes") | Some("mysteries") | Some("bacchic") => {
                    (&ORPHIC_CORPUS, "godtexts/orphic.txt.zst", parse_bahai)
                }
                Some("neoplatonism") | Some("neoplatonist") | Some("plotinus") | Some("plotinos") | Some("enneads") | Some("theone") | Some("emanation") | Some("nous") | Some("proclus") | Some("porphyry") | Some("iamblichus") => {
                    (&NEOPLATONISM_CORPUS, "godtexts/neoplatonism.txt.zst", parse_bahai)
                }
                Some("kabbalah") | Some("zohar") | Some("kabbalist") | Some("kabbalistic") | Some("sefirot") | Some("sephirot") | Some("simeonbaryochai") | Some("rashbi") | Some("einsof") | Some("soncino") | Some("jewishmysticism") => {
                    (&KABBALAH_CORPUS, "godtexts/kabbalah.txt.zst", parse_bahai)
                }
                Some("dss") | Some("deadseascrolls") | Some("qumran") | Some("essene") | Some("essenes") | Some("communityrule") | Some("damascusdocument") | Some("warscroll") | Some("thanksgivinghymns") | Some("templeoscroll") | Some("vermes") => {
                    (&DSS_CORPUS, "godtexts/deadseascrolls.txt.zst", parse_bahai)
                }
                Some("deuterocanon") | Some("deuterocanonical") | Some("enoch") | Some("1enoch") | Some("bookofjubilees") | Some("jubilees") | Some("tawahedo") | Some("ethiopianorthodox") | Some("charles") => {
                    (&DEUTEROCANON_CORPUS, "godtexts/deuterocanon.txt.zst", parse_bahai)
                }
                Some("acim") | Some("courseinmiracles") | Some("acourseinmiracles") | Some("miracles") | Some("holyspirit") | Some("forgiveness") | Some("atonement") | Some("workbook") | Some("manualforteachers") | Some("urtext") => {
                    (&ACIM_CORPUS, "godtexts/acim.txt.zst", parse_bahai)
                }
                Some("faithism") | Some("oahspe") | Some("jehovih") | Some("kosmon") | Some("newbrough") | Some("saphah") | Some("etherea") | Some("atmospherea") => {
                    (&FAITHISM_CORPUS, "godtexts/faithism.txt.zst", parse_bahai)
                }
                Some("aquarian") | Some("aquariangospel") | Some("dowling") | Some("akashic") | Some("piscean") | Some("aquarianage") => {
                    (&AQUARIAN_CORPUS, "godtexts/aquarian.txt.zst", parse_bahai)
                }
                Some("lawofone") | Some("ramaterial") | Some("confederation") | Some("densities") | Some("wanderer") | Some("harvest") | Some("larussell") | Some("donelkins") | Some("carlarueckert") | Some("racontact") => {
                    (&LAWOFONE_CORPUS, "godtexts/lawofone.txt.zst", parse_bahai)
                }
                Some("iammovement") | Some("iam") | Some("saintgermain") | Some("stgermain") | Some("godfreking") | Some("guyballard") | Some("ballard") | Some("lotusray") | Some("ascendedmaster") | Some("mightyiampresence") => {
                    (&IAMMOVEMENT_CORPUS, "godtexts/iammovement.txt.zst", parse_bahai)
                }
                Some("acadfuturesci") | Some("hurtak") | Some("affs") | Some("academyforfuturescience") | Some("brotherhoodoflight") | Some("ophanimenoch") => {
                    (&ACADFUTURESCI_CORPUS, "godtexts/acadfuturesci.txt.zst", parse_bahai)
                }
                Some("unarius") | Some("ernestnorman") | Some("shamballa") | Some("voiceoferos") | Some("voiceofhermes") | Some("voiceoforion") | Some("voiceofvenus") => {
                    (&UNARIUS_CORPUS, "godtexts/unarius.txt.zst", parse_bahai)
                }
                Some("aetherius") | Some("georgeking") | Some("ninefreedoms") | Some("twelveblessings") | Some("saintgooling") | Some("marssector6") => {
                    (&AETHERIUS_CORPUS, "godtexts/aetherius.txt.zst", parse_bahai)
                }
                Some("anthroposophy") | Some("steiner") | Some("rudolfsteiner") => {
                    (&ANTHROPOSOPHY_CORPUS, "godtexts/anthroposophy.txt.zst", parse_bahai)
                }
                Some("mahikari") | Some("sukyomahikari") | Some("okada") | Some("sukuinushisama") => {
                    (&MAHIKARI_CORPUS, "godtexts/mahikari.txt.zst", parse_bahai)
                }
                Some("radhasoami") | Some("sarbachan") | Some("soamiji") | Some("santmat") => {
                    (&RADHASOAMI_CORPUS, "godtexts/radhasoami.txt.zst", parse_bahai)
                }
                Some("hawaii") | Some("hawaiian") | Some("kumulipo") | Some("kalakaua") => {
                    (&HAWAIIAN_CORPUS, "godtexts/hawaiian.txt.zst", parse_bahai)
                }
                Some("commofchr") | Some("communityofchrist") | Some("rlds") | Some("reorganized") => {
                    (&COMMOFCHR_CORPUS, "godtexts/commofchr.txt.zst", parse_bahai)
                }
                Some("strangite") | Some("strang") | Some("jamesstrang") | Some("lawofthelord") => {
                    (&STRANGITE_CORPUS, "godtexts/strangite.txt.zst", parse_bahai)
                }
                Some("agelesswisdom") | Some("alicebailey") | Some("bailey") | Some("djwhal") | Some("djwhalkhul") | Some("lucistrust") | Some("thehierarchy") | Some("arcane") | Some("arcaneschool") | Some("treatise") | Some("theplan") | Some("sevenrays") => {
                    (&AGELESSWISDOM_CORPUS, "godtexts/agelesswisdom.txt.zst", parse_bahai)
                }
                Some("meherbaba") | Some("meher") | Some("baba") | Some("godspeaks") | Some("avatar") | Some("sufismreoriented") => {
                    (&MEHERBABA_CORPUS, "godtexts/meherbaba.txt.zst", parse_bahai)
                }
                Some("bible") | Some("god") | Some("jesus") | Some("christ") | Some("kjv") | Some("christian") => {
                    (&KJV_CORPUS, "godtexts/kjv.txt.zst", parse_kjv)
                }
                Some(_) => (&KJV_CORPUS, "godtexts/__unknown__", parse_kjv),
                None => {
                    let all = all_corpora();
                    // Knuth multiplicative hash on secs — scrambles the bits so
                    // corpus selection has no visible pattern.
                    let h = secs.wrapping_mul(2654435761);
                    all[(h as usize) % all.len()]
                }
            };

        let (corpus_cell, path, parser) = if path == "godtexts/__unknown__" || !std::path::Path::new(path).exists() {
            let phrase = OUT_TO_LUNCH[(secs as usize).wrapping_mul(2654435761) % OUT_TO_LUNCH.len()];
            let hint = if path == "godtexts/__unknown__" { " (use !searchgod for keywords)" } else { "" };
            ctx.chat(format!("Sorry, that God is {phrase}, another God will answer your mortal cries instead.{hint}"));
            pick_random_available_corpus(secs)
        } else {
            (corpus_cell, path, parser)
        };

        let corpus = match get_corpus(corpus_cell, path, parser).await {
            Ok(c) => c,
            Err(e) => {
                ctx.whisper(format!("Oracle unavailable: {e}"));
                return Ok(());
            }
        };

        if corpus.is_empty() {
            ctx.whisper("The oracle is silent.");
            return Ok(());
        }

        // Drop 4 low noisy bits (TempleOS GOD_BAD_BITS=4), index into corpus.
        let idx = (nanos >> 4) as usize % corpus.len();
        let verse = &corpus[idx];

        let full = format!("[{}] {}", verse.reference, verse.text);
        let out = if full.chars().count() > 240 {
            format!("{}...", full.chars().take(237).collect::<String>())
        } else {
            full
        };

        ctx.chat(out);
        Ok(())
    })
}

async fn get_corpus(
    cell: &'static OnceLock<Vec<Verse>>,
    path: &'static str,
    parser: fn(&str) -> anyhow::Result<Vec<Verse>>,
) -> anyhow::Result<&'static Vec<Verse>> {
    if let Some(c) = cell.get() {
        return Ok(c);
    }
    let verses = tokio::task::spawn_blocking(move || load_corpus_sync(path, parser)).await??;
    Ok(cell.get_or_init(|| verses))
}

fn load_corpus_sync(
    path: &str,
    parser: fn(&str) -> anyhow::Result<Vec<Verse>>,
) -> anyhow::Result<Vec<Verse>> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Cannot open {path}: {e}"))?;
    let bytes = zstd::decode_all(file)?;
    let content = String::from_utf8(bytes)?;
    parser(&content)
}

// ── KJV parser ───────────────────────────────────────────────────────────────
//
// Format (Gutenberg plain text):
//   3+ blank lines before a book title line
//   verse lines start with "digits:digits " (e.g. "1:1 In the beginning")
//   continuation lines wrap without a prefix

fn parse_kjv(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses: Vec<Verse> = Vec::with_capacity(40_000);
    let mut current_book = String::from("Bible");
    let mut current_ref: Option<String> = None;
    let mut current_text = String::new();
    let mut blank_count: usize = 0;

    macro_rules! flush {
        () => {
            if let Some(vref) = current_ref.take() {
                let text = current_text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    verses.push(Verse {
                        reference: format!("{} {}", current_book, vref),
                        text,
                    });
                }
                current_text.clear();
            }
        };
    }

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush!();
            blank_count += 1;
            continue;
        }

        if let Some((maybe_ref, rest)) = trimmed.split_once(' ') {
            if is_verse_ref(maybe_ref) {
                flush!();
                current_ref = Some(maybe_ref.to_string());
                current_text = rest.to_string();
                blank_count = 0;
                continue;
            }
        }

        // 3+ blank lines before a non-verse line = book title.
        if blank_count >= 3 && current_ref.is_none() {
            current_book = clean_book_title(trimmed);
        } else if current_ref.is_some() {
            if !current_text.is_empty() {
                current_text.push(' ');
            }
            current_text.push_str(trimmed);
        }
        blank_count = 0;
    }

    flush!();
    Ok(verses)
}

fn is_verse_ref(s: &str) -> bool {
    let mut saw_colon = false;
    let mut digits_before = false;
    let mut digits_after = false;
    for ch in s.chars() {
        match ch {
            ':' if !saw_colon && digits_before => saw_colon = true,
            ':' => return false,
            '0'..='9' if saw_colon => digits_after = true,
            '0'..='9' => digits_before = true,
            _ => return false,
        }
    }
    saw_colon && digits_before && digits_after
}

fn clean_book_title(raw: &str) -> String {
    // Strip "; or, ..." suffix (e.g. "Ecclesiastes; or, the Preacher")
    let raw = if let Some(i) = raw.find(';') { raw[..i].trim() } else { raw.trim() };
    // "Called X" covers all five books of Moses
    if let Some(pos) = raw.find("Called ") {
        return raw[pos + 7..].trim().to_string();
    }
    let base = raw.strip_prefix("The ").unwrap_or(raw);
    let (s, ord) = strip_ordinal(base);
    // "General " qualifier appears on some epistle titles
    let s = s.strip_prefix("General ").unwrap_or(s);
    if let Some(r) = s.strip_prefix("Gospel According to Saint ") {
        return format!("{}{}", ord, r.trim());
    }
    if let Some(r) = s.strip_prefix("Gospel According to ") {
        return format!("{}{}", ord, r.trim());
    }
    if s.starts_with("Acts of") { return "Acts".to_string(); }
    if s.starts_with("Revelation") { return format!("{}Revelation", ord); }
    if s.starts_with("Lamentations") { return format!("{}Lamentations", ord); }
    if let Some(r) = s.strip_prefix("Book of the Prophet ") {
        return format!("{}{}", ord, r.trim());
    }
    if let Some(r) = s.strip_prefix("Book of ") {
        // "the Chronicles", "the Kings" — strip lowercase "the "
        let name = r.trim().strip_prefix("the ").unwrap_or(r.trim());
        return format!("{}{}", ord, name);
    }
    // Epistles addressed to a church or person: "to the Romans", "to Timothy"
    if let Some(pos) = s.rfind(" to the ") {
        return format!("{}{}", ord, s[pos + 8..].trim());
    }
    if let Some(pos) = s.rfind(" to ") {
        return format!("{}{}", ord, s[pos + 4..].trim());
    }
    if let Some(r) = s.strip_prefix("Epistle of ") {
        return format!("{}{}", ord, r.trim());
    }
    if let Some(r) = s.strip_prefix("Epistle General of ") {
        return format!("{}{}", ord, r.trim());
    }
    format!("{}{}", ord, s.trim())
}

fn strip_ordinal(s: &str) -> (&str, &str) {
    for (word, num) in &[
        ("First ", "1 "), ("Second ", "2 "), ("Third ", "3 "),
        ("Fourth ", "4 "), ("Fifth ", "5 "),
    ] {
        if let Some(rest) = s.strip_prefix(word) {
            return (rest, num);
        }
    }
    (s, "")
}

// ── Bahá'í corpus parser ──────────────────────────────────────────────────────
//
// Combined corpus from Kitab-i-Aqdas, Hidden Words, Gleanings, Seven Valleys,
// and Tablets of Bahá'u'lláh. Format produced by the extraction script:
//   [Reference text]
//   Full passage text on one line.
//   <blank line>

fn parse_merged_mormon(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses = parse_bahai(content)?;
    let path2 = "godtexts/mormon2.txt.zst";
    if std::path::Path::new(path2).exists() {
        if let Ok(file) = std::fs::File::open(path2) {
            if let Ok(bytes) = zstd::decode_all(file) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if let Ok(v2) = parse_bahai(&s) {
                        verses.extend(v2);
                    }
                }
            }
        }
    }
    Ok(verses)
}

fn parse_merged_gnostic(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses = parse_bahai(content)?;
    let path2 = "godtexts/gnosticism2.txt.zst";
    if std::path::Path::new(path2).exists() {
        if let Ok(file) = std::fs::File::open(path2) {
            if let Ok(bytes) = zstd::decode_all(file) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if let Ok(v2) = parse_bahai(&s) {
                        verses.extend(v2);
                    }
                }
            }
        }
    }
    Ok(verses)
}

fn parse_merged_mandaean(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses = parse_bahai(content)?;
    let path2 = "godtexts/mandaeanism2.txt.zst";
    if std::path::Path::new(path2).exists() {
        if let Ok(file) = std::fs::File::open(path2) {
            if let Ok(bytes) = zstd::decode_all(file) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if let Ok(v2) = parse_bahai(&s) {
                        verses.extend(v2);
                    }
                }
            }
        }
    }
    Ok(verses)
}

fn parse_merged_aztec(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses = parse_bahai(content)?;
    let path2 = "godtexts/aztec2.txt.zst";
    if std::path::Path::new(path2).exists() {
        if let Ok(file) = std::fs::File::open(path2) {
            if let Ok(bytes) = zstd::decode_all(file) {
                if let Ok(s) = String::from_utf8(bytes) {
                    if let Ok(v2) = parse_bahai(&s) {
                        verses.extend(v2);
                    }
                }
            }
        }
    }
    Ok(verses)
}

fn parse_bahai(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses: Vec<Verse> = Vec::with_capacity(1_100);
    let mut pending_ref: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            pending_ref = None;
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            pending_ref = Some(inner.to_string());
        } else if let Some(r) = pending_ref.take() {
            verses.push(Verse {
                reference: r,
                text: trimmed.to_string(),
            });
        }
    }
    Ok(verses)
}

// ── Koran parser ─────────────────────────────────────────────────────────────
//
// Format (Rodwell translation, Gutenberg #2800):
//   SURA <Roman>.-<TITLE> [<canonical>.]   — sura header
//   MECCA.-N Verses / MEDINA.-N Verses     — location line (skip)
//   In the Name of God...                  — basmala (count as verse 1)
//   blank-line-separated paragraphs        — individual verses
//   _______________________                — footnote separator (stop collecting)
//
// Verse numbers are not embedded; we count blank-separated blocks within each sura.
// Inline footnote markers (bare trailing digits, e.g. "path,2") are left as-is.

fn parse_koran(content: &str) -> anyhow::Result<Vec<Verse>> {
    let mut verses: Vec<Verse> = Vec::with_capacity(8_000);
    let mut sura_number: usize = 0;
    let mut verse_number: usize = 0;
    let mut in_verses = false;
    let mut current_text = String::new();

    macro_rules! flush {
        () => {
            let text = current_text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() && sura_number > 0 {
                verse_number += 1;
                verses.push(Verse {
                    reference: format!("Quran {}:{}", sura_number, verse_number),
                    text,
                });
            }
            current_text.clear();
        };
    }

    for line in content.lines() {
        let trimmed = line.trim();

        // New sura header.
        if trimmed.starts_with("SURA ") && trimmed[5..].starts_with(|c: char| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'M')) {
            flush!();
            sura_number += 1;
            verse_number = 0;
            in_verses = false;
            continue;
        }

        // Footnote separator — stop collecting for this sura.
        if trimmed == "_______________________" {
            flush!();
            in_verses = false;
            continue;
        }

        if !in_verses {
            // Skip until first content line after header (location line, blank lines).
            if trimmed.is_empty() || (trimmed.contains(" Verses") && (trimmed.contains("MECCA") || trimmed.contains("MEDINA"))) {
                continue;
            }
            in_verses = true;
        }

        if !in_verses {
            continue;
        }

        if trimmed.is_empty() {
            flush!();
        } else {
            if !current_text.is_empty() {
                current_text.push(' ');
            }
            current_text.push_str(trimmed);
        }
    }

    flush!();
    Ok(verses)
}

fn search_corpora(keyword: &str) -> Vec<&'static Verse> {
    let kw = keyword.to_lowercase();
    let mut hits = Vec::new();
    for (lock, _, _) in all_corpora() {
        if let Some(verses) = lock.get() {
            for verse in verses {
                if verse.text.to_lowercase().contains(&kw) {
                    hits.push(verse);
                }
            }
        }
    }
    hits
}

fn find_by_reference(query: &str) -> Vec<&'static Verse> {
    let q = query.to_lowercase();
    let mut hits = Vec::new();
    for (lock, _, _) in all_corpora() {
        if let Some(verses) = lock.get() {
            for verse in verses {
                if verse.reference.to_lowercase().contains(&q) {
                    hits.push(verse);
                }
            }
        }
    }
    hits
}

fn godverse(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper("Usage: !godverse <reference>");
            return Ok(());
        }
        let query = ctx.args.join(" ");
        let hits = find_by_reference(&query);
        if hits.is_empty() {
            ctx.chat("No verse found matching that reference.".to_string());
            return Ok(());
        }
        let pick = match hits.iter().find(|v| v.reference.eq_ignore_ascii_case(&query)) {
            Some(v) => *v,
            None => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                let h = now.as_secs().wrapping_mul(2654435761).wrapping_add(now.subsec_nanos() as u64);
                hits[(h as usize) % hits.len()]
            }
        };
        ctx.chat(make_output_with_keyword(&pick.reference, &pick.text, ""));
        Ok(())
    })
}

fn make_output_with_keyword(reference: &str, text: &str, keyword: &str) -> String {
    const MAX: usize = 240;
    const WING: usize = 90;

    let full = format!("[{reference}] {text}");
    if full.chars().count() <= MAX {
        return full;
    }

    let kw_lower = keyword.to_lowercase();
    let text_lower = text.to_lowercase();
    let kw_pos = text_lower.find(&kw_lower).unwrap_or(0);

    let raw_start = kw_pos.saturating_sub(WING);
    let raw_end = (kw_pos + keyword.len() + WING).min(text.len());

    let start = (0..=raw_start).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    let end = (raw_end..=text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());

    let snippet = &text[start..end];
    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < text.len() { "..." } else { "" };

    let candidate = format!("[{reference}] {prefix}{snippet}{suffix}");
    if candidate.chars().count() <= MAX {
        candidate
    } else {
        format!("{}...", candidate.chars().take(MAX - 3).collect::<String>())
    }
}

fn pick_random_available_corpus(seed: u64) -> CorpusEntry {
    let all = all_corpora();
    let available: Vec<CorpusEntry> = all
        .iter()
        .copied()
        .filter(|(_, path, _)| std::path::Path::new(path).exists())
        .collect();
    let h = seed.wrapping_mul(2654435761);
    if available.is_empty() {
        all[(h as usize) % all.len()]
    } else {
        available[(h as usize) % available.len()]
    }
}

struct GodStats {
    corpora_loaded: u32,
    total_verses: usize,
    total_bytes: usize,
    total_compressed_bytes: usize,
    elapsed: std::time::Duration,
}

static GOD_STATS: OnceLock<GodStats> = OnceLock::new();

// Total unique god/keyword aliases across all match arms in `execute`'s god_arg match.
// Manual count, like the all_corpora() array size — bump when aliases are added/removed.
const KNOWN_GODS_COUNT: usize = 645;

pub fn preload_all_corpora() {
    let t = std::time::Instant::now();
    let mut loaded = 0u32;
    let mut total_verses = 0usize;
    let mut total_bytes = 0usize;
    let mut total_compressed_bytes = 0usize;
    for (lock, path, parser) in all_corpora() {
        if lock.get().is_none() {
            if let Ok(meta) = std::fs::metadata(path) {
                total_compressed_bytes += meta.len() as usize;
            }
            if let Ok(file) = std::fs::File::open(path) {
                if let Ok(bytes) = zstd::decode_all(file) {
                    total_bytes += bytes.len();
                }
            }
            match load_corpus_sync(path, parser) {
                Ok(verses) => {
                    total_verses += verses.len();
                    let _ = lock.get_or_init(|| verses);
                    loaded += 1;
                }
                Err(_) => {}
            }
        }
    }
    let elapsed = t.elapsed();
    let _ = GOD_STATS.set(GodStats { corpora_loaded: loaded, total_verses, total_bytes, total_compressed_bytes, elapsed });
    crate::structure::logger::info(format!("Loaded {loaded} god corpora in {:?}", elapsed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_corpus_load() {
        let t = std::time::Instant::now();
        for (lock, path, parser) in all_corpora() {
            let t2 = std::time::Instant::now();
            lock.get_or_init(|| parser(path).expect("load failed"));
            println!("{path}: {:?}", t2.elapsed());
        }
        println!("TOTAL: {:?}", t.elapsed());
    }
}
