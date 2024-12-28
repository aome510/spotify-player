use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
/// Key represents a key received from user's input
pub enum Key {
    Unknown,
    None(KeyCode),
    Ctrl(KeyCode),
    Alt(KeyCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// `KeySequence` represents a combination of pressed keys
pub struct KeySequence {
    pub keys: Vec<Key>,
}

impl Key {
    fn parse_key_code(s: &str) -> Option<KeyCode> {
        Some(match s {
            "enter" => KeyCode::Enter,
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "esc" => KeyCode::Esc,

            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,

            "insert" => KeyCode::Insert,
            "delete" => KeyCode::Delete,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "page_up" => KeyCode::PageUp,
            "page_down" => KeyCode::PageDown,

            "f1" => KeyCode::F(1),
            "f2" => KeyCode::F(2),
            "f3" => KeyCode::F(3),
            "f4" => KeyCode::F(4),
            "f5" => KeyCode::F(5),
            "f6" => KeyCode::F(6),
            "f7" => KeyCode::F(7),
            "f8" => KeyCode::F(8),
            "f9" => KeyCode::F(9),
            "f10" => KeyCode::F(10),
            "f11" => KeyCode::F(11),
            "f12" => KeyCode::F(12),

            _ => {
                let chars = s.chars().collect::<Vec<_>>();
                if chars.len() == 1 && chars[0] != ' ' {
                    KeyCode::Char(chars[0])
                } else {
                    return None;
                }
            }
        })
    }

    /// creates a `Key` from its string representation
    pub fn from_str(s: &str) -> Option<Self> {
        let chars = s.chars().collect::<Vec<_>>();
        if chars.len() > 2 && chars[1] == '-' && chars[2] != ' ' {
            // M-<c> for alt-<c> and C-<c> for ctrl-<c>
            let mut chars = chars.into_iter();
            let c = chars.next().unwrap();
            chars.next();
            let key = Self::parse_key_code(&chars.collect::<String>());
            match c {
                'C' => key.map(Key::Ctrl),
                'M' => key.map(Key::Alt),
                _ => None,
            }
        } else {
            Self::parse_key_code(s).map(Key::None)
        }
    }
}

fn key_code_to_string(k: KeyCode) -> String {
    match k {
        KeyCode::Char(c) => {
            if c == ' ' {
                "space".to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "backtab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Esc => "esc".to_string(),

        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),

        KeyCode::Insert => "insert".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "page_up".to_string(),
        KeyCode::PageDown => "page_down".to_string(),

        KeyCode::F(1) => "f1".to_string(),
        KeyCode::F(2) => "f2".to_string(),
        KeyCode::F(3) => "f3".to_string(),
        KeyCode::F(4) => "f4".to_string(),
        KeyCode::F(5) => "f5".to_string(),
        KeyCode::F(6) => "f6".to_string(),
        KeyCode::F(7) => "f7".to_string(),
        KeyCode::F(8) => "f8".to_string(),
        KeyCode::F(9) => "f9".to_string(),
        KeyCode::F(10) => "f10".to_string(),
        KeyCode::F(11) => "f11".to_string(),
        KeyCode::F(12) => "f12".to_string(),

        _ => panic!("unknown key: {k:?}"),
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Key::Ctrl(k) => write!(f, "C-{}", key_code_to_string(k)),
            Key::Alt(k) => write!(f, "M-{}", key_code_to_string(k)),
            Key::None(k) => write!(f, "{}", key_code_to_string(k)),
            Key::Unknown => write!(f, "unknown key"),
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match Self::from_str(&s) {
            Some(key) => Ok(key),
            None => Err(serde::de::Error::custom(format!(
                "failed to parse key: unknown key {s}"
            ))),
        }
    }
}

impl KeySequence {
    /// creates a `KeySequence` from its string representation
    pub fn from_str(s: &str) -> Option<Self> {
        let keys = s.split(' ').collect::<Vec<_>>();
        if keys.is_empty() {
            return None;
        }
        keys.into_iter()
            .map(Key::from_str)
            .collect::<Option<Vec<_>>>()
            .map(|keys| Self { keys })
    }

    /// checks if a key sequence is a prefix of `other` key sequence
    pub fn is_prefix(&self, other: &Self) -> bool {
        if self.keys.len() > other.keys.len() {
            return false;
        }
        (0..self.keys.len()).fold(true, |acc, i| acc & (self.keys[i] == other.keys[i]))
    }
}

impl From<KeyEvent> for Key {
    fn from(event: KeyEvent) -> Self {
        let mut modifiers = event.modifiers;
        // if the key combination contains `SHIFT`, remove it
        // because the `event.code` already represents the with-SHIFT key code
        if modifiers & KeyModifiers::SHIFT == KeyModifiers::SHIFT {
            modifiers ^= KeyModifiers::SHIFT;
        }

        match modifiers {
            KeyModifiers::NONE => Key::None(event.code),
            KeyModifiers::ALT => Key::Alt(event.code),
            KeyModifiers::CONTROL => Key::Ctrl(event.code),
            _ => Key::Unknown,
        }
    }
}

impl std::fmt::Display for KeySequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.keys
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

impl<'de> serde::de::Deserialize<'de> for KeySequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match KeySequence::from_str(&s) {
            Some(key_sequence) => Ok(key_sequence),
            None => Err(serde::de::Error::custom(format!(
                "failed to parse key sequence: invalid key sequence {s}"
            ))),
        }
    }
}
