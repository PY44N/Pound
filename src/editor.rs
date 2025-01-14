use std::{cmp, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{editor_rows::FileType, output::Output, reader::Reader, QUIT_TIMES};

pub struct Editor {
    reader: Reader,
    output: Output,
    quit_times: u8,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            reader: Reader,
            output: Output::new(),
            quit_times: QUIT_TIMES,
        }
    }

    pub fn process_keypress(&mut self) -> crossterm::Result<bool> {
        match self.reader.read_key()? {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                if self.output.dirty > 0 && self.quit_times > 0 {
                    self.output.status_message.set_message(format!(
                        "WARNING!!! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                        self.quit_times
                    ));
                    self.quit_times -= 1;
                    return Ok(true);
                }
                return Ok(false);
            }
            KeyEvent {
                code:
                    direction @ (KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Home
                    | KeyCode::End),
                modifiers: KeyModifiers::NONE,
            } => self.output.move_cursor(direction),
            KeyEvent {
                code: val @ (KeyCode::PageUp | KeyCode::PageDown),
                modifiers: KeyModifiers::NONE,
            } => {
                if matches!(val, KeyCode::PageUp) {
                    self.output.cursor_controller.cursor_y =
                        self.output.cursor_controller.row_offset
                } else {
                    self.output.cursor_controller.cursor_y = cmp::min(
                        self.output.win_size.1 + self.output.cursor_controller.row_offset - 1,
                        self.output.editor_rows.number_of_rows() - 1,
                    );
                }
                (0..self.output.win_size.1).for_each(|_| {
                    self.output.move_cursor(if matches!(val, KeyCode::PageUp) {
                        KeyCode::Up
                    } else {
                        KeyCode::Down
                    });
                })
            }
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                self.output.save_file()?;
            }
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                self.output.find()?;
            }
            KeyEvent {
                code: key @ (KeyCode::Backspace | KeyCode::Delete),
                modifiers: KeyModifiers::NONE,
            } => {
                if matches!(key, KeyCode::Delete) {
                    self.output.move_cursor(KeyCode::Right)
                }
                self.output.delete_char()
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            } => {
                if self.output.editor_rows.file_type == FileType::DIR {
                    self.output.open_file(
                        self.output
                            .editor_rows
                            .get_editor_row(self.output.cursor_controller.cursor_y)
                            .row_content
                            .clone()
                            .into(),
                    )?;
                } else {
                    self.output.insert_newline()
                }
            }
            KeyEvent {
                code: code @ (KeyCode::Char(..) | KeyCode::Tab),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            } => self.output.insert_char(match code {
                KeyCode::Tab => '\t',
                KeyCode::Char(ch) => ch,
                _ => unreachable!(),
            }),
            KeyEvent {
                code: KeyCode::Char('o'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                let open_prompt: Option<PathBuf> = self
                    .output
                    .prompt("Open file: {} (ESC to cancel)")
                    .map(|v| v.into());
                match open_prompt {
                    Some(open_file) => {
                        self.output.open_file(open_file)?;
                    }
                    None => {}
                }
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
            } => self.output.status_message.set_message(
                "HELP: Ctrl-S = Save | Ctrl-Q = Quit | Ctrl-F = Find | Ctrl-O = Open".into(),
            ),
            _ => {}
        }
        self.quit_times = QUIT_TIMES;
        Ok(true)
    }

    pub fn run(&mut self) -> crossterm::Result<bool> {
        self.output.refresh_screen()?;
        self.process_keypress()
    }
}
