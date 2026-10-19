use crate::domain::status::Status;

pub struct ParsedTitle {
    pub title: String,
    pub status: Option<Status>,
    pub note: Option<String>,
}

/// Extracts a trailing "(...)" from a task title. If the first word inside the
/// parens fuzzy-matches a known status, it's consumed as a status change and
/// any remaining text in the parens becomes a note. Otherwise the whole paren
/// is left as a note and the title's status is untouched. This is what lets
/// typing "Update cilium cni (in review)" behave like the old Apple Note.
///
/// Tolerates the real note's typos: a trailing paren that's never closed
/// ("...for now0", a typo'd ")") and a doubled close paren ("(in review))").
/// Neither needs the parens perfectly balanced - we just read from the last
/// "(" to the end of the line.
pub fn parse_inline_status(input: &str) -> ParsedTitle {
    let trimmed = input.trim_end();
    let Some(open) = trimmed.rfind('(') else {
        return ParsedTitle {
            title: trimmed.to_string(),
            status: None,
            note: None,
        };
    };
    let title = trimmed[..open].trim_end().to_string();
    let after_open = &trimmed[open + 1..];
    let inner = after_open.strip_suffix(')').unwrap_or(after_open);
    let inner = inner.trim_end_matches(')');

    // Try progressively shorter leading word-groups so "in review" and
    // "in review, still blocked on X" both resolve to InReview.
    let words: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect();
    for take in (1..=words.len().min(2)).rev() {
        let candidate = words[..take].join(" ");
        if let Some(status) = Status::fuzzy_match(&candidate) {
            let remainder = inner[candidate.len()..].trim_start_matches(',').trim();
            let note = if remainder.is_empty() {
                None
            } else {
                Some(remainder.to_string())
            };
            return ParsedTitle { title, status: Some(status), note };
        }
    }

    ParsedTitle {
        title,
        status: None,
        note: Some(inner.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_title_untouched() {
        let p = parse_inline_status("Hide Phone");
        assert_eq!(p.title, "Hide Phone");
        assert!(p.status.is_none());
    }

    #[test]
    fn done_status_extracted() {
        let p = parse_inline_status("Calendar (done)");
        assert_eq!(p.title, "Calendar");
        assert_eq!(p.status.unwrap().as_str(), "done");
    }

    #[test]
    fn two_word_status_extracted() {
        let p = parse_inline_status("Update cilium cni (in review)");
        assert_eq!(p.title, "Update cilium cni");
        assert_eq!(p.status.unwrap().as_str(), "in_review");
    }

    #[test]
    fn non_status_paren_becomes_note() {
        let p = parse_inline_status(
            "Pass giraffe flags into all the components (maybe should be done a bit later when we bump the component)",
        );
        assert_eq!(p.title, "Pass giraffe flags into all the components");
        assert!(p.status.is_none());
        assert!(p.note.is_some());
    }

    #[test]
    fn status_with_trailing_note() {
        let p = parse_inline_status("Track Testing scenarios somewhere (done, in ai first doc for now)");
        assert_eq!(p.status.unwrap().as_str(), "done");
        assert_eq!(p.note.unwrap(), "in ai first doc for now");
    }

    #[test]
    fn doubled_close_paren_still_resolves_status() {
        // Real line from the user's note: "SS (in review))"
        let p = parse_inline_status("SS (in review))");
        assert_eq!(p.title, "SS");
        assert_eq!(p.status.unwrap().as_str(), "in_review");
        assert!(p.note.is_none());
    }

    #[test]
    fn missing_close_paren_still_resolves_status_and_note() {
        // Real line from the user's note (typo'd "0" instead of ")"):
        // "Track Testing scenarios somewhere (done, in ai first doc for now0"
        let p = parse_inline_status("Track Testing scenarios somewhere (done, in ai first doc for now0");
        assert_eq!(p.title, "Track Testing scenarios somewhere");
        assert_eq!(p.status.unwrap().as_str(), "done");
        assert_eq!(p.note.unwrap(), "in ai first doc for now0");
    }
}
