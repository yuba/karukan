//! User key bindings (`keymap.toml`): ATOK-style whole-composition
//! conversions and overriding built-in chords.

use super::*;
use crate::core::engine::keymap::{Keymap, romanize, to_fullwidth};

const ATOK: &str = r#"
[bindings]
"Ctrl+U" = "hiragana"
"Ctrl+I" = "katakana"
"Ctrl+P" = "alphanumeric_full"
"Ctrl+O" = "alphanumeric_half"
"#;

fn engine() -> InputMethodEngine {
    let mut engine = InputMethodEngine::new();
    engine.keymap.keymap = Keymap::parse(ATOK).unwrap();
    engine
}

fn ctrl(ch: char) -> KeyEvent {
    KeyEvent::new(
        Keysym(ch as u32),
        KeyModifiers::new().with_control(true),
        true,
    )
}

fn type_str(engine: &mut InputMethodEngine, s: &str) {
    for c in s.chars() {
        engine.process_key(&press(c));
    }
}

fn committed(result: &EngineResult) -> Option<String> {
    result.actions.iter().find_map(|a| match a {
        EngineAction::Commit(t) => Some(t.clone()),
        _ => None,
    })
}

#[test]
fn ctrl_i_converts_to_katakana_and_enter_commits() {
    let mut e = engine();
    type_str(&mut e, "kanji");
    let r = e.process_key(&ctrl('i'));
    assert!(r.consumed);
    assert!(matches!(e.state(), InputState::Conversion { .. }));
    assert_eq!(e.preedit().unwrap().text(), "カンジ");
    let r = e.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed(&r).as_deref(), Some("カンジ"));
}

#[test]
fn ctrl_u_after_ctrl_i_goes_back_to_hiragana() {
    let mut e = engine();
    type_str(&mut e, "kanji");
    e.process_key(&ctrl('i'));
    e.process_key(&ctrl('u'));
    assert_eq!(e.preedit().unwrap().text(), "かんじ");
}

#[test]
fn ctrl_p_and_ctrl_o_use_the_typed_keys() {
    let mut e = engine();
    type_str(&mut e, "kanji");
    e.process_key(&ctrl('p'));
    assert_eq!(e.preedit().unwrap().text(), "ｋａｎｊｉ");
    e.process_key(&ctrl('o'));
    assert_eq!(e.preedit().unwrap().text(), "kanji");
}

#[test]
fn alphanumeric_keeps_the_keys_of_a_custom_romaji_table() {
    let table = karukan_engine::ChainTable::parse_tsv("kd\tけん\n");
    let mut e = engine();
    e.converters.romaji =
        RomajiConverter::with_chain_table(std::sync::Arc::new(table), e.config.width);
    type_str(&mut e, "kd");
    assert_eq!(e.preedit().unwrap().text(), "けん");
    e.process_key(&ctrl('o'));
    assert_eq!(e.preedit().unwrap().text(), "kd");
}

#[test]
fn alphanumeric_falls_back_to_romanizing_after_an_edit() {
    let mut e = engine();
    type_str(&mut e, "kyo");
    e.process_key(&press_key(Keysym::BACKSPACE));
    e.process_key(&ctrl('o'));
    assert_eq!(e.preedit().unwrap().text(), "ki");
}

#[test]
fn escape_returns_to_the_composition() {
    let mut e = engine();
    type_str(&mut e, "kanji");
    e.process_key(&ctrl('i'));
    e.process_key(&press_key(Keysym::ESCAPE));
    assert!(matches!(e.state(), InputState::Composing { .. }));
    e.process_key(&ctrl('o'));
    assert_eq!(e.preedit().unwrap().text(), "kanji");
}

#[test]
fn bound_keys_pass_through_with_nothing_composed() {
    let mut e = engine();
    assert!(!e.process_key(&ctrl('u')).consumed);
    assert!(!e.process_key(&ctrl('p')).consumed);
}

#[test]
fn unbound_engine_keeps_builtin_ctrl_i() {
    // No keymap: Ctrl+I still opens the AI view (Conversion state).
    let mut e = InputMethodEngine::new();
    type_str(&mut e, "kanji");
    assert!(e.process_key(&ctrl('i')).consumed);
    assert!(matches!(e.state(), InputState::Conversion { .. }));
    assert_ne!(e.preedit().unwrap().text(), "カンジ");
}

#[test]
fn uppercase_keysym_from_fcitx5_still_matches() {
    // fcitx5 sends Ctrl+I as keysym `I` with only Control set.
    let mut e = engine();
    type_str(&mut e, "kanji");
    assert!(e.process_key(&ctrl('I')).consumed);
    assert_eq!(e.preedit().unwrap().text(), "カンジ");
}

#[test]
fn shift_comes_from_the_modifier_flag() {
    let map = Keymap::parse("[bindings]\n\"Ctrl+Shift+U\" = \"katakana\"\n").unwrap();
    let mut e = InputMethodEngine::new();
    e.keymap.keymap = map;
    type_str(&mut e, "kana");
    // Plain Ctrl+U (in either case) is not bound
    e.process_key(&ctrl('U'));
    assert_ne!(e.preedit().unwrap().text(), "カナ");
    let ctrl_shift_u = KeyEvent::new(
        Keysym('U' as u32),
        KeyModifiers::new().with_control(true).with_shift(true),
        true,
    );
    e.process_key(&ctrl_shift_u);
    assert_eq!(e.preedit().unwrap().text(), "カナ");
}

#[test]
fn unknown_entries_are_skipped() {
    let map =
        Keymap::parse("[bindings]\n\"Ctrl+Q\" = \"nope\"\n\"Hyper+X\" = \"katakana\"\n").unwrap();
    let mut e = InputMethodEngine::new();
    e.keymap.keymap = map;
    type_str(&mut e, "a");
    assert!(!e.process_key(&ctrl('q')).consumed);
}

#[test]
fn romanize_and_fullwidth() {
    assert_eq!(romanize("きょうはっぱ"), "kyouhappa");
    assert_eq!(romanize("しんや"), "shinnya");
    assert_eq!(romanize("かんじ、ー"), "kanji,-");
    assert_eq!(to_fullwidth("ab 1!"), "ａｂ\u{3000}１！");
}
