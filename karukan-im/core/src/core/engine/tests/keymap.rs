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
"Ctrl+Y" = "toggle_width"
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

#[test]
fn toggle_width_flips_kana_and_alphanumerics() {
    let mut e = engine();
    type_str(&mut e, "kanji");
    // Composing hiragana → half-width katakana, as ATOK does
    e.process_key(&ctrl('y'));
    assert_eq!(e.preedit().unwrap().text(), "ｶﾝｼﾞ");
    e.process_key(&ctrl('y'));
    assert_eq!(e.preedit().unwrap().text(), "カンジ");
    e.process_key(&ctrl('p'));
    assert_eq!(e.preedit().unwrap().text(), "ｋａｎｊｉ");
    e.process_key(&ctrl('y'));
    assert_eq!(e.preedit().unwrap().text(), "kanji");
    e.process_key(&ctrl('y'));
    assert_eq!(e.preedit().unwrap().text(), "ｋａｎｊｉ");
}

fn engine_with_shift_tap() -> InputMethodEngine {
    let mut e = InputMethodEngine::new();
    e.keymap.keymap = Keymap::parse("[bindings]\n\"Shift\" = \"hiragana_mode\"\n").unwrap();
    e
}

fn shift_release() -> KeyEvent {
    KeyEvent::new(Keysym::SHIFT_L, KeyModifiers::new().with_shift(true), false)
}

#[test]
fn shift_tap_leaves_alphabet_mode() {
    let mut e = engine_with_shift_tap();
    // Shift+L enters alphabet mode; letting go of Shift is not a tap
    e.process_key(&press_key(Keysym::SHIFT_L));
    e.process_key(&press_shift('L'));
    e.process_key(&shift_release());
    assert!(e.mode.current() == InputMode::Alphabet);
    type_str(&mut e, "inux");

    // A bare tap goes back to kana; the release still reaches the app
    e.process_key(&press_key(Keysym::SHIFT_R));
    let r = e.process_key(&shift_release());
    assert!(!r.consumed);
    assert!(e.mode.current() == InputMode::Hiragana);
    type_str(&mut e, "ga");
    assert_eq!(e.preedit().unwrap().text(), "Linuxが");
}

#[test]
fn shift_tap_without_binding_does_nothing() {
    let mut e = engine();
    e.process_key(&press_shift('L'));
    e.process_key(&press_key(Keysym::SHIFT_L));
    e.process_key(&shift_release());
    assert!(e.mode.current() == InputMode::Alphabet);
}

#[test]
fn shift_in_a_chord_is_not_a_tap() {
    let mut e = engine_with_shift_tap();
    e.process_key(&press_shift('L'));
    // Ctrl held while Shift goes down
    e.process_key(&KeyEvent::new(
        Keysym::SHIFT_L,
        KeyModifiers::new().with_control(true),
        true,
    ));
    e.process_key(&shift_release());
    assert!(e.mode.current() == InputMode::Alphabet);
}

fn engine_with_alphabet_lock(enabled: bool) -> InputMethodEngine {
    let mut e = InputMethodEngine::new();
    e.keymap.keymap = Keymap::parse(&format!(
        "[bindings]\n\"Shift\" = \"hiragana_mode\"\n[options]\nuppercase_alphabet_lock = {enabled}\n"
    ))
    .unwrap();
    e
}

/// Hold Shift, type `letters` with it, let go.
fn shifted(e: &mut InputMethodEngine, letters: &str) {
    e.process_key(&press_key(Keysym::SHIFT_L));
    for c in letters.chars() {
        e.process_key(&press_shift(c));
    }
    e.process_key(&shift_release());
}

#[test]
fn second_shifted_letter_locks_uppercase_and_a_lowercase_letter_leaves() {
    let mut e = engine_with_alphabet_lock(true);
    shifted(&mut e, "NHK");
    assert!(e.mode.current() == InputMode::Alphabet);
    // The unshifted k leaves the lock and starts the romaji
    type_str(&mut e, "ka");
    assert!(e.mode.current() == InputMode::Hiragana);
    assert_eq!(e.preedit().unwrap().text(), "NHKか");
}

#[test]
fn single_shifted_letter_is_plain_alphabet_mode() {
    let mut e = engine_with_alphabet_lock(true);
    shifted(&mut e, "L");
    type_str(&mut e, "inux");
    assert!(e.mode.current() == InputMode::Alphabet);
    assert_eq!(e.preedit().unwrap().text(), "Linux");
}

#[test]
fn releasing_shift_before_the_second_letter_does_not_lock() {
    let mut e = engine_with_alphabet_lock(true);
    shifted(&mut e, "N");
    shifted(&mut e, "H");
    type_str(&mut e, "a");
    assert!(e.mode.current() == InputMode::Alphabet);
    assert_eq!(e.preedit().unwrap().text(), "NHa");
}

#[test]
fn uppercase_lock_is_off_by_default() {
    let mut e = engine_with_alphabet_lock(false);
    shifted(&mut e, "NHK");
    type_str(&mut e, "ka");
    assert!(e.mode.current() == InputMode::Alphabet);
    assert_eq!(e.preedit().unwrap().text(), "NHKka");
}

#[test]
fn shift_tap_still_leaves_the_uppercase_lock() {
    let mut e = engine_with_alphabet_lock(true);
    shifted(&mut e, "NHK");
    e.process_key(&press_key(Keysym::SHIFT_L));
    e.process_key(&shift_release());
    type_str(&mut e, "ga");
    assert_eq!(e.preedit().unwrap().text(), "NHKが");
}

#[test]
fn ctrl_shift_v_reaches_the_app_when_idle_if_asked() {
    let mut e = InputMethodEngine::new();
    e.keymap.keymap =
        Keymap::parse("[options]\nverbose_toggle_only_while_composing = true\n").unwrap();
    let ctrl_shift_v = KeyEvent::new(
        Keysym::KEY_V_UPPER,
        KeyModifiers::new().with_control(true).with_shift(true),
        true,
    );
    // Nothing composed: a terminal's paste, untouched
    assert!(!e.process_key(&ctrl_shift_v).consumed);
    assert!(!e.config.verbose);
    // Composing: still the verbose toggle
    type_str(&mut e, "ka");
    assert!(e.process_key(&ctrl_shift_v).consumed);
    assert!(e.config.verbose);
}

#[test]
fn ctrl_shift_v_toggles_verbose_when_idle_by_default() {
    let mut e = engine();
    let ctrl_shift_v = KeyEvent::new(
        Keysym::KEY_V_UPPER,
        KeyModifiers::new().with_control(true).with_shift(true),
        true,
    );
    assert!(e.process_key(&ctrl_shift_v).consumed);
    assert!(e.config.verbose);
}
