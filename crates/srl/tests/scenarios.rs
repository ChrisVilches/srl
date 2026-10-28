//! Runs the real `srl` binary through what a person usually does with it,
//! over several days: making a store, writing and adding items, studying
//! them as a frontend would, and coming back on later days.
//!
//! Every command runs at a time set with `SRL_NOW`, in UTC, so a scenario
//! can move from one study day to the next.

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use jiff::Span;
use jiff::civil::Date;
use serde_json::Value;
use tempfile::TempDir;

/// Day `day` of a scenario, counting March 1, 2026 as day 1.
fn date(day: i64) -> Date {
    jiff::civil::date(2026, 3, 1) + Span::new().days(day - 1)
}

/// The day of a scenario that `date` is.
#[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
fn day_of(date: &str) -> i64 {
    let date: Date = date.parse().unwrap();
    i64::from(date.since(self::date(1)).unwrap().get_days()) + 1
}

/// The time of `hour`:00 UTC on day `day` of a scenario.
fn at(day: i64, hour: u8) -> String {
    format!("{}T{hour:02}:00:00Z", date(day))
}

/// A person's store, made with `srl init`, with notes written by hand and
/// given IDs with `srl id`.
#[derive(Debug)]
struct Store {
    tmp: TempDir,
}

impl Store {
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn new(notes: &[(&str, &str)]) -> Self {
        let store = Self {
            tmp: TempDir::new().unwrap(),
        };
        store.ok(&at(1, 9), &["init"]);
        for (path, text) in notes {
            store.write(path, text);
        }
        if !notes.is_empty() {
            store.ok(&at(1, 9), &["id"]);
        }
        store
    }

    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn write(&self, path: &str, text: &str) {
        let file = self.tmp.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.tmp.path().join(path)).unwrap()
    }

    /// Runs `srl` on this store at the time `now`, with `input` on
    /// standard input.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn run_with_input(&self, now: &str, args: &[&str], input: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_srl"))
            .args(args)
            .arg("--dir")
            .arg(self.tmp.path())
            .env_remove("SRL_SEED")
            .env_remove("SRL_DIR")
            .env("SRL_NOW", now)
            .env("TZ", "UTC")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn run(&self, now: &str, args: &[&str]) -> Output {
        self.run_with_input(now, args, "")
    }

    /// Runs `srl`, checks that it exits with `code`, and returns its
    /// standard output.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn expect(&self, code: i32, now: &str, args: &[&str], input: &str) -> String {
        let output = self.run_with_input(now, args, input);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(code), "srl {args:?}: {stderr}");
        String::from_utf8(output.stdout).unwrap()
    }

    fn ok(&self, now: &str, args: &[&str]) -> String {
        self.expect(0, now, args, "")
    }

    /// One JSON object per line of what `srl` prints.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn json(&self, now: &str, args: &[&str]) -> Vec<Value> {
        self.ok(now, args)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// The card `srl next` gives, as a frontend gets it, or `None` when
    /// nothing is left to study.
    #[expect(clippy::unwrap_used, reason = "a test helper should fail loudly")]
    fn next(&self, now: &str, selector: &[&str]) -> Option<Value> {
        let output = self.run(now, &[&["next", "--json"][..], selector].concat());
        if output.status.code() == Some(1) {
            return None;
        }
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Some(serde_json::from_slice(&output.stdout).unwrap())
    }

    /// Records a review as a frontend does.
    fn review(&self, now: &str, card: &str, rating: u8) {
        let request = serde_json::json!({"card": card, "rating": rating});
        self.expect(0, now, &["review", "--stdin"], &request.to_string());
    }

    /// Studies until `srl next` has nothing left, giving every card
    /// `rating`, and returns the IDs of the cards studied, in order.
    fn study(&self, now: &str, rating: u8) -> Vec<String> {
        let mut studied = Vec::new();
        while let Some(card) = self.next(now, &[]) {
            let id = text(&card, "id");
            self.review(now, &id, rating);
            studied.push(id);
        }
        studied
    }

    /// The `srl find --json` object of each card, by card ID.
    fn cards(&self, now: &str) -> Vec<Value> {
        self.json(now, &["find", "--json"])
    }

    /// The due day of `card`, as `srl find --json` gives it.
    fn due(&self, now: &str, card: &str) -> String {
        self.cards(now)
            .into_iter()
            .find(|found| text(found, "id") == card)
            .map(|found| text(&found, "due"))
            .unwrap_or_default()
    }

    fn stats(&self, now: &str) -> Value {
        self.json(now, &["stats", "--json"])
            .into_iter()
            .next()
            .unwrap_or_default()
    }
}

const CAPITALS: &str = "# Capitals

```srl
@
Q: Capital of France?
A: Paris

@
Q: Capital of Japan?
A: Tokyo

@
Q: Capital of Chile?
A: Santiago

@
Q: Capital of Kenya?
A: Nairobi
```
";

/// The text of `object`'s field `key`, or "" if it has no such text.
fn text(object: &Value, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `count` questions, each in a block of its own.
fn questions(count: usize) -> String {
    (1..=count)
        .map(|n| format!("```srl\nQ: Question {n}?\nA: Answer {n}\n```\n\n"))
        .collect::<Vec<_>>()
        .concat()
}

#[test]
fn a_first_session_studies_every_new_card_once_in_written_order() {
    let store = Store::new(&[("geography/capitals.md", CAPITALS)]);
    let now = at(1, 19);
    let written = store
        .cards(&now)
        .iter()
        .map(|card| card["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(written.len(), 4);
    let first = store.next(&now, &[]).unwrap();
    assert_eq!(first["prompt"], "Capital of France?");
    assert_eq!(first["answer"], "Paris");
    assert_eq!(store.study(&now, 3), written);
    let stats = store.stats(&now);
    assert_eq!(stats["cards"], 4);
    assert_eq!(stats["states"]["new"], 0);
    assert_eq!(stats["states"]["young"], 4);
    assert_eq!(stats["reviews_today"], 4);
    assert_eq!(store.json(&now, &["log", "--json"]).len(), 4);
}

#[test]
fn cards_known_well_come_back_at_growing_intervals() {
    let store = Store::new(&[("geography/capitals.md", CAPITALS)]);
    let card = store.next(&at(1, 19), &[]).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    store.review(&at(1, 19), &card, 3);
    let only_it = format!("id:{card}");
    let mut day = 1;
    let mut intervals = Vec::new();
    for _ in 0..3 {
        let due = store.due(&at(day, 19), &card);
        let due_day = day_of(&due);
        intervals.push(due_day - day);
        // Not offered before its due day, offered on it.
        let before = store.next(&at(due_day - 1, 19), &[&only_it]);
        assert!(before.is_none(), "{card} was offered before {due}");
        assert_eq!(store.next(&at(due_day, 19), &[]).unwrap()["id"], *card);
        store.review(&at(due_day, 19), &card, 3);
        day = due_day;
    }
    assert!(
        intervals.windows(2).all(|pair| pair[0] < pair[1]),
        "{intervals:?}"
    );
}

#[test]
fn a_forgotten_card_is_due_the_next_day_before_any_new_card() {
    let store = Store::new(&[("geography/capitals.md", CAPITALS)]);
    let now = at(1, 19);
    let france = store.next(&now, &[]).unwrap();
    let france = france["id"].as_str().unwrap();
    store.review(&now, france, 1);
    // A card rated Again is not shown again the same day.
    let japan = store.next(&now, &[]).unwrap();
    assert_ne!(japan["id"], france);
    let tomorrow = at(2, 19);
    assert_eq!(store.due(&tomorrow, france), "2026-03-02");
    let next = store.next(&tomorrow, &[]).unwrap();
    assert_eq!(next["id"], france);
    assert_eq!(next["state"], "due");
}

#[test]
fn new_cards_are_introduced_ten_a_day() {
    let store = Store::new(&[("deck.md", &questions(15))]);
    assert_eq!(store.study(&at(1, 19), 3).len(), 10);
    assert_eq!(store.stats(&at(1, 19))["states"]["new"], 5);
    // The next day brings the other five; the first ten aren't due yet.
    assert_eq!(store.study(&at(2, 19), 3).len(), 5);
    assert_eq!(store.stats(&at(2, 19))["states"]["new"], 0);
}

#[test]
fn a_session_past_midnight_belongs_to_the_day_it_began() {
    let store = Store::new(&[("deck.md", &questions(15))]);
    let evening = store.study(&at(1, 23), 3);
    let after_midnight = at(2, 1);
    // Before day_starts_at (4:00), it is still the 1st: the limit is spent
    // and the cards just studied are not offered again.
    assert!(store.next(&after_midnight, &[]).is_none());
    assert_eq!(store.stats(&after_midnight)["reviews_today"], 10);
    // From 4:00 it is the 2nd.
    let morning = store.study(&at(2, 8), 3);
    assert_eq!(morning.len(), 5);
    assert!(morning.iter().all(|card| !evening.contains(card)));
}

#[test]
fn cloze_cards_of_one_item_are_introduced_one_a_day() {
    let store = Store::new(&[(
        "japanese/kanji.md",
        "```srl\nQ: {{c1::東京}}は{{c2::日本}}の{{c3::首都}}です。\nA: Tokyo is the capital of Japan.\n```\n",
    )]);
    let item = store.cards(&at(1, 19))[0]["item"]
        .as_str()
        .unwrap()
        .to_owned();
    for (day, cloze) in [(1, 1), (2, 2), (3, 3)] {
        let now = at(day, 19);
        assert_eq!(store.study(&now, 3), [format!("{item}-{cloze}")]);
    }
}

#[test]
fn drafts_added_by_a_tool_are_studied_once_accepted() {
    let store = Store::new(&[("geography/capitals.md", CAPITALS)]);
    let now = at(1, 19);
    store.study(&now, 3);
    let input = concat!(
        "{\"see\": \"geography/capitals.md\", \"draft\": true, \"Q\": \"Capital of Peru?\", \"A\": \"Lima\"}\n",
        "{\"see\": \"geography/capitals.md\", \"draft\": true, \"Q\": \"Capital of Egypt?\", \"A\": \"Cairo\"}\n",
    );
    let added = store.expect(0, &now, &["add", "generated/capitals.md", "--stdin"], input);
    assert_eq!(added.lines().count(), 2);
    // Drafts wait for a person to look at them.
    assert!(store.next(&now, &[]).is_none());
    let drafts = store.ok(&now, &["find", "status:draft"]);
    assert_eq!(drafts.lines().count(), 2);
    assert!(drafts.contains("Capital of Peru?"), "{drafts}");
    store.ok(&now, &["accept", "in:generated"]);
    let studied = store.study(&now, 3);
    assert_eq!(studied.len(), 2);
    assert_eq!(
        store.ok(&now, &["show", &studied[0], "--context", "none"]),
        format!(
            "generated/capitals.md:1\n```srl id:{} see:geography/capitals.md\nQ: Capital of Peru?\nA: Lima\n```\n",
            studied[0]
        )
    );
}

#[test]
fn fixing_a_typo_keeps_the_cards_schedule() {
    let store = Store::new(&[(
        "geography/capitals.md",
        "```srl\nQ: Capital of Austrailia?\nA: Canberra\n```\n",
    )]);
    let now = at(1, 19);
    let card = store.study(&now, 3).remove(0);
    let due = store.due(&now, &card);
    let shown = store.ok(&now, &["show", &card, "--context", "none"]);
    let item = shown
        .split_once('\n')
        .unwrap()
        .1
        .replace("Austrailia", "Australia");
    store.expect(0, &at(2, 19), &["edit", &card, "--stdin"], &item);
    assert!(
        store
            .read("geography/capitals.md")
            .contains("Capital of Australia?")
    );
    assert_eq!(store.due(&at(2, 19), &card), due);
    assert_eq!(store.cards(&at(2, 19))[0]["state"], "young");
}
