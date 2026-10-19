use chrono::{Datelike, NaiveDate};

use crate::domain::status::Status;
use crate::parse::parse_inline_status;

pub struct ParsedTask {
    pub title: String,
    pub status: Status,
    pub note: Option<String>,
    pub children: Vec<ParsedTask>,
}

pub struct ParsedDay {
    pub date: NaiveDate,
    pub tasks: Vec<ParsedTask>,
}

/// Matches a day header like "09/22 TODAY" or "09/21" - digits, '/', digits,
/// then end-of-line or whitespace. Deliberately not a bullet line (those are
/// filtered out by the caller before this is tried).
fn parse_day_header(line: &str) -> Option<(u32, u32)> {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();

    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i > 2 || bytes.get(i) != Some(&b'/') {
        return None;
    }
    let month: u32 = trimmed[..i].parse().ok()?;
    let rest = &trimmed[i + 1..];

    let mut j = 0;
    let rest_bytes = rest.as_bytes();
    while j < rest_bytes.len() && rest_bytes[j].is_ascii_digit() {
        j += 1;
    }
    if j == 0 || j > 2 {
        return None;
    }
    let day: u32 = rest[..j].parse().ok()?;

    let after = &rest[j..];
    if !after.is_empty() && !after.starts_with(char::is_whitespace) {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((month, day))
}

fn leading_indent_width(line: &str) -> usize {
    let mut width = 0;
    for c in line.chars() {
        match c {
            ' ' => width += 1,
            '\t' => width += 4,
            _ => break,
        }
    }
    width
}

/// Attaches a finished deeper level to the last task at the level above, or -
/// if there's no task to hang it off (malformed leading indentation) -
/// promotes those items to that level instead of silently losing them.
fn attach(parent: &mut Vec<ParsedTask>, children: Vec<ParsedTask>) {
    match parent.last_mut() {
        Some(last) => last.children = children,
        None => parent.extend(children),
    }
}

fn parse_bullet_tree(lines: &[&str]) -> Vec<ParsedTask> {
    let mut items: Vec<(usize, String)> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('-') {
            continue;
        }
        let depth = leading_indent_width(line) / 4;
        let content = trimmed[1..].trim().to_string();
        if content.is_empty() {
            continue;
        }
        items.push((depth, content));
    }

    let mut stack: Vec<(usize, Vec<ParsedTask>)> = vec![(0, Vec::new())];

    for (depth, content) in items {
        while stack.len() > 1 && stack.last().unwrap().0 > depth {
            let (_, finished) = stack.pop().unwrap();
            attach(&mut stack.last_mut().unwrap().1, finished);
        }
        if stack.last().unwrap().0 < depth {
            stack.push((depth, Vec::new()));
        }
        let parsed = parse_inline_status(&content);
        stack.last_mut().unwrap().1.push(ParsedTask {
            title: parsed.title,
            status: parsed.status.unwrap_or(Status::Todo),
            note: parsed.note,
            children: Vec::new(),
        });
    }

    while stack.len() > 1 {
        let (_, finished) = stack.pop().unwrap();
        attach(&mut stack.last_mut().unwrap().1, finished);
    }
    stack.pop().unwrap().1
}

/// Parses a dumped reverse-chronological daily note (newest block first) into
/// per-day task trees. `anchor` is the latest date a block is allowed to
/// resolve to - typically the day before whatever is live in the app, since
/// today's real tasks shouldn't be touched by a historical import. Headers
/// have no year, so each one resolves to the latest year that doesn't put it
/// after the previous (i.e. more recent) resolved date - this is what lets
/// a note spanning a New Year's Eve resolve correctly without being told to.
pub fn parse_note(text: &str, anchor: NaiveDate) -> Vec<ParsedDay> {
    struct Block<'a> {
        month: u32,
        day: u32,
        lines: Vec<&'a str>,
    }

    let mut blocks: Vec<Block> = Vec::new();
    for line in text.lines() {
        let trimmed_start = line.trim_start();
        if !trimmed_start.starts_with('-') {
            if let Some((month, day)) = parse_day_header(line) {
                blocks.push(Block { month, day, lines: Vec::new() });
                continue;
            }
        }
        if let Some(block) = blocks.last_mut() {
            block.lines.push(line);
        }
    }

    let mut running = anchor;
    let mut result = Vec::with_capacity(blocks.len());
    for block in blocks {
        let this_year = NaiveDate::from_ymd_opt(running.year(), block.month, block.day);
        let prev_year = NaiveDate::from_ymd_opt(running.year() - 1, block.month, block.day);
        let resolved = match (this_year, prev_year) {
            (Some(d), _) if d <= running => d,
            (_, Some(d)) => d,
            (Some(d), None) => d,
            (None, None) => continue, // invalid calendar date (e.g. Feb 30) - skip this block
        };
        running = resolved;
        result.push(ParsedDay { date: resolved, tasks: parse_bullet_tree(&block.lines) });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_NOTE: &str = r#"
09/22 TODAY
- Hide Phone
- Walk station (unless run)
- Calendar (done)
- Giraffe flag
    - Reaserch it (done)
    - Create giraffe flag (in review)
    - Pass giraffe flags into all the components (maybe should be done a bit later when we bump the component)
    - Update cilium cni (in review)
- Address test fix CLs (submitting)
- Present my AI automation suite
    - Make AI skills universal (wip)
    - Test the skills
        - Have Manju test them for me

09/21
- Hide Phone
- Calendar (done)
- Address test fix CLs (in review)
    - NP (in review)
    - SS (in review))
- Add reviewers column and next steps column to CL tracker (wip)

- Track Testing scenarios somewhere (done, in ai first doc for now0
- Present my AI automation suite
    - Make AI skills universal (wip)
"#;

    #[test]
    fn splits_into_two_day_blocks_newest_first() {
        let anchor = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        let days = parse_note(SAMPLE_NOTE, anchor);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, NaiveDate::from_ymd_opt(2026, 9, 22).unwrap());
        assert_eq!(days[1].date, NaiveDate::from_ymd_opt(2026, 9, 21).unwrap());
    }

    #[test]
    fn year_rolls_back_at_a_new_year_boundary() {
        // A note written in early January referencing late December should
        // resolve the December block to the *previous* year.
        let note = "01/02\n- New year task\n\n12/30\n- Old year task\n";
        let anchor = NaiveDate::from_ymd_opt(2027, 1, 2).unwrap();
        let days = parse_note(note, anchor);
        assert_eq!(days[0].date, NaiveDate::from_ymd_opt(2027, 1, 2).unwrap());
        assert_eq!(days[1].date, NaiveDate::from_ymd_opt(2026, 12, 30).unwrap());
    }

    #[test]
    fn nests_bullets_by_indentation_depth() {
        let anchor = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        let days = parse_note(SAMPLE_NOTE, anchor);
        let today = &days[0];
        let giraffe = today.tasks.iter().find(|t| t.title == "Giraffe flag").unwrap();
        assert_eq!(giraffe.children.len(), 4);
        assert_eq!(giraffe.children[0].title, "Reaserch it"); // typo preserved verbatim
        assert_eq!(giraffe.children[0].status.as_str(), "done");

        let suite = today.tasks.iter().find(|t| t.title == "Present my AI automation suite").unwrap();
        let test_skills = suite.children.iter().find(|t| t.title == "Test the skills").unwrap();
        assert_eq!(test_skills.children[0].title, "Have Manju test them for me");
    }

    #[test]
    fn real_note_quirks_all_parse_without_losing_data() {
        let anchor = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        let days = parse_note(SAMPLE_NOTE, anchor);
        let yesterday = &days[1];

        let address = yesterday.tasks.iter().find(|t| t.title == "Address test fix CLs").unwrap();
        let ss = address.children.iter().find(|t| t.title == "SS").unwrap();
        assert_eq!(ss.status.as_str(), "in_review"); // doubled close-paren still resolves

        let tracking = yesterday.tasks.iter().find(|t| t.title == "Track Testing scenarios somewhere").unwrap();
        assert_eq!(tracking.status.as_str(), "done"); // missing close-paren still resolves
        assert_eq!(tracking.note.as_deref(), Some("in ai first doc for now0"));

        // The blank line in the middle of 09/21's block must not truncate parsing.
        assert!(yesterday.tasks.iter().any(|t| t.title == "Present my AI automation suite"));
    }

    #[test]
    fn non_header_preamble_before_first_date_is_ignored() {
        let note = "some stray text\nnot a date\n\n09/22\n- Real task\n";
        let anchor = NaiveDate::from_ymd_opt(2026, 9, 22).unwrap();
        let days = parse_note(note, anchor);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].tasks.len(), 1);
        assert_eq!(days[0].tasks[0].title, "Real task");
    }
}
