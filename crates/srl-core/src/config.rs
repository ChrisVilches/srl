//! The configuration in `.srl/config.toml` (see `docs/store.md`,
//! "Configuration").

use std::fs;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use jiff::civil::{Date, DateTime};
use serde::Deserialize;

use crate::store::nfc;
use crate::{ContextMode, Error, Result, Store};

/// Every setting of `config.toml`. A key the file leaves out has its
/// default; a key the file has but `srl` doesn't know is an error, so that
/// a typo never goes unnoticed.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub notes: Notes,
    pub schedule: Schedule,
    pub study: Study,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notes {
    /// Globs of notes to ignore, relative to the store's folder.
    pub exclude: Vec<String>,
}

impl Notes {
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Study {
    /// How context is shown (see `docs/notes.md`, "Context").
    pub context: ContextMode,
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
        let config = toml::from_str::<Self>(text).map_err(|error| error.message().to_owned())?;
        let schedule = &config.schedule;
        if !(schedule.desired_retention > 0.0 && schedule.desired_retention < 1.0) {
            return Err("`schedule.desired_retention` must be between 0 and 1".to_owned());
        }
        if schedule.day_starts_at > 23 {
            return Err("`schedule.day_starts_at` must be an hour from 0 to 23".to_owned());
        }
        globs(&config.notes.exclude).map_err(|reason| format!("`notes.exclude`: {reason}"))?;
        Ok(config)
    }
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
            Config::parse("[notes]\nexclude = [\"templates/**\"]\n[study]\ncontext = \"full\"\n")
                .unwrap();
        assert_eq!(config.notes.exclude, ["templates/**"]);
        assert_eq!(config.study.context, ContextMode::Full);
        assert_eq!(config.schedule, Schedule::default());
        assert!(config.notes.exclude_set().is_match("templates/a/b.md"));
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
            "[study]\ncontext = \"some\"\n",
            "[notes]\nexclude = [\"[\"]\n",
            "not toml",
        ] {
            assert!(Config::parse(text).is_err(), "accepted:\n{text}");
        }
    }
}
