//! User key bindings loaded from `keymap.toml` beside `config.toml`.
//!
//! ```toml
//! [bindings]
//! "Ctrl+U" = "hiragana"
//! "Ctrl+I" = "katakana"
//! "Ctrl+P" = "alphanumeric_full"
//! "Ctrl+O" = "toggle_width"
//! ```
//!
//! A binding is checked before the built-in key handling while composing or
//! converting, so it overrides a built-in binding of the same chord. With
//! nothing composed the key is left alone, so the application keeps it.
//!
//! The conversions (ATOK-style Ctrl+U/I/P, and Ctrl+O flipping the width) open the conversion window on
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
    /// Flip the shown text between full and half width (ATOK Ctrl+O):
    /// katakana ⇄ half-width katakana (hiragana goes to half-width
    /// katakana), full-width ⇄ half-width alphanumerics
    ToggleWidth,
    /// Back to hiragana input from alphabet/katakana mode (the built-in
    /// 変換 key; ATOK binds it to a Shift tap)
    HiraganaMode,
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
            "toggle_width" => Self::ToggleWidth,
            "hiragana_mode" => Self::HiraganaMode,
            "ai_candidates" => Self::AiCandidates,
            "katakana_mode" => Self::KatakanaMode,
            _ => return None,
        })
    }
}

/// A key chord such as `Ctrl+U`. Letters are stored lowercase and Shift is
/// only the modifier flag (see [`Chord::of_event`]).
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
        // fcitx5 hands Ctrl+letter over as the uppercase keysym without
        // Shift (Ctrl+I arrives as `I`), so the case says nothing about
        // Shift: like the built-in bindings, match letters either way and
        // take Shift from the modifier state alone.
        let keysym = match char::from_u32(key.keysym.0) {
            Some(c) => c.to_ascii_lowercase() as u32,
            None => key.keysym.0,
        };
        Chord {
            keysym,
            control: key.modifiers.control_key,
            alt: key.modifiers.alt_key,
            shift: key.modifiers.shift_key,
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
        // Bare "Shift": a tap of either Shift key on its own
        "shift" => SHIFT_TAP,
        _ => return None,
    })
}

/// The keysym a bare `"Shift"` binding (a Shift tap) is stored under.
const SHIFT_TAP: u32 = 0xffe1;

/// The user's bindings.
#[derive(Debug, Clone, Default)]
pub(super) struct Keymap {
    bindings: HashMap<Chord, KeyAction>,
    /// `[options] uppercase_alphabet_lock`, see [`AlphabetLock`]
    uppercase_alphabet_lock: bool,
}

#[derive(serde::Deserialize)]
struct KeymapFile {
    #[serde(default)]
    bindings: HashMap<String, String>,
    #[serde(default)]
    options: KeymapOptions,
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct KeymapOptions {
    #[serde(default)]
    uppercase_alphabet_lock: bool,
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
        Ok(Self {
            bindings,
            uppercase_alphabet_lock: file.options.uppercase_alphabet_lock,
        })
    }

    fn lookup(&self, key: &KeyEvent) -> Option<KeyAction> {
        if self.bindings.is_empty() {
            return None;
        }
        self.bindings.get(&Chord::of_event(key)).copied()
    }
}

/// The whole composition in each rendering the conversions offer.
struct Forms {
    reading: String,
    hiragana: String,
    katakana: String,
    half_katakana: String,
    /// Typed keys, full-width
    full: String,
    /// Typed keys as typed
    typed: String,
}

/// Per-engine keymap state: the bindings plus the keystrokes typed into the
/// current composition (for the alphanumeric conversions).
#[derive(Debug, Clone, Default)]
pub(super) struct KeymapState {
    pub(super) keymap: Keymap,
    typed: String,
    /// A Shift key is down and nothing else was pressed since
    shift_tap_armed: bool,
    /// Progress towards / state of the uppercase alphabet lock
    alphabet_lock: AlphabetLock,
}

/// ATOK's uppercase alphabet lock (`[options] uppercase_alphabet_lock`):
/// a second Shift+letter while Shift is still held from the one that
/// entered alphabet mode (`NHK`) locks uppercase input, which an unshifted
/// letter then leaves — that letter starting the next romaji.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AlphabetLock {
    #[default]
    Off,
    /// Shift+letter just entered alphabet mode and Shift is still held
    Armed,
    /// Uppercase lock: an unshifted letter returns to hiragana
    Locked,
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
        self.track_alphabet_lock(key);
        let action = self.keymap.keymap.lookup(key);
        let Some(action) = action.filter(|_| !matches!(self.state, InputState::Empty)) else {
            self.record_keystroke(key);
            return None;
        };
        Some(self.run_key_action(action))
    }

    /// Hook run before the modifier keys are passed through: a Shift tap
    /// (press and release with nothing in between) runs the `"Shift"`
    /// binding. The release still reaches the application, so its view of
    /// the Shift state stays right.
    pub(super) fn process_modifier_tap(&mut self, key: &KeyEvent) -> Option<EngineResult> {
        if key.keysym != Keysym::SHIFT_L && key.keysym != Keysym::SHIFT_R {
            if key.is_press {
                self.keymap.shift_tap_armed = false;
            }
            return None;
        }
        if key.is_press {
            let m = &key.modifiers;
            self.keymap.shift_tap_armed = !(m.control_key || m.alt_key || m.super_key);
            return None;
        }
        // Letting go of Shift before a second letter: plain alphabet mode
        if self.keymap.alphabet_lock == AlphabetLock::Armed {
            self.keymap.alphabet_lock = AlphabetLock::Off;
        }
        if !std::mem::take(&mut self.keymap.shift_tap_armed) {
            return None;
        }
        let tap = Chord {
            keysym: SHIFT_TAP,
            control: false,
            alt: false,
            shift: false,
            super_: false,
        };
        let action = *self.keymap.keymap.bindings.get(&tap)?;
        let mut result = self.run_key_action(action);
        result.consumed = false;
        Some(result)
    }

    /// Advance [`AlphabetLock`] on a key press, before the built-in
    /// handling sees it. Leaving the lock switches to hiragana here, so the
    /// built-in handling then reads the letter as romaji.
    fn track_alphabet_lock(&mut self, key: &KeyEvent) {
        if !self.keymap.keymap.uppercase_alphabet_lock {
            return;
        }
        let in_alphabet = self.mode.current() == InputMode::Alphabet;
        let letter = key
            .to_char()
            .filter(|c| c.is_ascii_alphabetic() && !key.modifiers.super_key);
        // The same test the built-in handling uses to enter alphabet mode
        let shifted = letter.is_some_and(|c| c.is_ascii_uppercase() || key.modifiers.shift_key);
        self.keymap.alphabet_lock = match (self.keymap.alphabet_lock, in_alphabet) {
            // Shift+letter is about to enter alphabet mode
            (_, false) if shifted => AlphabetLock::Armed,
            (_, false) => AlphabetLock::Off,
            (AlphabetLock::Armed, true) if shifted => AlphabetLock::Locked,
            (AlphabetLock::Locked, true) if letter.is_some() && !shifted => {
                self.handle_mode_toggle_key(&KeyEvent::press(Keysym::HENKAN));
                AlphabetLock::Off
            }
            (AlphabetLock::Locked, true) => AlphabetLock::Locked,
            (_, true) => AlphabetLock::Off,
        };
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
            KeyAction::ToggleWidth => self.toggle_width(),
            KeyAction::HiraganaMode => self
                .handle_mode_toggle_key(&KeyEvent::press(Keysym::HENKAN))
                .unwrap_or_else(EngineResult::consumed),
            _ => self.convert_whole(action),
        }
    }

    /// Open the conversion window on the whole composition rendered as
    /// `action`, followed by the other renderings.
    fn convert_whole(&mut self, action: KeyAction) -> EngineResult {
        let Some(forms) = self.composition_forms() else {
            return EngineResult::consumed();
        };
        let first = match action {
            KeyAction::Katakana => forms.katakana.clone(),
            KeyAction::AlphanumericFull => forms.full.clone(),
            KeyAction::AlphanumericHalf => forms.typed.clone(),
            _ => forms.hiragana.clone(),
        };
        self.open_forms(first, forms)
    }

    /// Flip the width of what is shown: the selected candidate while
    /// converting, the hiragana while composing.
    fn toggle_width(&mut self) -> EngineResult {
        let Some(forms) = self.composition_forms() else {
            return EngineResult::consumed();
        };
        let current = match &self.state {
            InputState::Conversion { candidates, .. } => candidates
                .selected_text()
                .map(str::to_string)
                .unwrap_or_else(|| forms.hiragana.clone()),
            _ => forms.hiragana.clone(),
        };
        let next = if current == forms.hiragana || current == forms.katakana {
            forms.half_katakana.clone()
        } else if current == forms.half_katakana {
            forms.katakana.clone()
        } else if current == forms.full {
            forms.typed.clone()
        } else if current == forms.typed {
            forms.full.clone()
        } else {
            let half = karukan_engine::width::to_half_width(&current);
            if half != current {
                half
            } else {
                karukan_engine::width::to_full_width(&current)
            }
        };
        self.open_forms(next, forms)
    }

    /// Every whole-composition rendering, or `None` with nothing composed.
    fn composition_forms(&self) -> Option<Forms> {
        let reading = self.input_buf.settled_reading(&self.converters.romaji);
        if reading.is_empty() {
            return None;
        }
        let hiragana = karukan_engine::katakana_to_hiragana(&reading);
        let katakana = karukan_engine::hiragana_to_katakana(&hiragana);
        let half_katakana = karukan_engine::kana::katakana_to_half_width(&katakana);
        let typed = self.typed_keys(&hiragana);
        let full = to_fullwidth(&typed);
        Some(Forms {
            reading,
            hiragana,
            katakana,
            half_katakana,
            full,
            typed,
        })
    }

    /// Open the conversion window with `first` selected, followed by the
    /// other renderings.
    fn open_forms(&mut self, first: String, forms: Forms) -> EngineResult {
        self.live.shown = false;
        let kana = [&forms.hiragana, &forms.katakana, &forms.half_katakana];
        let order = [
            &first,
            &forms.hiragana,
            &forms.katakana,
            &forms.half_katakana,
            &forms.full,
            &forms.typed,
        ];
        let mut seen = std::collections::HashSet::new();
        let candidates: Vec<Candidate> = order
            .into_iter()
            .filter(|t| seen.insert(t.as_str()))
            .map(|t| {
                // Kana forms learn like the built-in kana fallbacks; the
                // others carry no reading, so committing them is never
                // learned.
                if kana.contains(&t) {
                    Candidate {
                        reading: Some(forms.reading.clone()),
                        source: Some(CandidateSource::Fallback),
                        ..Candidate::new(t.as_str())
                    }
                } else {
                    Candidate::new(t.as_str())
                }
            })
            .collect();
        self.enter_conversion_state(&forms.reading, CandidateList::new(candidates))
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
