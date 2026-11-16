//! End-of-text rules (`{composition-separator}`) in a user romaji table:
//! applied when the composition settles, never while typing.

use super::*;

fn engine_with_table(tsv: &str) -> InputMethodEngine {
    let mut e = InputMethodEngine::new();
    e.converters.romaji = RomajiConverter::with_chain_table(
        std::sync::Arc::new(karukan_engine::ChainTable::parse_tsv(tsv)),
        e.config.width,
    );
    e
}

const TABLE: &str = "ka\tか\nna\tな\nn{composition-separator}\tん\n";

fn type_str(e: &mut InputMethodEngine, s: &str) {
    for c in s.chars() {
        e.process_key(&press(c));
    }
}

#[test]
fn trailing_n_commits_as_n_sound() {
    let mut e = engine_with_table(TABLE);
    type_str(&mut e, "kan");
    let r = e.process_key(&press_key(Keysym::RETURN));
    assert!(
        r.actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(t) if t == "かん"))
    );
}

#[test]
fn trailing_n_converts_with_n_sound() {
    let mut e = engine_with_table(TABLE);
    type_str(&mut e, "kan");
    e.process_key(&press_key(Keysym::SPACE));
    assert_eq!(e.state().reading(), Some("かん"));
}

#[test]
fn n_before_a_vowel_is_untouched() {
    let mut e = engine_with_table(TABLE);
    type_str(&mut e, "kana");
    let r = e.process_key(&press_key(Keysym::RETURN));
    assert!(
        r.actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(t) if t == "かな"))
    );
}
