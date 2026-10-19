#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Todo,
    Wip,
    InReview,
    Submitting,
    Done,
    Blocked,
    Dropped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Wip => "wip",
            Status::InReview => "in_review",
            Status::Submitting => "submitting",
            Status::Done => "done",
            Status::Blocked => "blocked",
            Status::Dropped => "dropped",
        }
    }

    pub fn from_str(s: &str) -> Option<Status> {
        match s {
            "todo" => Some(Status::Todo),
            "wip" => Some(Status::Wip),
            "in_review" => Some(Status::InReview),
            "submitting" => Some(Status::Submitting),
            "done" => Some(Status::Done),
            "blocked" => Some(Status::Blocked),
            "dropped" => Some(Status::Dropped),
            _ => None,
        }
    }

    /// Fuzzy-matches the first token of a trailing paren against known statuses,
    /// e.g. "in review" / "in-review" / "review" -> InReview. Used by the inline
    /// parser so free typing like "(wip)" or "(in review)" works without exact spelling.
    pub fn fuzzy_match(token: &str) -> Option<Status> {
        let t = token.to_lowercase().replace('-', " ");
        let t = t.trim();
        match t {
            "todo" | "to do" => Some(Status::Todo),
            "wip" | "in progress" => Some(Status::Wip),
            "in review" | "review" | "inreview" => Some(Status::InReview),
            "submitting" | "submit" => Some(Status::Submitting),
            "done" | "complete" | "completed" => Some(Status::Done),
            "blocked" | "block" => Some(Status::Blocked),
            "dropped" | "drop" | "cancelled" | "canceled" => Some(Status::Dropped),
            _ => None,
        }
    }
}

/// Parent status derives from children unless manually overridden.
pub fn derive(children: &[Status]) -> Status {
    if children.is_empty() {
        return Status::Todo;
    }
    let live: Vec<&Status> = children
        .iter()
        .filter(|s| !matches!(s, Status::Dropped))
        .collect();
    if live.is_empty() {
        return Status::Dropped;
    }
    if live.iter().all(|s| matches!(s, Status::Done)) {
        return Status::Done;
    }
    if live.iter().all(|s| matches!(s, Status::Blocked)) {
        return Status::Blocked;
    }
    if live
        .iter()
        .any(|s| matches!(s, Status::Wip | Status::InReview | Status::Submitting))
    {
        return Status::Wip;
    }
    Status::Todo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_children_is_todo() {
        assert_eq!(derive(&[]), Status::Todo);
    }

    #[test]
    fn all_done_is_done() {
        assert_eq!(derive(&[Status::Done, Status::Done]), Status::Done);
    }

    #[test]
    fn dropped_child_excluded_from_derivation() {
        assert_eq!(derive(&[Status::Done, Status::Dropped]), Status::Done);
    }

    #[test]
    fn any_active_child_is_wip() {
        assert_eq!(derive(&[Status::Done, Status::InReview]), Status::Wip);
    }

    #[test]
    fn all_dropped_is_dropped() {
        assert_eq!(derive(&[Status::Dropped, Status::Dropped]), Status::Dropped);
    }
}
