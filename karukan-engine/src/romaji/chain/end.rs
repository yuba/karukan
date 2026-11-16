//! End-of-text rules: azooKey / hazkey's `{composition-separator}` marker.
//!
//! A key ending in the marker (`n{composition-separator}` → ん) applies only
//! where the composition ends — when it settles for conversion or commit —
//! so a trailing `n` can become ん without ever stealing the `n` of `na`.
//! The longest such key matching the end of the live tail wins.

use super::ChainTable;

/// azooKey's spelling of the end-of-text marker.
pub const END_MARKER: &str = "{composition-separator}";

impl ChainTable {
    /// Take `key` as an end-of-text rule when it ends in [`END_MARKER`].
    /// Returns false (leaving it to the ordinary rules) otherwise.
    pub(super) fn insert_end_rule(&mut self, key: &str, value: &str) -> bool {
        let Some(body) = key.strip_suffix(END_MARKER) else {
            return false;
        };
        if !body.is_empty() {
            self.end_rules.insert(body.to_string(), value.to_string());
        }
        true
    }

    /// Settle a live tail at the end of the composition: the longest
    /// end-of-text key it ends with is rewritten, the rest kept as typed.
    pub fn flush_end(&self, tail: &str) -> String {
        for (at, _) in tail.char_indices() {
            if let Some(value) = self.end_rules.get(&tail[at..]) {
                return format!("{}{value}", &tail[..at]);
            }
        }
        tail.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ChainTable {
        ChainTable::parse_tsv(
            "na\tな\nn{composition-separator}\tん\nっn{composition-separator}\tっん\nky{composition-separator}\tき\n{composition-separator}\tx\n",
        )
    }

    #[test]
    fn end_rules_do_not_fire_mid_input() {
        let t = table();
        let mut buf = String::new();
        for c in "na".chars() {
            t.feed(&mut buf, c);
        }
        assert_eq!(buf, "な");
        assert!(!t.rules.keys().any(|k| k.contains('{')));
    }

    #[test]
    fn flush_end_rewrites_the_tail() {
        let t = table();
        assert_eq!(t.flush_end("n"), "ん");
        assert_eq!(t.flush_end("っn"), "っん");
        assert_eq!(t.flush_end("ky"), "き");
        assert_eq!(t.flush_end("xky"), "xき");
        assert_eq!(t.flush_end("k"), "k");
        assert_eq!(t.flush_end(""), "");
    }
}
