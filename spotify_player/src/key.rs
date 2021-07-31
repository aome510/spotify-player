use crossterm::event::KeyCode;

/// Key denotes a key received from user's input
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    None(KeyCode),
    Ctrl(KeyCode),
    Alt(KeyCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySequence {
    pub keys: Vec<Key>,
}

impl Key {
    pub fn from_str(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() == 1 && chars[0] != ' ' {
            // a single character
            Some(Key::None(KeyCode::Char(chars[0])))
        } else if chars.len() == 3 && chars[1] == '-' && chars[2] != ' ' {
            // M-<c> for alt-<c> and C-<c> for ctrl-<c>
            match chars[0] {
                'C' => Some(Key::Ctrl(KeyCode::Char(chars[2]))),
                'M' => Some(Key::Alt(KeyCode::Char(chars[2]))),
                _ => None,
            }
        } else {
            match s {
                "enter" => Some(Key::None(KeyCode::Enter)),
                "space" => Some(Key::None(KeyCode::Char(' '))),
                "tab" => Some(Key::None(KeyCode::Tab)),
                "backspace" => Some(Key::None(KeyCode::Backspace)),
                "esc" => Some(Key::None(KeyCode::Esc)),

                "left" => Some(Key::None(KeyCode::Left)),
                "right" => Some(Key::None(KeyCode::Right)),
                "up" => Some(Key::None(KeyCode::Up)),
                "down" => Some(Key::None(KeyCode::Down)),

                "insert" => Some(Key::None(KeyCode::Insert)),
                "delete" => Some(Key::None(KeyCode::Delete)),
                "home" => Some(Key::None(KeyCode::Home)),
                "end" => Some(Key::None(KeyCode::End)),
                "page_up" => Some(Key::None(KeyCode::PageUp)),
                "page_down" => Some(Key::None(KeyCode::PageDown)),

                "f1" => Some(Key::None(KeyCode::F(1))),
                "f2" => Some(Key::None(KeyCode::F(2))),
                "f3" => Some(Key::None(KeyCode::F(3))),
                "f4" => Some(Key::None(KeyCode::F(4))),
                "f5" => Some(Key::None(KeyCode::F(5))),
                "f6" => Some(Key::None(KeyCode::F(6))),
                "f7" => Some(Key::None(KeyCode::F(7))),
                "f8" => Some(Key::None(KeyCode::F(8))),
                "f9" => Some(Key::None(KeyCode::F(9))),
                "f10" => Some(Key::None(KeyCode::F(10))),
                "f11" => Some(Key::None(KeyCode::F(11))),
                "f12" => Some(Key::None(KeyCode::F(12))),

                _ => None,
            }
        }
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Key::Ctrl(KeyCode::Char(c)) => write!(f, "C-{}", c),
            Key::Alt(KeyCode::Char(c)) => write!(f, "M-{}", c),
            Key::None(k) => match k {
                KeyCode::Char(c) => {
                    if c == ' ' {
                        write!(f, "space")
                    } else {
                        write!(f, "{}", c)
                    }
                }
                KeyCode::Enter => write!(f, "enter"),
                KeyCode::Tab => write!(f, "tab"),
                KeyCode::Backspace => write!(f, "backspace"),
                KeyCode::Esc => write!(f, "esc"),

                KeyCode::Left => write!(f, "left"),
                KeyCode::Right => write!(f, "right"),
                KeyCode::Up => write!(f, "up"),
                KeyCode::Down => write!(f, "down"),

                KeyCode::Insert => write!(f, "insert"),
                KeyCode::Delete => write!(f, "delete"),
                KeyCode::Home => write!(f, "home"),
                KeyCode::End => write!(f, "end"),
                KeyCode::PageUp => write!(f, "page_up"),
                KeyCode::PageDown => write!(f, "page_down"),

                KeyCode::F(1) => write!(f, "f1"),
                KeyCode::F(2) => write!(f, "f2"),
                KeyCode::F(3) => write!(f, "f3"),
                KeyCode::F(4) => write!(f, "f4"),
                KeyCode::F(5) => write!(f, "f5"),
                KeyCode::F(6) => write!(f, "f6"),
                KeyCode::F(7) => write!(f, "f7"),
                KeyCode::F(8) => write!(f, "f8"),
                KeyCode::F(9) => write!(f, "f9"),
                KeyCode::F(10) => write!(f, "f10"),
                KeyCode::F(11) => write!(f, "f11"),
                KeyCode::F(12) => write!(f, "f12"),

                _ => panic!("unknown key: {:?}", self),
            },
            _ => panic!("unknown key: {:?}", self),
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
                "failed to parse key: unknown key {}",
                s
            ))),
        }
    }
}

impl KeySequence {
    pub fn from_str(s: &str) -> Option<Self> {
        log::info!("from str keysequence: {}", s);
        let keys = s.split(' ').collect::<Vec<_>>();
        if keys.is_empty() {
            return None;
        }
        keys.into_iter()
            .map(|s| Key::from_str(s))
            .collect::<Option<Vec<_>>>()
            .map(|keys| Self { keys })
    }

    /// checks a key sequence is a prefix of `other` key sequence
    pub fn is_prefix(&self, other: &Self) -> bool {
        if self.keys.len() > other.keys.len() {
            return false;
        }
        (0..self.keys.len()).fold(true, |acc, i| acc & (self.keys[i] == other.keys[i]))
    }
}

impl std::fmt::Display for KeySequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.keys
                .iter()
                .map(|k| k.to_string())
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
                "failed to parse key sequence: invalid key sequence {}",
                s
            ))),
        }
    }
}
