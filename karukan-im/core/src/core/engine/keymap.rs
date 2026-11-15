//! User key bindings loaded from `keymap.toml` beside `config.toml`.
//!
//! ```toml
//! [bindings]
//! "Ctrl+U" = "hiragana"
//! "Ctrl+I" = "katakana"
//! "Ctrl+P" = "alphanumeric_full"
//! "Ctrl+O" = "alphanumeric_half"
//! ```
//!
//! A binding is checked before the built-in key handling while composing or
//! converting, so it overrides a built-in binding of the same chord. With
//! nothing composed the key is left alone, so the application keeps it.
//!
//! The conversions (ATOK-style Ctrl+U/I/P/O) open the conversion window on
//! the converted text, so Enter commits it and Escape returns to the
//! composition untouched. The alphanumeric ones use the keys actually typed
//! (`kd` stays `kd` even when the romaji table turns it into けん); when the
//! composition was edited in a way that cannot be traced back to keystrokes,
//! they fall back to a romanization of the reading.
//!
//! Kept in its own file (with one-line hooks in `mod.rs` / `init.rs`) so the
//! fork stays easy to rebase onto upstream.

use std::collections::HashMap;
use std::path::Path;

use super::*;

/// What a bound key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyAction {
    /// The reading in hiragana
    Hiragana,
    /// The reading in full-width katakana
    Katakana,
    /// The typed keys, full-width (ＡＢＣ)
    AlphanumericFull,
    /// The typed keys, half-width (ABC)
    AlphanumericHalf,
    /// Open the 🤖 AI candidate view (built-in Ctrl+I)
    AiCandidates,
    /// Switch to katakana input mode (built-in Ctrl+K)
    KatakanaMode,
}

impl KeyAction {
    fn parse(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "hiragana" => Self::Hiragana,
            "katakana" => Self::Katakana,
            "alphanumeric_full" => Self::AlphanumericFull,
            "alphanumeric_half" | "alphanumeric" => Self::AlphanumericHalf,
            "ai_candidates" => Self::AiCandidates,
            "katakana_mode" => Self::KatakanaMode,
            _ => return None,
        })
    }
}

/// A key chord such as `Ctrl+U`. Letters are stored lowercase; Shift is a
/// separate flag, since fcitx5 may report Shift+u as either `U` or `u`+Shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Chord {
    keysym: u32,
    control: bool,
    alt: bool,
    shift: bool,
    super_: bool,
}

impl Chord {
    fn parse(spec: &str) -> Option<Self> {
        let mut chord = Chord {
            keysym: 0,
            control: false,
            alt: false,
            shift: false,
            super_: false,
        };
        let parts: Vec<&str> = spec.split('+').map(str::trim).collect();
        // "Ctrl++" names the plus key
        let (mods, key) = match parts.as_slice() {
            [.., "", ""] => (&parts[..parts.len() - 2], "+"),
            [mods @ .., key] => (mods, *key),
            [] => return None,
        };
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.control = true,
                "alt" => chord.alt = true,
                "shift" => chord.shift = true,
                "super" => chord.super_ = true,
                _ => return None,
            }
        }
        // Key names are case-insensitive: "Ctrl+U" is Ctrl+u; Shift is
        // only ever the explicit modifier.
        chord.keysym = keysym_for_name(key)?;
        if let Some(c) = char::from_u32(chord.keysym) {
            chord.keysym = c.to_ascii_lowercase() as u32;
        }
        Some(chord)
    }

    fn of_event(key: &KeyEvent) -> Self {
        let mut keysym = key.keysym.0;
        let mut shift = key.modifiers.shift_key;
        if let Some(c) = char::from_u32(keysym)
            && c.is_ascii_uppercase()
        {
            keysym = c.to_ascii_lowercase() as u32;
            shift = true;
        }
        Chord {
            keysym,
            control: key.modifiers.control_key,
            alt: key.modifiers.alt_key,
            shift,
            super_: key.modifiers.super_key,
        }
    }
}

fn keysym_for_name(name: &str) -> Option<u32> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_ascii_graphic().then_some(c as u32);
    }
    let lower = name.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u32>().ok())
        && (1..=12).contains(&n)
    {
        return Some(0xffbe + n - 1);
    }
    Some(match lower.as_str() {
        "space" => 0x0020,
        "tab" => 0xff09,
        "return" | "enter" => 0xff0d,
        "escape" | "esc" => 0xff1b,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "home" => 0xff50,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "end" => 0xff57,
        "muhenkan" => 0xff22,
        "henkan" => 0xff23,
        _ => return None,
    })
}

/// The user's bindings.
#[derive(Debug, Clone, Default)]
pub(super) struct Keymap {
    bindings: HashMap<Chord, KeyAction>,
}

#[derive(serde::Deserialize)]
struct KeymapFile {
    #[serde(default)]
    bindings: HashMap<String, String>,
}

impl Keymap {
    /// `keymap.toml` from the config directory; empty when absent or broken.
    pub(super) fn load() -> Self {
        crate::config::Settings::config_dir()
            .map(|dir| Self::load_from(&dir.join("keymap.toml")))
            .unwrap_or_default()
    }

    fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match Self::parse(&text) {
            Ok(map) => {
                tracing::debug!("Loaded {} key bindings from {path:?}", map.bindings.len());
                map
            }
            Err(e) => {
                tracing::error!("{path:?} not loaded: {e}");
                Self::default()
            }
        }
    }

    pub(super) fn parse(text: &str) -> Result<Self, toml::de::Error> {
        let file: KeymapFile = toml::from_str(text)?;
        let mut bindings = HashMap::new();
        for (spec, action) in &file.bindings {
            match (Chord::parse(spec), KeyAction::parse(action)) {
                (Some(chord), Some(action)) => {
                    bindings.insert(chord, action);
                }
                (None, _) => tracing::warn!("keymap.toml: unknown key {spec:?}"),
                (_, None) => tracing::warn!("keymap.toml: unknown action {action:?}"),
            }
        }
        Ok(Self { bindings })
    }

    fn lookup(&self, key: &KeyEvent) -> Option<KeyAction> {
        if self.bindings.is_empty() {
            return None;
        }
        self.bindings.get(&Chord::of_event(key)).copied()
    }
}

/// Per-engine keymap state: the bindings plus the keystrokes typed into the
/// current composition (for the alphanumeric conversions).
#[derive(Debug, Clone, Default)]
pub(super) struct KeymapState {
    pub(super) keymap: Keymap,
    typed: String,
}

impl InputMethodEngine {
    /// Hook run before the built-in key handling. `Some` when a user
    /// binding handled the key.
    pub(super) fn process_keymap(&mut self, key: &KeyEvent) -> Option<EngineResult> {
        if matches!(self.state, InputState::Empty) {
            self.keymap.typed.clear();
        }
        if !key.is_press {
            return None;
        }
        let action = self.keymap.keymap.lookup(key);
        let Some(action) = action.filter(|_| !matches!(self.state, InputState::Empty)) else {
            self.record_keystroke(key);
            return None;
        };
        Some(self.run_key_action(action))
    }

    fn record_keystroke(&mut self, key: &KeyEvent) {
        if let Some(c) = key.to_char() {
            self.keymap.typed.push(c);
        } else if key.keysym == Keysym::BACKSPACE {
            self.keymap.typed.pop();
        }
    }

    fn run_key_action(&mut self, action: KeyAction) -> EngineResult {
        match action {
            KeyAction::AiCandidates => self.jump_to_source(CandidateSource::Model),
            KeyAction::KatakanaMode => self.enter_katakana_mode(),
            _ => self.convert_whole(action),
        }
    }

    /// Open the conversion window on the whole composition rendered as
    /// `action`, followed by the other renderings.
    fn convert_whole(&mut self, action: KeyAction) -> EngineResult {
        let reading = self.input_buf.settled_reading(&self.converters.romaji);
        if reading.is_empty() {
            return EngineResult::consumed();
        }
        self.live.shown = false;

        let hiragana = karukan_engine::katakana_to_hiragana(&reading);
        let katakana = karukan_engine::hiragana_to_katakana(&hiragana);
        let typed = self.typed_keys(&hiragana);
        let full = to_fullwidth(&typed);

        // Kana forms learn like the built-in kana fallbacks; the typed-key
        // forms carry no reading, so committing them is never learned.
        let kana = |text: &str| Candidate {
            reading: Some(reading.clone()),
            source: Some(CandidateSource::Fallback),
            ..Candidate::new(text)
        };
        let order = match action {
            KeyAction::Katakana => [&katakana, &hiragana, &full, &typed],
            KeyAction::AlphanumericFull => [&full, &typed, &hiragana, &katakana],
            KeyAction::AlphanumericHalf => [&typed, &full, &hiragana, &katakana],
            _ => [&hiragana, &katakana, &full, &typed],
        };
        let mut seen = std::collections::HashSet::new();
        let candidates: Vec<Candidate> = order
            .into_iter()
            .filter(|t| seen.insert(t.as_str()))
            .map(|t| {
                if *t == hiragana || *t == katakana {
                    kana(t)
                } else {
                    Candidate::new(t.as_str())
                }
            })
            .collect();
        self.enter_conversion_state(&reading, CandidateList::new(candidates))
    }

    /// The keys typed for this composition, when replaying them reproduces
    /// it; otherwise a romanization of `hiragana`.
    fn typed_keys(&self, hiragana: &str) -> String {
        let typed = &self.keymap.typed;
        if !typed.is_empty() {
            let mut replay = InputBuffer::new();
            for c in typed.chars() {
                replay.push_romaji(c, &self.converters.romaji);
            }
            if replay.display() == self.input_buf.display() {
                return typed.clone();
            }
        }
        romanize(hiragana)
    }
}

/// ASCII → full-width (space → ideographic space); other chars unchanged.
pub(super) fn to_fullwidth(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xfee0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Hepburn-ish romanization of hiragana, for compositions whose keystrokes
/// are unknown. Non-kana characters pass through.
pub(super) fn romanize(hiragana: &str) -> String {
    let chars: Vec<char> = hiragana.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'っ' {
            match kana_at(&chars, i + 1) {
                Some((r, _)) if r.starts_with(|c: char| !"aiueon".contains(c)) => {
                    out.push_str(&r[..1]);
                }
                _ => out.push_str("xtu"),
            }
            i += 1;
            continue;
        }
        if chars[i] == 'ん' {
            let next_vowelish = kana_at(&chars, i + 1)
                .is_some_and(|(r, _)| r.starts_with(|c: char| "aiueoyn".contains(c)));
            out.push_str(if next_vowelish { "nn" } else { "n" });
            i += 1;
            continue;
        }
        match kana_at(&chars, i) {
            Some((r, len)) => {
                out.push_str(&r);
                i += len;
            }
            None => {
                out.push(match chars[i] {
                    'ー' => '-',
                    '、' => ',',
                    '。' => '.',
                    '「' => '[',
                    '」' => ']',
                    '・' => '/',
                    '〜' => '~',
                    c => c,
                });
                i += 1;
            }
        }
    }
    out
}

/// Romaji for the kana at `i` (a yōon pair first), with its length.
fn kana_at(chars: &[char], i: usize) -> Option<(String, usize)> {
    let c = *chars.get(i)?;
    if let Some(&small) = chars.get(i + 1)
        && let Some(r) = youon(c, small)
    {
        return Some((r, 2));
    }
    kana1(c).map(|r| (r.to_string(), 1))
}

fn youon(c: char, small: char) -> Option<String> {
    let base = match c {
        'き' => "ky",
        'ぎ' => "gy",
        'し' => "sh",
        'じ' => "j",
        'ち' => "ch",
        'ぢ' => "dy",
        'に' => "ny",
        'ひ' => "hy",
        'び' => "by",
        'ぴ' => "py",
        'み' => "my",
        'り' => "ry",
        _ => return None,
    };
    let vowel = match small {
        'ゃ' => 'a',
        'ゅ' => 'u',
        'ょ' => 'o',
        _ => return None,
    };
    Some(format!("{base}{vowel}"))
}

fn kana1(c: char) -> Option<&'static str> {
    const ROWS: &[(&str, [&str; 5])] = &[
        ("あいうえお", ["a", "i", "u", "e", "o"]),
        ("かきくけこ", ["ka", "ki", "ku", "ke", "ko"]),
        ("がぎぐげご", ["ga", "gi", "gu", "ge", "go"]),
        ("さしすせそ", ["sa", "shi", "su", "se", "so"]),
        ("ざじずぜぞ", ["za", "ji", "zu", "ze", "zo"]),
        ("たちつてと", ["ta", "chi", "tsu", "te", "to"]),
        ("だぢづでど", ["da", "di", "du", "de", "do"]),
        ("なにぬねの", ["na", "ni", "nu", "ne", "no"]),
        ("はひふへほ", ["ha", "hi", "fu", "he", "ho"]),
        ("ばびぶべぼ", ["ba", "bi", "bu", "be", "bo"]),
        ("ぱぴぷぺぽ", ["pa", "pi", "pu", "pe", "po"]),
        ("まみむめも", ["ma", "mi", "mu", "me", "mo"]),
        ("らりるれろ", ["ra", "ri", "ru", "re", "ro"]),
        ("ぁぃぅぇぉ", ["xa", "xi", "xu", "xe", "xo"]),
    ];
    for (kana, romaji) in ROWS {
        if let Some(i) = kana.chars().position(|k| k == c) {
            return Some(romaji[i]);
        }
    }
    Some(match c {
        'や' => "ya",
        'ゆ' => "yu",
        'よ' => "yo",
        'ゃ' => "xya",
        'ゅ' => "xyu",
        'ょ' => "xyo",
        'わ' => "wa",
        'を' => "wo",
        'ゎ' => "xwa",
        'ゐ' => "wi",
        'ゑ' => "we",
        'ゔ' => "vu",
        _ => return None,
    })
}
