const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;

fn digit_value(b: u8) -> usize {
    ALPHABET.iter().position(|&c| c == b).expect("invalid sort_key digit")
}

fn single_char(v: usize) -> String {
    (ALPHABET[v] as char).to_string()
}

/// Returns a sort key that sorts strictly between `lo` and `hi`. Either bound
/// may be absent (inserting at the very start/end of a list).
pub fn key_between(lo: Option<&str>, hi: Option<&str>) -> String {
    match (lo, hi) {
        (None, None) => single_char(BASE / 2),
        (None, Some(h)) => before(h),
        (Some(l), None) => after(l),
        (Some(l), Some(h)) => mid(l, h),
    }
}

fn after(s: &str) -> String {
    if s.is_empty() {
        return single_char(BASE / 2);
    }
    let d = digit_value(s.as_bytes()[0]);
    if d < BASE - 1 {
        single_char(d + 1)
    } else {
        format!("{}{}", single_char(d), after(&s[1..]))
    }
}

fn before(s: &str) -> String {
    if s.is_empty() {
        return single_char(BASE / 2);
    }
    let d = digit_value(s.as_bytes()[0]);
    if d > 0 {
        // Adjacent decrement rather than halving: preserves the most headroom
        // at this digit position for future `before` calls on the result.
        single_char(d - 1)
    } else if s.len() > 1 {
        // "0" is already < s by the prefix rule (s has more after it).
        single_char(0)
    } else {
        // s is exactly the minimum digit with nothing after it - the true
        // floor of this scheme. Only the empty string sorts below it.
        String::new()
    }
}

fn mid(lo: &str, hi: &str) -> String {
    let common = lo
        .bytes()
        .zip(hi.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    if common > 0 {
        return format!("{}{}", &lo[..common], mid(&lo[common..], &hi[common..]));
    }
    // lo being empty after stripping the common prefix means the original lo was
    // a strict prefix of hi (e.g. "V" vs "V0V") — any non-empty extension of the
    // prefix already sorts after lo, so this reduces to finding something before hi.
    if lo.is_empty() {
        return before(hi);
    }
    let lo_d = digit_value(lo.as_bytes()[0]);
    let hi_d = hi.as_bytes().first().map(|&b| digit_value(b)).unwrap_or(BASE);
    if hi_d - lo_d > 1 {
        single_char(lo_d + (hi_d - lo_d) / 2)
    } else {
        format!("{}{}", single_char(lo_d), after(&lo[1..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_key_is_stable_midpoint() {
        assert_eq!(key_between(None, None), "V");
    }

    #[test]
    fn key_between_sorts_correctly() {
        let a = key_between(None, None);
        let b = key_between(Some(&a), None);
        assert!(a < b);
        let c = key_between(Some(&a), Some(&b));
        assert!(a < c && c < b);
    }

    #[test]
    fn repeated_inserts_stay_ordered() {
        let mut keys = vec![key_between(None, None)];
        for _ in 0..50 {
            let last = keys.last().unwrap().clone();
            keys.push(key_between(Some(&last), None));
        }
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn repeated_midpoint_inserts_stay_ordered() {
        let lo = key_between(None, None);
        let mut hi = key_between(Some(&lo), None);
        for _ in 0..20 {
            let m = key_between(Some(&lo), Some(&hi));
            assert!(lo < m && m < hi, "{} < {} < {}", lo, m, hi);
            hi = m;
        }
    }
}
