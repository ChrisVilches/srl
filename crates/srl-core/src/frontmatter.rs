//! Changing one key in the front-matter of an existing atom file, and
//! nothing else (see `docs/rust.md`, "Front-matter editing: the YAML caveat").
//!
//! This is the only code that writes into an existing atom file. It edits the
//! text rather than the data, so that comments, quoting and layout survive,
//! and it checks its own work by parsing the result again.

use serde_json::{Map, Value};

use crate::{atom, yaml};

/// Sets the top-level `key` of the front-matter in `text` to the string
/// `value`, and returns the new text.
///
/// If the key is there, only its value changes, and a trailing `# comment`
/// stays. If it is not, a line `key: value` is added right after the opening
/// `---`. Every other byte stays the same. Front-matter this can't edit
/// safely is refused: no front-matter, a flow mapping (`{id: x}`), a key that
/// appears twice, or a value that is not a one-line scalar.
///
/// Errors are plain reasons, to be put into a sentence by the caller.
pub(crate) fn set(text: &str, key: &str, value: &str) -> Result<String, String> {
    let (head, front, tail, newline) = parts(text)?;
    let old = yaml::parse_mapping(front)?;
    if front
        .lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .is_some_and(|line| line.starts_with('{'))
    {
        return Err("its front-matter is a `{…}` mapping; write one key per line".to_owned());
    }
    let rendered = yaml::render_scalar(value)?;
    let lines = front.split_inclusive('\n').collect::<Vec<_>>();
    let found = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| split_key(line).is_some_and(|(name, _)| name == key))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let new_front = match found.as_slice() {
        [] => format!("{}: {rendered}{newline}{front}", yaml::render_scalar(key)?),
        [index] => replace_line(&lines, *index, key, &rendered)?,
        [_, _, ..] => return Err(format!("`{key}` appears more than once")),
    };
    verify(&old, &new_front, key, value)?;
    Ok(format!("{head}{new_front}{tail}"))
}

/// Splits an atom file into the text before the front-matter (a BOM and the
/// opening `---` line), the front-matter, the rest, and the line ending of
/// the opening `---` line.
fn parts(text: &str) -> Result<(&str, &str, &str, &'static str), String> {
    let (front, _) = atom::split_front_matter(text)
        .ok_or_else(|| "it does not start with front-matter between `---` lines".to_owned())?;
    let bom = if text.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let crlf = text
        .get(bom..)
        .is_some_and(|rest| rest.starts_with("---\r\n"));
    let (start, newline) = if crlf {
        (bom + "---\r\n".len(), "\r\n")
    } else {
        (bom + "---\n".len(), "\n")
    };
    let (head, rest) = text
        .split_at_checked(start)
        .expect("the opening `---` line should end at a character boundary");
    let (front, tail) = rest
        .split_at_checked(front.len())
        .expect("the front-matter should follow the opening `---` line");
    Ok((head, front, tail, newline))
}

/// The front-matter with the value on line `index` replaced by `rendered`.
fn replace_line(lines: &[&str], index: usize, key: &str, rendered: &str) -> Result<String, String> {
    let line = lines
        .get(index)
        .expect("the index of a found line should be in range");
    let continued = lines
        .get(index + 1)
        .is_some_and(|next| next.starts_with([' ', '\t', '-']) && !next.trim().is_empty());
    if continued {
        return Err(format!("the value of `{key}` spans several lines"));
    }
    let (_, rest) = split_key(line).expect("a found line should have a key");
    let colon_end = line.len() - rest.len();
    let (start, end) =
        value_span(rest).map_err(|reason| format!("the value of `{key}` {reason}"))?;
    let (before, value_and_after) = line
        .split_at_checked(colon_end + start)
        .expect("the value should start at a character boundary");
    let (_, after) = value_and_after
        .split_at_checked(end - start)
        .expect("the value should end at a character boundary");
    let replaced = if start == end {
        let comment = after.trim_start_matches([' ', '\t']);
        let gap = if comment.starts_with('#') { " " } else { "" };
        let prefix = line
            .split_at_checked(colon_end)
            .expect("the colon should end at a character boundary")
            .0;
        format!("{prefix} {rendered}{gap}{comment}")
    } else {
        format!("{before}{rendered}{after}")
    };
    Ok(lines
        .iter()
        .enumerate()
        .map(|(number, original)| {
            if number == index {
                replaced.as_str()
            } else {
                original
            }
        })
        .collect())
}

/// The key of a top-level `key: value` line, and the rest of the line after
/// the colon. Indented lines, comments and list items have no key.
fn split_key(line: &str) -> Option<(String, &str)> {
    match line.chars().next()? {
        quote @ ('"' | '\'') => {
            let (key, rest) = line.get(1..)?.split_once(quote)?;
            Some((key.to_owned(), rest.strip_prefix(':')?))
        }
        ' ' | '\t' | '#' | '-' | '{' | '[' | '\r' | '\n' => None,
        _ => {
            let (colon, _) = line.char_indices().find(|&(index, character)| {
                character == ':'
                    && line.get(index + 1..).is_some_and(|after| {
                        after.is_empty() || after.starts_with([' ', '\t', '\r', '\n'])
                    })
            })?;
            let (key, rest) = line.split_at_checked(colon)?;
            Some((key.trim_end().to_owned(), rest.get(1..)?))
        }
    }
}

/// Where the value sits in `rest`, the part of a line after the colon: its
/// start and end, equal if the value is empty. Errors complete the sentence
/// "the value of `key` …".
fn value_span(rest: &str) -> Result<(usize, usize), String> {
    let content = rest.trim_end_matches(['\r', '\n']);
    let value = content.trim_start_matches([' ', '\t']);
    let start = content.len() - value.len();
    let length = match value.chars().next() {
        None | Some('#') => 0,
        Some('|' | '>') => return Err("spans several lines".to_owned()),
        Some('[' | '{' | '&' | '*' | '!') => return Err("is not a plain value".to_owned()),
        Some('"') => double_quoted_len(value).ok_or_else(|| "spans several lines".to_owned())?,
        Some('\'') => single_quoted_len(value).ok_or_else(|| "spans several lines".to_owned())?,
        Some(_) => plain_len(value),
    };
    Ok((start, start + length))
}

/// The length of a `"…"` value at the start of `value`, if it closes.
fn double_quoted_len(value: &str) -> Option<usize> {
    let mut escaped = false;
    value.char_indices().skip(1).find_map(|(index, character)| {
        let closes = character == '"' && !escaped;
        escaped = character == '\\' && !escaped;
        closes.then_some(index + 1)
    })
}

/// The length of a `'…'` value at the start of `value`, if it closes. Inside,
/// `''` stands for one `'`.
fn single_quoted_len(value: &str) -> Option<usize> {
    let mut chars = value.char_indices().skip(1).peekable();
    while let Some((index, character)) = chars.next() {
        if character == '\'' && chars.next_if(|&(_, next)| next == '\'').is_none() {
            return Some(index + 1);
        }
    }
    None
}

/// The length of a plain value at the start of `value`: up to a ` #`
/// comment or the end, without trailing spaces.
fn plain_len(value: &str) -> usize {
    let end = [" #", "\t#"]
        .iter()
        .filter_map(|marker| value.find(marker))
        .min()
        .unwrap_or(value.len());
    value
        .split_at_checked(end)
        .expect("a comment marker should start at a character boundary")
        .0
        .trim_end()
        .len()
}

/// Checks that `new_front` holds the same keys and values as `old`, except
/// for `key`, which must now be `value`.
fn verify(old: &Map<String, Value>, new_front: &str, key: &str, value: &str) -> Result<(), String> {
    let new = yaml::parse_mapping(new_front)
        .map_err(|reason| format!("the edited front-matter would not parse ({reason})"))?;
    let expected = if old.contains_key(key) {
        old.iter()
            .map(|(name, old_value)| {
                if name == key {
                    (name.clone(), Value::from(value))
                } else {
                    (name.clone(), old_value.clone())
                }
            })
            .collect::<Map<_, _>>()
    } else {
        std::iter::once((key.to_owned(), Value::from(value)))
            .chain(old.clone())
            .collect()
    };
    if new == expected {
        Ok(())
    } else {
        Err(format!(
            "setting `{key}` would change other values too; edit the file by hand"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_status(text: &str) -> String {
        set(text, "status", "active").unwrap()
    }

    #[test]
    fn an_existing_value_is_replaced() {
        assert_eq!(
            set_status("---\nid: a7f3k2\nstatus: draft\ntitle: x\n---\nbody\n"),
            "---\nid: a7f3k2\nstatus: active\ntitle: x\n---\nbody\n"
        );
    }

    #[test]
    fn a_missing_key_goes_after_the_opening_fence() {
        assert_eq!(
            set("---\ntitle: x\n---\nbody\n", "id", "a7f3k2").unwrap(),
            "---\nid: a7f3k2\ntitle: x\n---\nbody\n"
        );
    }

    #[test]
    fn empty_front_matter_gets_the_key() {
        assert_eq!(
            set("---\n---\n", "id", "a7f3k2").unwrap(),
            "---\nid: a7f3k2\n---\n"
        );
    }

    #[test]
    fn crlf_line_endings_are_kept() {
        assert_eq!(
            set_status("---\r\ntitle: x\r\nstatus: draft\r\n---\r\nbody\r\n"),
            "---\r\ntitle: x\r\nstatus: active\r\n---\r\nbody\r\n"
        );
        assert_eq!(
            set("---\r\ntitle: x\r\n---\r\n", "id", "a7f3k2").unwrap(),
            "---\r\nid: a7f3k2\r\ntitle: x\r\n---\r\n"
        );
    }

    #[test]
    fn a_bom_is_kept() {
        assert_eq!(
            set("\u{feff}---\ntitle: x\n---\n", "id", "a7f3k2").unwrap(),
            "\u{feff}---\nid: a7f3k2\ntitle: x\n---\n"
        );
    }

    #[test]
    fn no_trailing_newline_is_kept() {
        assert_eq!(
            set_status("---\nstatus: draft\ntitle: x\n---"),
            "---\nstatus: active\ntitle: x\n---"
        );
        assert_eq!(
            set_status("---\ntitle: x\n---\nbody"),
            "---\nstatus: active\ntitle: x\n---\nbody"
        );
    }

    #[test]
    fn trailing_comments_are_kept() {
        assert_eq!(
            set_status("---\ntitle: x\nstatus: draft   # from the agent\n---\n"),
            "---\ntitle: x\nstatus: active   # from the agent\n---\n"
        );
        assert_eq!(
            set_status("---\ntitle: x\nstatus: \"dr#ft\" # c\n---\n"),
            "---\ntitle: x\nstatus: active # c\n---\n"
        );
        assert_eq!(
            set_status("---\ntitle: x\nstatus: 'it''s' # c\n---\n"),
            "---\ntitle: x\nstatus: active # c\n---\n"
        );
    }

    #[test]
    fn an_empty_value_is_filled() {
        assert_eq!(
            set_status("---\ntitle: x\nstatus:\n---\n"),
            "---\ntitle: x\nstatus: active\n---\n"
        );
        assert_eq!(
            set_status("---\ntitle: x\nstatus:  # none yet\n---\n"),
            "---\ntitle: x\nstatus: active # none yet\n---\n"
        );
    }

    #[test]
    fn quoted_keys_are_found() {
        assert_eq!(
            set_status("---\ntitle: x\n\"status\": draft\n---\n"),
            "---\ntitle: x\n\"status\": active\n---\n"
        );
        assert_eq!(
            set_status("---\ntitle: x\n'status': draft\n---\n"),
            "---\ntitle: x\n'status': active\n---\n"
        );
    }

    #[test]
    fn non_ascii_text_around_the_line_is_untouched() {
        let text = "---\ntitle: 〜をものともせず # 文法\nstatus: draft\ncloze:\n  - 嵐を{{ものともせず}}、漁師たちは海へ出た。\n---\n漁師たちは海へ出た。\n";
        assert_eq!(set_status(text), text.replace("draft", "active"));
    }

    #[test]
    fn values_that_look_like_other_types_stay_text() {
        assert_eq!(
            set_status("---\ntitle: no\nstatus: draft\n---\n"),
            "---\ntitle: no\nstatus: active\n---\n"
        );
        let text = set("---\ntitle: x\n---\n", "id", "123456").unwrap();
        let fields = yaml::parse_mapping(atom::split_front_matter(&text).unwrap().0).unwrap();
        assert_eq!(fields.get("id"), Some(&Value::from("123456")));
    }

    #[test]
    fn nested_keys_of_the_same_name_are_ignored() {
        assert_eq!(
            set_status("---\ntitle: x\ncards:\n  - q: a\n    status: b\nstatus: draft\n---\n"),
            "---\ntitle: x\ncards:\n  - q: a\n    status: b\nstatus: active\n---\n"
        );
    }

    #[test]
    fn unsafe_front_matter_is_refused() {
        for text in [
            "no front-matter\n",
            "---\n{title: x, status: draft}\n---\n",
            "---\ntitle: x\nstatus: |\n  draft\n---\n",
            "---\ntitle: x\nstatus: >\n  draft\n---\n",
            "---\ntitle: x\nstatus:\n  draft\n---\n",
            "---\ntitle: x\nstatus: dr\n  aft\n---\n",
            "---\ntitle: x\nstatus: \"draft\n  \"\n---\n",
            "---\ntitle: x\nstatus: [draft]\n---\n",
            "---\ntitle: x\nstatus: draft\nstatus: draft\n---\n",
            "---\ntitle: [x\n---\n",
        ] {
            assert!(set(text, "status", "active").is_err(), "accepted:\n{text}");
        }
    }
}
