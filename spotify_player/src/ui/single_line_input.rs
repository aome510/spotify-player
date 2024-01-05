use crossterm::event::KeyCode;
use tui::widgets::Widget;

use super::*;
use crate::key::{Key, KeySequence};

#[derive(Debug, Clone)]
pub struct LineInput {
    // This is less space-efficient than String, but it's easier to work with text manipulation at the
    // cursor. Otherwise, you have to shuffle back and forth between String and String::chars().
    line: Vec<char>,
    cursor: u16,
}

pub enum InputEffect {
    TextChanged,
    CursorMoved,
    // Sometimes a given input has no effect, but it is should still be considered as 'consumed' by
    // the input element. For instance, pressing backspace when there is no text.
    Ack,
}

impl Default for LineInput {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl LineInput {
    pub fn new(str: Vec<char>) -> Self {
        Self {
            line: str,
            cursor: 0,
        }
    }

    pub fn input(&mut self, key_sequence: &KeySequence) -> Option<InputEffect> {
        return match key_sequence.keys[0] {
            Key::None(c) => match c {
                KeyCode::Char(c) => {
                    if self.cursor as usize == self.line.len() {
                        self.line.push(c);
                    } else {
                        self.line.insert(self.cursor.into(), c);
                    }
                    self.cursor += 1;
                    Some(InputEffect::TextChanged)
                }
                KeyCode::Backspace => {
                    if self.line.len() == 0 || self.cursor == 0 {
                        Some(InputEffect::Ack)
                    } else {
                        // Perform the decrement first.
                        self.cursor -= 1;
                        self.line.remove(self.cursor.into());
                        Some(InputEffect::TextChanged)
                    }
                }
                KeyCode::Left => {
                    if self.cursor == 0 {
                        Some(InputEffect::Ack)
                    } else {
                        self.cursor -= 1;
                        Some(InputEffect::CursorMoved)
                    }
                }
                KeyCode::Right => {
                    if self.cursor as usize == self.line.len() {
                        Some(InputEffect::Ack)
                    } else {
                        self.cursor += 1;
                        Some(InputEffect::CursorMoved)
                    }
                }
                _ => None,
            },
            _ => None,
        };
    }

    pub fn widget(&self, is_active: bool) -> impl Widget {
        if !is_active {
            let converted_str: String = self.line.iter().collect();
            return Paragraph::new(converted_str);
        }

        let mut before_cursor = String::new();
        // Default cursor to be an empty space. This ensures it's displayed even if the cursor is
        // at the end of the string.
        let mut cursor = " ".to_string();
        let mut after_cursor = String::new();
        for (idx, chr) in self.line.iter().enumerate() {
            let chr = *chr;
            match idx.cmp(&(self.cursor as usize)) {
                std::cmp::Ordering::Less => before_cursor.push(chr),
                std::cmp::Ordering::Equal => cursor = chr.to_string(),
                std::cmp::Ordering::Greater => after_cursor.push(chr),
            }
        }
        let text_style = Style::default();
        let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
        let formatted_line = Line::from(vec![
            Span::styled(before_cursor, text_style),
            Span::styled(cursor, cursor_style),
            Span::styled(after_cursor, text_style),
        ]);

        Paragraph::new(formatted_line)
    }

    pub fn is_empty(&self) -> bool {
        self.line.is_empty()
    }

    pub fn get_text(&self) -> String {
        self.line.iter().collect()
    }
}
