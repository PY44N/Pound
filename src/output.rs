use std::{
    cmp, env, fs,
    io::{self, stdout, Write},
    path::PathBuf,
};

use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute, queue, style,
    terminal::{self, ClearType},
};

use crate::{
    cursor_controller::CursorController,
    editor_contents::EditorContents,
    editor_rows::{EditMode, EditorRows, FileType},
    reader::Reader,
    row::Row,
    search_index::{SearchDirection, SearchIndex},
    status_message::StatusMessage,
    syntax_highlighting::{HighlightType, RustHighlight, SyntaxHighlight},
    VERSION,
};

pub struct Output {
    pub win_size: (usize, usize),
    pub editor_contents: EditorContents,
    pub cursor_controller: CursorController,
    pub editor_rows: EditorRows,
    pub status_message: StatusMessage,
    pub dirty: u64,
    pub search_index: SearchIndex,
    pub syntax_highlight: Option<Box<dyn SyntaxHighlight>>,
}

impl Output {
    pub fn select_syntax(extension: &str) -> Option<Box<dyn SyntaxHighlight>> {
        let list: Vec<Box<dyn SyntaxHighlight>> = vec![Box::new(RustHighlight::new())];
        list.into_iter()
            .find(|it| it.extensions().contains(&extension))
    }

    pub fn new() -> Self {
        let win_size = terminal::size()
            .map(|(x, y)| (x as usize, y as usize - 2))
            .unwrap();
        let syntax_highlight = None; // modify
        let mut new_self = Self {
            win_size,
            editor_contents: EditorContents::new(),
            cursor_controller: CursorController::new(win_size),
            editor_rows: EditorRows::new(),
            status_message: StatusMessage::new("HELP: Ctrl-h".into()),
            dirty: 0,
            search_index: SearchIndex::new(),
            syntax_highlight,
        };

        match env::args().nth(1) {
            Some(file) => new_self
                .open_file(file.into())
                .expect("Failed to open file"),
            None => (),
        }

        new_self
    }

    pub fn prompt(&mut self, message: &str) -> Option<String> {
        self.prompt_callback(message, None)
    }

    pub fn prompt_callback(
        &mut self,
        message: &str,
        callback: Option<&dyn Fn(&mut Output, &str, KeyCode)>,
    ) -> Option<String> {
        let mut input = String::with_capacity(32);
        loop {
            self.status_message
                .set_message(message.replace("{}", &input));
            match self.refresh_screen() {
                Ok(_) => {}
                Err(_) => return None,
            }
            let key_event = Reader.read_key().unwrap();
            match key_event {
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                } => {
                    if !input.is_empty() {
                        self.status_message.set_message(String::new());
                        match callback {
                            Some(c) => c(self, &input, KeyCode::Enter),
                            None => {}
                        }
                        // $callback(output, &input, KeyCode::Enter);
                        break;
                    }
                }
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.status_message.set_message(String::new());
                    input.clear();
                    match callback {
                        Some(c) => c(self, &input, KeyCode::Esc),
                        None => {}
                    }
                    // $callback(output, &input, KeyCode::Esc);
                    break;
                }
                KeyEvent {
                    code: KeyCode::Backspace | KeyCode::Delete,
                    modifiers: KeyModifiers::NONE,
                } => {
                    input.pop();
                }
                KeyEvent {
                    code: code @ (KeyCode::Char(..) | KeyCode::Tab),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                } => {
                    input.push(match code {
                        KeyCode::Tab => '\t',
                        KeyCode::Char(ch) => ch,
                        _ => unreachable!(),
                    });
                }
                _ => {}
            }
            match callback {
                Some(c) => c(self, &input, key_event.code),
                None => {}
            }
            // $callback(output, &input, key_event.code);
        }
        if input.is_empty() {
            None
        } else {
            Some(input)
        }
    }

    pub fn save_file(&mut self) -> crossterm::Result<()> {
        if matches!(self.editor_rows.filename, None) {
            let prompt = self
                .prompt("Save as : {} (ESC to cancel)")
                .map(|it| it.into());
            if prompt.is_none() {
                self.status_message.set_message("Save Aborted".into());
                return Ok(());
            }
            /* add the following */
            prompt
                .as_ref()
                .and_then(|path: &PathBuf| path.extension())
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    Output::select_syntax(ext).map(|syntax| {
                        let highlight = self.syntax_highlight.insert(syntax);
                        for i in 0..self.editor_rows.number_of_rows() {
                            highlight.update_syntax(i, &mut self.editor_rows.row_contents)
                        }
                    })
                });

            self.editor_rows.filename = prompt
        }
        self.editor_rows.save().map(|len| {
            self.status_message
                .set_message(format!("{} bytes written to disk", len));
            self.dirty = 0
        })?;

        Ok(())
    }

    pub fn open_file(&mut self, open_file: PathBuf) -> crossterm::Result<()> {
        if self.dirty != 0 {
            let save_prompt = self.prompt("You have unsaved changes, save? (y/n) {}");
            match save_prompt {
                Some(answer) => {
                    if answer.to_lowercase() == "y" {
                        self.save_file()?;
                    }
                }
                None => self.status_message.set_message("Open File Aborted".into()),
            }
        }

        if open_file.is_file() {
            self.editor_rows = EditorRows::from_file(open_file, &mut self.syntax_highlight);
        } else if open_file.is_dir() {
            let mut rows = vec![];

            for file in fs::read_dir(open_file).unwrap() {
                let mut row =
                    Row::new(file.unwrap().path().to_str().unwrap().into(), String::new());

                EditorRows::render_row(&mut row);

                rows.push(row);
            }

            let editor_rows = EditorRows {
                row_contents: rows,
                filename: None,
                file_type: FileType::DIR,
                edit_mode: EditMode::READONLY,
            };

            self.editor_rows = editor_rows;
        } else {
            self.editor_rows = EditorRows {
                row_contents: Vec::new(),
                filename: Some(open_file),
                file_type: FileType::FILE,
                edit_mode: EditMode::NORMAL,
            }
        }

        Ok(())
    }

    pub fn clear_screen() -> crossterm::Result<()> {
        execute!(stdout(), terminal::Clear(ClearType::All))?;
        execute!(stdout(), cursor::MoveTo(0, 0))
    }

    pub fn find_callback(output: &mut Output, keyword: &str, key_code: KeyCode) {
        if let Some((index, highlight)) = output.search_index.previous_highlight.take() {
            output.editor_rows.get_editor_row_mut(index).highlight = highlight;
        }
        match key_code {
            KeyCode::Esc | KeyCode::Enter => {
                output.search_index.reset();
            }
            _ => {
                output.search_index.y_direction = None;
                output.search_index.x_direction = None;
                match key_code {
                    KeyCode::Down => {
                        output.search_index.y_direction = SearchDirection::Forward.into()
                    }
                    KeyCode::Up => {
                        output.search_index.y_direction = SearchDirection::Backward.into()
                    }
                    KeyCode::Left => {
                        output.search_index.x_direction = SearchDirection::Backward.into()
                    }
                    KeyCode::Right => {
                        output.search_index.x_direction = SearchDirection::Forward.into()
                    }
                    _ => {}
                }
                for i in 0..output.editor_rows.number_of_rows() {
                    let row_index = match output.search_index.y_direction.as_ref() {
                        None => {
                            if output.search_index.x_direction.is_none() {
                                output.search_index.y_index = i;
                            }
                            output.search_index.y_index
                        }
                        Some(dir) => {
                            if matches!(dir, SearchDirection::Forward) {
                                output.search_index.y_index + i + 1
                            } else {
                                let res = output.search_index.y_index.saturating_sub(i);
                                if res == 0 {
                                    break;
                                }
                                res - 1
                            }
                        }
                    };
                    if row_index > output.editor_rows.number_of_rows() - 1 {
                        break;
                    }
                    let row = output.editor_rows.get_editor_row_mut(row_index);
                    let index = match output.search_index.x_direction.as_ref() {
                        None => row.render.find(&keyword),
                        Some(dir) => {
                            let index = if matches!(dir, SearchDirection::Forward) {
                                let start =
                                    cmp::min(row.render.len(), output.search_index.x_index + 1);
                                row.render[start..]
                                    .find(&keyword)
                                    .map(|index| index + start)
                            } else {
                                row.render[..output.search_index.x_index].rfind(&keyword)
                            };
                            if index.is_none() {
                                break;
                            }
                            index
                        }
                    };
                    if let Some(index) = index {
                        output.search_index.previous_highlight =
                            Some((row_index, row.highlight.clone()));
                        (index..index + keyword.len())
                            .for_each(|index| row.highlight[index] = HighlightType::SearchMatch);
                        output.cursor_controller.cursor_y = row_index;
                        output.search_index.y_index = row_index;
                        output.search_index.x_index = index;
                        output.cursor_controller.cursor_x = row.get_row_content_x(index);
                        output.cursor_controller.row_offset = output.editor_rows.number_of_rows();
                        break;
                    }
                }
            }
        }
    }

    pub fn find(&mut self) -> io::Result<()> {
        let cursor_controller = self.cursor_controller;
        if self
            .prompt_callback(
                "Search: {} (Use ESC / Arrows / Enter)",
                Some(&Output::find_callback),
            )
            .is_none()
        {
            self.cursor_controller = cursor_controller
        }
        Ok(())
    }

    pub fn draw_message_bar(&mut self) {
        queue!(
            self.editor_contents,
            terminal::Clear(ClearType::UntilNewLine)
        )
        .unwrap();
        if let Some(msg) = self.status_message.message() {
            self.editor_contents
                .push_str(&msg[..cmp::min(self.win_size.0, msg.len())]);
        }
    }

    pub fn delete_char(&mut self) {
        if self.editor_rows.edit_mode == EditMode::READONLY {
            self.status_message
                .set_message("Failed to edit readonly buffer".into());
            return;
        }

        if self.cursor_controller.cursor_y == self.editor_rows.number_of_rows() {
            return;
        }
        if self.cursor_controller.cursor_y == 0 && self.cursor_controller.cursor_x == 0 {
            return;
        }
        if self.cursor_controller.cursor_x > 0 {
            self.editor_rows
                .get_editor_row_mut(self.cursor_controller.cursor_y)
                .delete_char(self.cursor_controller.cursor_x - 1);
            self.cursor_controller.cursor_x -= 1;
        } else {
            let previous_row_content = self
                .editor_rows
                .get_row(self.cursor_controller.cursor_y - 1);
            self.cursor_controller.cursor_x = previous_row_content.len();
            self.editor_rows
                .join_adjacent_rows(self.cursor_controller.cursor_y);
            self.cursor_controller.cursor_y -= 1;
        }
        if let Some(it) = self.syntax_highlight.as_ref() {
            it.update_syntax(
                self.cursor_controller.cursor_y,
                &mut self.editor_rows.row_contents,
            );
        }
        self.dirty += 1;
    }

    pub fn insert_newline(&mut self) {
        if self.editor_rows.edit_mode == EditMode::READONLY {
            self.status_message
                .set_message("Failed to edit readonly buffer".into());
            return;
        }

        if self.cursor_controller.cursor_x == 0 {
            self.editor_rows
                .insert_row(self.cursor_controller.cursor_y, String::new())
        } else {
            let current_row = self
                .editor_rows
                .get_editor_row_mut(self.cursor_controller.cursor_y);
            let new_row_content = current_row.row_content[self.cursor_controller.cursor_x..].into();
            current_row
                .row_content
                .truncate(self.cursor_controller.cursor_x);
            EditorRows::render_row(current_row);
            self.editor_rows
                .insert_row(self.cursor_controller.cursor_y + 1, new_row_content);
            if let Some(it) = self.syntax_highlight.as_ref() {
                it.update_syntax(
                    self.cursor_controller.cursor_y,
                    &mut self.editor_rows.row_contents,
                );
                it.update_syntax(
                    self.cursor_controller.cursor_y + 1,
                    &mut self.editor_rows.row_contents,
                )
            }
        }
        self.cursor_controller.cursor_x = 0;
        self.cursor_controller.cursor_y += 1;
        self.dirty += 1;
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.editor_rows.edit_mode == EditMode::READONLY {
            self.status_message
                .set_message("Failed to edit readonly buffer".into());
            return;
        }

        if self.cursor_controller.cursor_y == self.editor_rows.number_of_rows() {
            self.editor_rows
                .insert_row(self.editor_rows.number_of_rows(), String::new());
            self.dirty += 1;
        }
        self.editor_rows
            .get_editor_row_mut(self.cursor_controller.cursor_y)
            .insert_char(self.cursor_controller.cursor_x, ch);
        if let Some(it) = self.syntax_highlight.as_ref() {
            it.update_syntax(
                self.cursor_controller.cursor_y,
                &mut self.editor_rows.row_contents,
            )
        }
        self.cursor_controller.cursor_x += 1;
        self.dirty += 1;
    }

    pub fn draw_status_bar(&mut self) {
        self.editor_contents
            .push_str(&style::Attribute::Reverse.to_string());
        let info = format!(
            "{} {} -- {} lines",
            self.editor_rows
                .filename
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("[No Name]"),
            if self.dirty > 0 { "(modified)" } else { "" },
            self.editor_rows.number_of_rows()
        );
        let info_len = cmp::min(info.len(), self.win_size.0);
        /* modify the following */
        let line_info = format!(
            "{} | {}/{}",
            self.syntax_highlight
                .as_ref()
                .map(|highlight| highlight.file_type())
                .unwrap_or("no ft"),
            self.cursor_controller.cursor_y + 1,
            self.editor_rows.number_of_rows()
        );
        self.editor_contents.push_str(&info[..info_len]);
        for i in info_len..self.win_size.0 {
            if self.win_size.0 - i == line_info.len() {
                self.editor_contents.push_str(&line_info);
                break;
            } else {
                self.editor_contents.push(' ')
            }
        }
        self.editor_contents
            .push_str(&style::Attribute::Reset.to_string());
        self.editor_contents.push_str("\r\n");
    }

    pub fn draw_rows(&mut self) {
        let screen_rows = self.win_size.1;
        let screen_columns = self.win_size.0;
        for i in 0..screen_rows {
            let file_row = i + self.cursor_controller.row_offset;
            if file_row >= self.editor_rows.number_of_rows() {
                if self.editor_rows.number_of_rows() == 0 && i == screen_rows / 3 {
                    let mut welcome = format!("Pound Editor --- Version {}", VERSION);
                    if welcome.len() > screen_columns {
                        welcome.truncate(screen_columns)
                    }
                    let mut padding = (screen_columns - welcome.len()) / 2;
                    if padding != 0 {
                        self.editor_contents.push('~');
                        padding -= 1
                    }
                    (0..padding).for_each(|_| self.editor_contents.push(' '));
                    self.editor_contents.push_str(&welcome);
                } else {
                    self.editor_contents.push('~');
                }
            } else {
                let row = self.editor_rows.get_editor_row(file_row);
                let render = &row.render;
                let column_offset = self.cursor_controller.column_offset;
                let len = cmp::min(render.len().saturating_sub(column_offset), screen_columns);
                let start = if len == 0 { 0 } else { column_offset };
                let render = render.chars().skip(start).take(len).collect::<String>();
                self.syntax_highlight
                    .as_ref()
                    .map(|syntax_highlight| {
                        syntax_highlight.color_row(
                            &render,
                            &row.highlight[start..cmp::min(start + len, row.highlight.len())],
                            &mut self.editor_contents,
                        )
                    })
                    .unwrap_or_else(|| self.editor_contents.push_str(&render));
            }
            queue!(
                self.editor_contents,
                terminal::Clear(ClearType::UntilNewLine)
            )
            .unwrap();
            self.editor_contents.push_str("\r\n");
        }
    }

    pub fn move_cursor(&mut self, direction: KeyCode) {
        self.cursor_controller
            .move_cursor(direction, &self.editor_rows);
    }

    pub fn refresh_screen(&mut self) -> crossterm::Result<()> {
        self.cursor_controller.scroll(&self.editor_rows);
        queue!(self.editor_contents, cursor::Hide, cursor::MoveTo(0, 0))?;
        self.draw_rows();
        self.draw_status_bar();
        self.draw_message_bar();
        let cursor_x = self.cursor_controller.render_x - self.cursor_controller.column_offset;
        let cursor_y = self.cursor_controller.cursor_y - self.cursor_controller.row_offset;
        queue!(
            self.editor_contents,
            cursor::MoveTo(cursor_x as u16, cursor_y as u16),
            cursor::Show
        )?;
        self.editor_contents.flush()
    }
}
