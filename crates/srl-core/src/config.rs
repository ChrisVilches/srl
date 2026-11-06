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
    /// The files items may name with `see` (see `docs/items.md`,
    /// "Context").
    pub context: ContextConfig,
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
            sources: BTreeMap::from([(DEFAULT_SOURCE.to_owned(), SourceConfig::default())]),
            context: ContextConfig::default(),
            schedule: Schedule::default(),
            kinds: BTreeMap::new(),
            kind_programs: KindPrograms::default(),
        }
    }
}

/// The name of the one source of a store without `[sources]`.
pub(crate) const DEFAULT_SOURCE: &str = "items";

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

/// `[context]`: the files of the store that items may name with `see`,
/// its study material, as globs relative to the store's folder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContextConfig {
    pub include: Vec<String>,
    /// They win over `include`.
    pub exclude: Vec<String>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            include: vec!["**/*.md".to_owned(), "**/*.txt".to_owned()],
            exclude: Vec::new(),
        }
    }
}

impl ContextConfig {
    /// The `include` globs, compiled. They were checked when the
    /// configuration was read.
    pub fn include_set(&self) -> GlobSet {
        globs(&self.include).expect("the context globs should have been checked when read")
    }

    /// The `exclude` globs, compiled. They were checked when the
    /// configuration was read.
    pub fn exclude_set(&self) -> GlobSet {
        globs(&self.exclude).expect("the context globs should have been checked when read")
    }
}

/// What a source is: the built-in driver, or a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Driver {
    /// Collections that are files of the store, in the formats of
    /// `docs/formats.md`.
    Filesystem {
        /// Globs of the files, relative to the store's folder.
        include: Vec<String>,
        /// Globs of the files to ignore. They win over `include`.
        exclude: Vec<String>,
    },
    /// A source program (see `docs/sources.md`, "Source programs").
    Program {
        /// The program, then its arguments.
        command: Vec<String>,
        /// Every key of the source but `program`: the program's own,
        /// given to it as they are.
        settings: serde_json::Map<String, serde_json::Value>,
    },
}

impl Driver {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Filesystem { .. } => "filesystem",
            Self::Program { .. } => "program",
        }
    }
}

/// The globs of the files a `filesystem` source reads when it gives no
/// `include`.
pub(crate) const DEFAULT_INCLUDE: &[&str] = &["**/*.jsonl"];

/// One source of `[sources]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfig {
    pub driver: Driver,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self::filesystem(DEFAULT_INCLUDE)
    }
}

/// A `filesystem` source as written in `config.toml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilesystemSource {
    driver: String,
    include: Option<Vec<String>>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl<'de> Deserialize<'de> for SourceConfig {
    /// A `filesystem` source has `driver`, and may have `include` and
    /// `exclude`, nothing else. A source program has `program`; every
    /// other key is the program's, which checks them itself.
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut table = toml::Table::deserialize(deserializer)?;
        if table.contains_key("prefix") {
            return Err(D::Error::custom(
                "`prefix` is not read: sources are not mounted under prefixes; the address of a \
                 collection is its name, as its source gives it",
            ));
        }
        let Some(program) = table.remove("program") else {
            let source = FilesystemSource::deserialize(toml::Value::Table(table))
                .map_err(D::Error::custom)?;
            return match source.driver.as_str() {
                "filesystem" => Ok(Self {
                    driver: Driver::Filesystem {
                        include: source.include.unwrap_or_else(|| {
                            DEFAULT_INCLUDE
                                .iter()
                                .map(|glob| (*glob).to_owned())
                                .collect()
                        }),
                        exclude: source.exclude,
                    },
                }),
                "markdown" | "jsonl" => Err(D::Error::custom(format!(
                    "the driver `{}` is gone: items are kept in structured files, which \
                     `driver = \"filesystem\"` reads (see docs/formats.md), and Markdown is \
                     study material that items name with `see`",
                    source.driver
                ))),
                other => Err(D::Error::custom(format!(
                    "unknown driver `{other}`; use filesystem, or name a program with `program`"
                ))),
            };
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
        let settings = serde_json::to_value(&table)
            .map_err(D::Error::custom)?
            .as_object()
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            driver: Driver::Program { command, settings },
        })
    }
}

impl SourceConfig {
    /// A `filesystem` source of the files `include` matches.
    pub fn filesystem(include: &[&str]) -> Self {
        Self {
            driver: Driver::Filesystem {
                include: include.iter().map(|glob| (*glob).to_owned()).collect(),
                exclude: Vec::new(),
            },
        }
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
        globs(&config.context.include).map_err(|reason| format!("`context.include`: {reason}"))?;
        globs(&config.context.exclude).map_err(|reason| format!("`context.exclude`: {reason}"))?;
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
const NOTES_MOVED: &str = "`[notes]` is not read: items are kept in sources, such as \
    `[sources.items]` with `driver = \"filesystem\"` and its `include` and `exclude`, and the \
    files items name with `see` are set in `[context]` (see docs/sources.md)";

/// Checks the source `name` of `[sources]`: its name and globs. Errors are
/// plain reasons.
fn check_source(name: &str, source: &SourceConfig) -> std::result::Result<(), String> {
    if !is_id_text(name) {
        return Err(format!(
            "`sources.{name}`: a source's name is made of words of a-z and 0-9 joined by -, \
             such as items or jp-vocab"
        ));
    }
    match &source.driver {
        Driver::Program { command, .. } if command.is_empty() => Err(format!(
            "`sources.{name}.program` is empty; give the program and its arguments, such as \
             [\"python3\", \"sources/{name}.py\"]"
        )),
        Driver::Program { .. } => Ok(()),
        Driver::Filesystem { include, exclude } => {
            globs(include).map_err(|reason| format!("`sources.{name}.include`: {reason}"))?;
            globs(exclude).map_err(|reason| format!("`sources.{name}.exclude`: {reason}"))?;
            Ok(())
        }
    }
}

/// One `filesystem` source, named `items`, of the files `include` matches.
/// For tests.
#[cfg(test)]
pub(crate) fn sources_of(include: &[&str]) -> BTreeMap<String, SourceConfig> {
    BTreeMap::from([(DEFAULT_SOURCE.to_owned(), SourceConfig::filesystem(include))])
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
        let config = Config::parse(
            "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"templates/**\"]\n",
        )
        .unwrap();
        assert_eq!(
            config.sources["items"].driver,
            Driver::Filesystem {
                include: vec!["**/*.jsonl".to_owned()],
                exclude: vec!["templates/**".to_owned()],
            }
        );
        assert_eq!(config.schedule, Schedule::default());
        assert_eq!(config.context, ContextConfig::default());
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
            "[sources.cards]\ndriver = \"filesystem\"\ninclude = [\"cards/**/*.jsonl\"]\n",
        )
        .unwrap();
        let names = config.sources.keys().map(String::as_str);
        assert_eq!(names.collect::<Vec<_>>(), ["cards"]);
        assert_eq!(
            Config::default().sources["items"],
            SourceConfig::filesystem(&["**/*.jsonl"])
        );
    }

    #[test]
    fn context_globs_are_read_and_checked() {
        let config =
            Config::parse("[context]\ninclude = [\"notes/**\"]\nexclude = [\"**/*.jsonl\"]\n")
                .unwrap();
        assert!(config.context.include_set().is_match("notes/a.pdf"));
        assert!(config.context.exclude_set().is_match("notes/a.jsonl"));
        let defaults = ContextConfig::default();
        assert!(defaults.include_set().is_match("a/b.md"));
        assert!(defaults.include_set().is_match("b.txt"));
        assert!(!defaults.include_set().is_match("b.jsonl"));
        for text in [
            "[context]\ninclude = [\"[\"]\n",
            "[context]\nexclude = [\"[\"]\n",
            "[context]\nglobs = []\n",
        ] {
            assert!(Config::parse(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn a_program_source_keeps_its_own_keys_as_its_settings() {
        let config = Config::parse(
            "[sources.vocab]\nprogram = [\"python3\", \"vocab.py\"]\ndatabase = \"vocab.json\"\nretries = 3\n",
        )
        .unwrap();
        let vocab = &config.sources["vocab"];
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
            "[sources.vocab]\nprogram = [\"vocab.py\"]\ndriver = \"filesystem\"\n",
            "[sources.vocab]\nprogram = [\"vocab.py\"]\nprefix = \"vocab/\"\n",
        ] {
            assert!(Config::parse(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn notes_is_refused_with_how_to_write_it_as_a_source() {
        let error = Config::parse("[notes]\ninclude = [\"**/*.md\"]\n").unwrap_err();
        assert!(error.contains("[sources.items]"), "{error}");
    }

    #[test]
    fn the_drivers_that_are_gone_say_what_replaced_them() {
        for driver in ["markdown", "jsonl"] {
            let error =
                Config::parse(&format!("[sources.notes]\ndriver = \"{driver}\"\n")).unwrap_err();
            assert!(error.contains("filesystem"), "{error}");
        }
        let error =
            Config::parse("[sources.items]\ndriver = \"filesystem\"\nprefix = \"\"\n").unwrap_err();
        assert!(error.contains("prefix"), "{error}");
    }

    #[test]
    fn source_names_and_globs_are_checked() {
        let source = |name: &str, rest: &str| {
            Config::parse(&format!(
                "[sources.{name}]\ndriver = \"filesystem\"\n{rest}\n"
            ))
        };
        assert!(source("jp-vocab", "include = [\"vocab/**/*.jsonl\"]").is_ok());
        for (name, rest) in [
            ("Items", ""),
            ("a_b", ""),
            ("items", "include = [\"[\"]"),
            ("items", "exclude = [\"[\"]"),
            ("items", "inclde = [\"**\"]"),
            ("items", "database = \"db.json\""),
        ] {
            assert!(source(name, rest).is_err(), "accepted {name}: {rest}");
        }
        assert!(Config::parse("[sources.items]\ndriver = \"org\"\n").is_err());
        assert!(Config::parse("[sources.items]\ninclude = []\n").is_err());
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
            "[sources.items]\ndriver = \"filesystem\"\nexclude = [\"[\"]\n",
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
