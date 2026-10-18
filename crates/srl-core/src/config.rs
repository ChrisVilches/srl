//! The configuration in `srl.toml` (see `docs/store.md`, "Configuration").

use std::fs;
use std::time::Duration;

use jiff::civil::{Date, DateTime};
use serde::Deserialize;

use crate::{Error, Result, Store};

/// Every setting of `srl.toml`. A key the file leaves out has its default;
/// a key the file has but `srl` doesn't know is an error, so that a typo
/// never goes unnoticed.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schedule: Schedule,
    pub drills: Drills,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Drills {
    pub timeout_seconds: u64,
}

impl Default for Drills {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
        }
    }
}

impl Drills {
    /// How long a drill may run before it counts as failed.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

impl Config {
    /// Reads the store's `srl.toml`.
    pub fn load(store: &Store) -> Result<Self> {
        let file = store.config_file();
        let text = fs::read_to_string(&file).map_err(Error::io(&file))?;
        Self::parse(&text).map_err(|reason| Error::InvalidConfig { file, reason })
    }

    /// Parses the text of `srl.toml`. Errors are plain reasons.
    fn parse(text: &str) -> std::result::Result<Self, String> {
        let config = toml::from_str::<Self>(text).map_err(|error| error.message().to_owned())?;
        let schedule = &config.schedule;
        if !(schedule.desired_retention > 0.0 && schedule.desired_retention < 1.0) {
            return Err("`schedule.desired_retention` must be between 0 and 1".to_owned());
        }
        if schedule.day_starts_at > 23 {
            return Err("`schedule.day_starts_at` must be an hour from 0 to 23".to_owned());
        }
        if config.drills.timeout_seconds == 0 {
            return Err("`drills.timeout_seconds` must be at least 1".to_owned());
        }
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
        let config = Config::parse("[drills]\ntimeout_seconds = 3\n").unwrap();
        assert_eq!(config.drills.timeout_seconds, 3);
        assert_eq!(config.schedule, Schedule::default());
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
            "[drills]\ntimeout = 3\n",
            "[shedule]\n",
            "[schedule]\nday_starts_at = 24\n",
            "[schedule]\ndesired_retention = 1.5\n",
            "[drills]\ntimeout_seconds = 0\n",
            "[schedule]\nnew_per_day = \"ten\"\n",
            "not toml",
        ] {
            assert!(Config::parse(text).is_err(), "accepted:\n{text}");
        }
    }
}
