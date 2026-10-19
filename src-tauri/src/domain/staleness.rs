use chrono::{DateTime, Local};

pub const STALE_THRESHOLD_DAYS: i64 = 3;

pub fn days_since(status_since: &str) -> i64 {
    let then = DateTime::parse_from_rfc3339(status_since)
        .map(|dt| dt.with_timezone(&Local))
        .unwrap_or_else(|_| Local::now());
    (Local::now().date_naive() - then.date_naive()).num_days()
}

pub fn is_stale(status_since: &str) -> bool {
    days_since(status_since) >= STALE_THRESHOLD_DAYS
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn fresh_status_is_not_stale() {
        let now = Local::now().to_rfc3339();
        assert!(!is_stale(&now));
    }

    #[test]
    fn old_status_is_stale() {
        let old = (Local::now() - Duration::days(5)).to_rfc3339();
        assert!(is_stale(&old));
    }

    #[test]
    fn just_under_threshold_is_not_stale() {
        let two_days_ago = (Local::now() - Duration::days(2)).to_rfc3339();
        assert!(!is_stale(&two_days_ago));
    }
}
