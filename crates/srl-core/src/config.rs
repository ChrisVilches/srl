//! The configuration in `.srl/config.toml` (see `docs/store.md`,
//! "Configuration").

use std::collections::BTreeMap;
use std::fs;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use jiff::civil::{Date, DateTime};
use serde::Deserialize;

use crate::domain::item::is_id_text;
use crate::store::nfc;
use crate::{Error, Result, Store};

/// Every setting of `config.toml`. A key the file leaves out has its
/// default; a key the file has but `srl` doesn't know is an error, so that
/// a typo never goes unnoticed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The sources of items, by name (see `docs/sources.md`).
    pub sources: BTreeMap<String, SourceConfig>,
    pub schedule: Schedule,
    /// The kinds that are programs: each name with its command, the
    /// program and then its arguments (see `docs/cards.md`, "Kind
    /// programs").
    pub kinds: BTreeMap<String, Vec<String>>,
    pub kind_programs: KindPrograms,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sources: BTreeMap::from([("notes".to_owned(), SourceConfig::default())]),
            schedule: Schedule::default(),
            kinds: BTreeMap::new(),
            kind_programs: KindPrograms::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KindPrograms {
    /// The seconds a kind program may take to make a card.
    pub card_timeout: u64,
}

impl Default for KindPrograms {
    fn default() -> Self {
        Self { card_timeout: 60 }
    }
}

/// What a source is: one of the built-in drivers, or a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Driver {
    /// Notes of srl blocks (see `docs/notes.md`, "The srl block syntax").
    Markdown,
    /// Notes of JSON lines (see `docs/notes.md`, "The JSONL syntax").
    Jsonl,
    /// A source program (see `docs/sources.md`, "Source programs").
    Program {
        /// The program, then its arguments.
        command: Vec<String>,
        /// Every key of the source but `program` and `prefix`: the
        /// program's own, given to it as they are.
        settings: serde_json::Map<String, serde_json::Value>,
    },
}

impl Driver {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Jsonl => "jsonl",
            Self::Program { .. } => "program",
        }
    }

    /// The globs of the notes a source of this driver reads when it gives
    /// no `include`.
    fn default_include(&self) -> &'static str {
        match self {
            Self::Markdown => "**/*.md",
            Self::Jsonl | Self::Program { .. } => "**/*.jsonl",
        }
    }
}

/// One source of `[sources]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfig {
    pub driver: Driver,
    /// Where the source's collections are among the store's addresses:
    /// `""`, or folders ending in `/`, such as `vocab/`.
    pub prefix: String,
    /// Globs of notes, relative to the store's folder; the driver's own
    /// when not given. Only for the notes' drivers.
    include: Option<Vec<String>>,
    /// Globs of notes to ignore, relative to the store's folder. They win
    /// over `include`. Only for the notes' drivers.
    pub exclude: Vec<String>,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            driver: Driver::Markdown,
            prefix: String::new(),
            include: None,
            exclude: Vec::new(),
        }
    }
}

/// A source as written in `config.toml`, before its keys are sorted out.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesSource {
    driver: String,
    #[serde(default)]
    prefix: String,
    include: Option<Vec<String>>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl<'de> Deserialize<'de> for SourceConfig {
    /// A source of a notes' driver has `driver`, and may have `prefix`,
    /// `include` and `exclude`, nothing else. A source program has
    /// `program`, and may have `prefix`; every other key is the program's.
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut table = toml::Table::deserialize(deserializer)?;
        let Some(program) = table.remove("program") else {
            let source =
                NotesSource::deserialize(toml::Value::Table(table)).map_err(D::Error::custom)?;
            let driver = match source.driver.as_str() {
                "markdown" => Driver::Markdown,
                "jsonl" => Driver::Jsonl,
                other => {
                    return Err(D::Error::custom(format!(
                        "unknown driver `{other}`; use markdown or jsonl, or name a program \
                         with `program`"
                    )));
                }
            };
            return Ok(Self {
                driver,
                prefix: source.prefix,
                include: source.include,
                exclude: source.exclude,
            });
        };
        if table.contains_key("driver") {
            return Err(D::Error::custom(
                "a source has a `driver` or a `program`, not both",
            ));
        }
        let command = Vec::<String>::deserialize(program).map_err(|error| {
            D::Error::custom(format!(
                "`program` must be a list of the program and its arguments ({error})"
            ))
        })?;
        let prefix = match table.remove("prefix") {
            None => String::new(),
            Some(prefix) => String::deserialize(prefix)
                .map_err(|error| D::Error::custom(format!("`prefix` must be text ({error})")))?,
        };
        let settings = serde_json::to_value(&table)
            .map_err(D::Error::custom)?
            .as_object()
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            driver: Driver::Program { command, settings },
            prefix,
            include: None,
            exclude: Vec::new(),
        })
    }
}

impl SourceConfig {
    /// A source of `driver`, with its default globs and no prefix.
    pub fn of(driver: Driver) -> Self {
        Self {
            driver,
            ..Self::default()
        }
    }

    /// This source with `include` as its globs of notes.
    #[must_use]
    pub fn including(self, include: &[&str]) -> Self {
        Self {
            include: Some(include.iter().map(|glob| (*glob).to_owned()).collect()),
            ..self
        }
    }

    /// The globs of notes: those given, or the driver's.
    pub fn include(&self) -> Vec<String> {
        self.include
            .clone()
            .unwrap_or_else(|| vec![self.driver.default_include().to_owned()])
    }

    /// The `include` globs, compiled. They were checked when the
    /// configuration was read.
    pub fn include_set(&self) -> GlobSet {
        globs(&self.include()).expect("the include globs should have been checked when read")
    }

    /// The `exclude` globs, compiled. They were checked when the
    /// configuration was read.
    pub fn exclude_set(&self) -> GlobSet {
        globs(&self.exclude).expect("the exclude globs should have been checked when read")
    }
}

/// Globs where `*` stops at `/` and `**` crosses it. Patterns are compared
/// in NFC (see `docs/store.md`, "File names"). Errors are plain reasons.
pub(crate) fn globs(patterns: &[impl AsRef<str>]) -> std::result::Result<GlobSet, String> {
    patterns
        .iter()
        .map(|pattern| {
            let pattern = pattern.as_ref();
            GlobBuilder::new(&nfc(pattern))
                .literal_separator(true)
                .build()
                .map_err(|error| format!("`{pattern}` is not a valid glob ({error})"))
        })
        .try_fold(GlobSetBuilder::new(), |mut set, glob| {
            set.add(glob?);
            Ok::<_, String>(set)
        })?
        .build()
        .map_err(|error| format!("the globs are not valid ({error})"))
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Schedule {
    pub desired_retention: f64,
    pub day_starts_at: u8,
    pub new_per_day: u32,
    pub max_skip: u32,
    pub leech_lapses: u32,
    pub mature_days: u32,
    pub weights: Option<Vec<f64>>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            desired_retention: 0.9,
            day_starts_at: 4,
            new_per_day: 10,
            max_skip: 0,
            leech_lapses: 8,
            mature_days: 21,
            weights: None,
        }
    }
}

impl Schedule {
    /// The study day that the local time `now` belongs to: its date, or the
    /// day before if it is earlier than `day_starts_at` o'clock (see
    /// `docs/scheduling.md`, "Days, not minutes").
    pub fn study_day(&self, now: DateTime) -> Date {
        if i16::from(now.hour()) < i16::from(self.day_starts_at) {
            now.date()
                .yesterday()
                .expect("a local date should have a day before it")
        } else {
            now.date()
        }
    }
}

impl Config {
    /// Reads the store's `config.toml`.
    pub fn load(store: &Store) -> Result<Self> {
        let file = store.config_file();
        let text = fs::read_to_string(&file).map_err(Error::io(&file))?;
        Self::parse(&text).map_err(|reason| Error::InvalidConfig { file, reason })
    }

    /// Parses the text of `config.toml`. Errors are plain reasons.
    fn parse(text: &str) -> std::result::Result<Self, String> {
        let table =
            toml::from_str::<toml::Table>(text).map_err(|error| error.message().to_owned())?;
        if table.contains_key("notes") {
            return Err(NOTES_MOVED.to_owned());
        }
        let config = toml::from_str::<Self>(text).map_err(|error| error.message().to_owned())?;
        let schedule = &config.schedule;
        if !(schedule.desired_retention > 0.0 && schedule.desired_retention < 1.0) {
            return Err("`schedule.desired_retention` must be between 0 and 1".to_owned());
        }
        if schedule.day_starts_at > 23 {
            return Err("`schedule.day_starts_at` must be an hour from 0 to 23".to_owned());
        }
        for (name, source) in &config.sources {
            check_source(name, source)?;
        }
        if config.kinds.contains_key("qa") {
            return Err("`kinds.qa`: qa is built in, so it can't be a program".to_owned());
        }
        if let Some((name, _)) = config.kinds.iter().find(|(_, command)| command.is_empty()) {
            return Err(format!(
                "`kinds.{name}` is empty; give the program and its arguments, such as \
                 [\"python3\", \"kinds/{name}.py\"]"
            ));
        }
        if config.kind_programs.card_timeout == 0 {
            return Err("`kind_programs.card_timeout` must be at least 1 second".to_owned());
        }
        Ok(config)
    }
}

/// What `[notes]` became, for a configuration that still has it.
const NOTES_MOVED: &str = "`[notes]` is not read: notes are a source of items now; write \
    `[sources.notes]` with `driver = \"markdown\"` and the same `include` and `exclude`, and \
    give JSONL notes a source of their own, such as `[sources.data]` with `driver = \"jsonl\"` \
    and `include = [\"**/*.jsonl\"]` (see docs/sources.md)";

/// Checks the source `name` of `[sources]`: its name, prefix and globs.
/// Errors are plain reasons.
fn check_source(name: &str, source: &SourceConfig) -> std::result::Result<(), String> {
    if !is_id_text(name) {
        return Err(format!(
            "`sources.{name}`: a source's name is made of words of a-z and 0-9 joined by -, \
             such as notes or jp-vocab"
        ));
    }
    let prefix = &source.prefix;
    if !prefix.is_empty() {
        let folders = prefix.strip_suffix('/').unwrap_or_default();
        let invalid = folders.is_empty()
            || prefix.contains('\\')
            || prefix.contains(':')
            || folders.split('/').any(|part| {
                part.is_empty() || part == "." || part == ".." || part.starts_with('.')
            });
        if invalid {
            return Err(format!(
                "`sources.{name}.prefix`: a prefix is \"\" or folders ending in /, such as \
                 \"vocab/\", without empty parts, . or .., or a folder starting with ."
            ));
        }
    }
    if let Driver::Program { command, .. } = &source.driver
        && command.is_empty()
    {
        return Err(format!(
            "`sources.{name}.program` is empty; give the program and its arguments, such as \
             [\"python3\", \"sources/{name}.py\"]"
        ));
    }
    globs(&source.include()).map_err(|reason| format!("`sources.{name}.include`: {reason}"))?;
    globs(&source.exclude).map_err(|reason| format!("`sources.{name}.exclude`: {reason}"))?;
    Ok(())
}

/// Sources that read the notes `include` matches as the extension of each
/// glob says: those ending with `.jsonl` in a `jsonl` source named `data`,
/// the others in a `markdown` source named `notes`. For tests.
#[cfg(test)]
pub(crate) fn sources_of(include: &[&str]) -> BTreeMap<String, SourceConfig> {
    let (jsonl, markdown): (Vec<&str>, Vec<&str>) = include.iter().partition(|glob| {
        std::path::Path::new(glob)
            .extension()
            .is_some_and(|ext| ext == "jsonl")
    });
    let mut sources = BTreeMap::new();
    if !markdown.is_empty() {
        sources.insert(
            "notes".to_owned(),
            SourceConfig::of(Driver::Markdown).including(&markdown),
        );
    }
    if !jsonl.is_empty() {
        sources.insert(
            "data".to_owned(),
            SourceConfig::of(Driver::Jsonl).including(&jsonl),
        );
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_file_gives_the_defaults() {
        assert_eq!(
            Config::parse(crate::store::DEFAULT_CONFIG).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn given_keys_replace_defaults() {
        let config =
            Config::parse("[sources.notes]\ndriver = \"markdown\"\nexclude = [\"templates/**\"]\n")
                .unwrap();
        let notes = &config.sources["notes"];
        assert_eq!(notes.exclude, ["templates/**"]);
        assert_eq!(config.schedule, Schedule::default());
        assert!(notes.exclude_set().is_match("templates/a/b.md"));
        assert_eq!(notes.include(), ["**/*.md"]);
        assert_eq!(notes.prefix, "");
    }
    #[test]
    fn kinds_name_commands() {
        let config = Config::parse(
            "[kinds]\nja-words = [\"python3\", \"kinds/ja_words.py\"]\n\n[kind_programs]\ncard_timeout = 5\n",
        )
        .unwrap();
        assert_eq!(config.kinds["ja-words"], ["python3", "kinds/ja_words.py"]);
        assert_eq!(config.kind_programs.card_timeout, 5);
        assert_eq!(Config::default().kind_programs.card_timeout, 60);
    }

    #[test]
    fn sources_replace_the_default_source() {
        let config = Config::parse(
            "[sources.notes]\ndriver = \"markdown\"\ninclude = [\"**/*.txt\"]\n\n[sources.data]\ndriver = \"jsonl\"\nprefix = \"data/\"\n",
        )
        .unwrap();
        let names = config.sources.keys().map(String::as_str);
        assert_eq!(names.collect::<Vec<_>>(), ["data", "notes"]);
        let notes = &config.sources["notes"];
        assert!(notes.include_set().is_match("a/b.txt"));
        assert!(!notes.include_set().is_match("a/b.md"));
        assert_eq!(config.sources["data"].include(), ["**/*.jsonl"]);
        assert_eq!(config.sources["data"].prefix, "data/");
        assert_eq!(Config::default().sources["notes"], SourceConfig::default());
    }

    #[test]
    fn a_program_source_keeps_its_own_keys_as_its_settings() {
        let config = Config::parse(
            "[sources.vocab]\nprogram = [\"python3\", \"vocab.py\"]\nprefix = \"vocab/\"\ndatabase = \"vocab.json\"\nretries = 3\n",
        )
        .unwrap();
        let vocab = &config.sources["vocab"];
        assert_eq!(vocab.prefix, "vocab/");
        let Driver::Program { command, settings } = &vocab.driver else {
            panic!("not a program: {vocab:?}");
        };
        assert_eq!(command, &["python3", "vocab.py"]);
        assert_eq!(
            serde_json::Value::Object(settings.clone()),
            serde_json::json!({"database": "vocab.json", "retries": 3})
        );
        for text in [
            "[sources.vocab]\nprogram = []\n",
            "[sources.vocab]\nprogram = \"vocab.py\"\n",
            "[sources.vocab]\nprogram = [\"vocab.py\"]\ndriver = \"jsonl\"\n",
        ] {
            assert!(Config::parse(text).is_err(), "accepted {text}");
        }
    }
    #[test]
    fn notes_is_refused_with_how_to_write_it_as_a_source() {
        let error = Config::parse("[notes]\ninclude = [\"**/*.md\"]\n").unwrap_err();
        assert!(error.contains("[sources.notes]"), "{error}");
    }

    #[test]
    fn source_names_prefixes_and_globs_are_checked() {
        let source = |name: &str, rest: &str| {
            Config::parse(&format!(
                "[sources.{name}]\ndriver = \"markdown\"\n{rest}\n"
            ))
        };
        assert!(source("jp-vocab", "prefix = \"vocab/n5/\"").is_ok());
        for (name, rest) in [
            ("Notes", ""),
            ("a_b", ""),
            ("notes", "prefix = \"vocab\""),
            ("notes", "prefix = \"/\""),
            ("notes", "prefix = \"a//\""),
            ("notes", "prefix = \"../a/\""),
            ("notes", "prefix = \".hidden/\""),
            ("notes", "include = [\"[\"]"),
            ("notes", "exclude = [\"[\"]"),
            ("notes", "drvier = \"markdown\""),
        ] {
            assert!(source(name, rest).is_err(), "accepted {name}: {rest}");
        }
        assert!(Config::parse("[sources.notes]\ndriver = \"org\"\n").is_err());
        assert!(Config::parse("[sources.notes]\ninclude = []\n").is_err());
    }
    #[test]
    fn a_study_day_starts_at_day_starts_at() {
        let schedule = Schedule::default();
        let day = jiff::civil::date(2026, 10, 26);
        assert_eq!(
            schedule.study_day(day.at(3, 59, 0, 0)),
            day.yesterday().unwrap()
        );
        assert_eq!(schedule.study_day(day.at(4, 0, 0, 0)), day);
        assert_eq!(schedule.study_day(day.at(23, 59, 0, 0)), day);
    }

    #[test]
    fn unknown_keys_and_bad_values_are_errors() {
        for text in [
            "[drills]\ntimeout_seconds = 3\n",
            "[shedule]\n",
            "[schedule]\nday_starts_at = 24\n",
            "[schedule]\ndesired_retention = 1.5\n",
            "[schedule]\nnew_per_day = \"ten\"\n",
            "[study]\ncontext = \"prose\"\n",
            "[sources.notes]\ndriver = \"markdown\"\nexclude = [\"[\"]\n",
            "[kinds]\nqa = [\"python3\", \"qa.py\"]\n",
            "[kinds]\nconj = []\n",
            "[kinds]\nconj = \"conj.py\"\n",
            "[kind_programs]\ncard_timeout = 0\n",
            "[kind_programs]\nids_timeout = 5\n",
            "not toml",
        ] {
            assert!(Config::parse(text).is_err(), "accepted:\n{text}");
        }
    }
}
