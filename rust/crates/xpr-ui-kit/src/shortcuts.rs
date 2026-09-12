//! Qt key-sequence strings ("Ctrl+Shift+A", "F1", "`", "Shift+1") mapped to
//! egui shortcuts, plus the dispatcher that consumes them each frame with
//! the text-field focus guard the Qt window applies.

use std::collections::HashMap;

use egui::{Key, KeyboardShortcut, Modifiers};
use xpr_core::Config;

/// Parse one Qt `QKeySequence::toString()` value. `Ctrl` maps to egui's
/// `COMMAND` (Ctrl on Windows/Linux, Cmd on macOS) exactly as Qt does.
pub fn parse_key_sequence(seq: &str) -> Option<KeyboardShortcut> {
    let seq = seq.trim();
    if seq.is_empty() {
        return None;
    }
    // Only the first sequence of a multi-sequence string is used.
    let seq = seq.split(", ").next().unwrap_or(seq);
    let mut modifiers = Modifiers::NONE;
    let mut key: Option<Key> = None;
    // A trailing "+" is the plus key itself ("Ctrl++").
    let parts: Vec<&str> = if seq.ends_with('+') && seq.len() > 1 {
        let mut p: Vec<&str> = seq[..seq.len() - 1].trim_end_matches('+').split('+').collect();
        p.push("+");
        p
    } else {
        seq.split('+').collect()
    };
    for part in parts {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers = modifiers | Modifiers::COMMAND,
            "shift" => modifiers = modifiers | Modifiers::SHIFT,
            "alt" => modifiers = modifiers | Modifiers::ALT,
            "meta" | "win" | "cmd" => modifiers = modifiers | Modifiers::COMMAND,
            _ => {
                key = key_from_qt_name(p);
                if key.is_none() {
                    return None;
                }
            }
        }
    }
    key.map(|k| KeyboardShortcut::new(modifiers, k))
}

fn key_from_qt_name(name: &str) -> Option<Key> {
    let n = name.trim();
    let mapped = match n.to_ascii_lowercase().as_str() {
        "del" => "Delete",
        "ins" => "Insert",
        "esc" => "Escape",
        "return" | "enter" => "Enter",
        "backtab" => "Tab",
        "pgup" => "PageUp",
        "pgdown" => "PageDown",
        "space" => "Space",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "`" | "quoteleft" => "Backtick",
        "'" => "Quote",
        _ => n,
    };
    Key::from_name(mapped)
}

/// Format a shortcut back into Qt's display form (for menus and tooltips).
pub fn format_key_sequence(sc: &KeyboardShortcut) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if sc.modifiers.command || sc.modifiers.ctrl {
        parts.push("Ctrl");
    }
    if sc.modifiers.shift {
        parts.push("Shift");
    }
    if sc.modifiers.alt {
        parts.push("Alt");
    }
    let key = qt_key_name(sc.logical_key);
    parts.push(&key);
    parts.join("+")
}

fn qt_key_name(k: Key) -> String {
    match k {
        Key::Backtick => "`".to_string(),
        Key::Enter => "Return".to_string(),
        Key::Escape => "Esc".to_string(),
        Key::Delete => "Del".to_string(),
        Key::ArrowLeft => "Left".to_string(),
        Key::ArrowRight => "Right".to_string(),
        Key::ArrowUp => "Up".to_string(),
        Key::ArrowDown => "Down".to_string(),
        Key::Minus => "-".to_string(),
        Key::Plus => "+".to_string(),
        Key::Equals => "=".to_string(),
        Key::Comma => ",".to_string(),
        Key::Period => ".".to_string(),
        Key::Slash => "/".to_string(),
        Key::Backslash => "\\".to_string(),
        Key::Semicolon => ";".to_string(),
        Key::Quote => "'".to_string(),
        Key::OpenBracket => "[".to_string(),
        Key::CloseBracket => "]".to_string(),
        Key::Num0 => "0".to_string(),
        Key::Num1 => "1".to_string(),
        Key::Num2 => "2".to_string(),
        Key::Num3 => "3".to_string(),
        Key::Num4 => "4".to_string(),
        Key::Num5 => "5".to_string(),
        Key::Num6 => "6".to_string(),
        Key::Num7 => "7".to_string(),
        Key::Num8 => "8".to_string(),
        Key::Num9 => "9".to_string(),
        other => other.name().to_string(),
    }
}

/// The configured map of action id -> shortcut, rebuilt on every rebinding.
#[derive(Clone, Debug, Default)]
pub struct ShortcutMap {
    shortcuts: HashMap<String, KeyboardShortcut>,
    labels: HashMap<String, String>,
}

impl ShortcutMap {
    pub fn from_config(cfg: &Config) -> ShortcutMap {
        let mut shortcuts = HashMap::new();
        let mut labels = HashMap::new();
        for (action_id, seq) in cfg.get_all_shortcuts() {
            labels.insert(action_id.clone(), seq.clone());
            if let Some(sc) = parse_key_sequence(&seq) {
                shortcuts.insert(action_id, sc);
            }
        }
        ShortcutMap { shortcuts, labels }
    }

    pub fn get(&self, action_id: &str) -> Option<&KeyboardShortcut> {
        self.shortcuts.get(action_id)
    }

    /// The display text of the binding (Qt string as configured).
    pub fn label(&self, action_id: &str) -> String {
        self.labels.get(action_id).cloned().unwrap_or_default()
    }

    /// Consume the chord for `action_id` if it was pressed this frame.
    pub fn consume(&self, ctx: &egui::Context, action_id: &str) -> bool {
        match self.shortcuts.get(action_id) {
            Some(sc) => ctx.input_mut(|i| i.consume_shortcut(sc)),
            None => false,
        }
    }

    /// Whether `action_id` is bound to a chord that needs no modifier or
    /// only Shift (a "plain key" that must not fire while typing).
    pub fn is_plain_key(&self, action_id: &str) -> bool {
        match self.shortcuts.get(action_id) {
            Some(sc) => !sc.modifiers.command && !sc.modifiers.ctrl && !sc.modifiers.alt,
            None => false,
        }
    }
}

/// Which key chords a focused text field swallows. Qt lets a `QLineEdit`
/// consume plain keys, so window-context shortcuts bound to bare keys never
/// fire while typing; `Ctrl`/`Alt` chords and function keys do (unless the
/// handler itself checks the text-field flag, which the app mirrors).
pub fn text_field_swallows(sc: &KeyboardShortcut) -> bool {
    if sc.modifiers.command || sc.modifiers.ctrl || sc.modifiers.alt {
        // the editing chords a line edit accepts itself (copy/paste/undo/select-all/word navigation)
        if (sc.modifiers.command || sc.modifiers.ctrl) && !sc.modifiers.alt {
            return matches!(
                sc.logical_key,
                Key::A | Key::C | Key::V | Key::X | Key::Z | Key::Y | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::Backspace | Key::Delete
            );
        }
        return false;
    }
    !matches!(
        sc.logical_key,
        Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 | Key::F11 | Key::F12 | Key::Escape
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qt_sequences() {
        let s = parse_key_sequence("Ctrl+Shift+A").unwrap();
        assert!(s.modifiers.command && s.modifiers.shift);
        assert_eq!(s.logical_key, Key::A);
        assert_eq!(parse_key_sequence("F1").unwrap().logical_key, Key::F1);
        assert_eq!(parse_key_sequence("`").unwrap().logical_key, Key::Backtick);
        assert_eq!(parse_key_sequence("Ctrl+`").unwrap().logical_key, Key::Backtick);
        let one = parse_key_sequence("Shift+1").unwrap();
        assert!(one.modifiers.shift);
        assert_eq!(one.logical_key, Key::Num1);
        assert_eq!(parse_key_sequence("Delete").unwrap().logical_key, Key::Delete);
        assert!(parse_key_sequence("").is_none());
        assert_eq!(format_key_sequence(&parse_key_sequence("Ctrl+Shift+Alt+F").unwrap()), "Ctrl+Shift+Alt+F");
        assert_eq!(format_key_sequence(&parse_key_sequence("Ctrl+`").unwrap()), "Ctrl+`");
    }
}
