use crossterm::terminal;
/// Authored by Kofi Otuo <otuokofi@outlook.com>
///
use editor::Editor;
use output::Output;
use reader::Reader;

const VERSION: &str = "0.0.1";
const TAB_STOP: usize = 8;
const QUIT_TIMES: u8 = 3;

pub mod cursor_controller;
pub mod editor;
pub mod editor_contents;
pub mod editor_rows;
pub mod output;
pub mod reader;
pub mod row;
pub mod search_index;
pub mod status_message;
pub mod syntax_highlighting;

struct CleanUp;

impl Drop for CleanUp {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable to disable raw mode");
        Output::clear_screen().expect("error");
    }
}

fn main() -> crossterm::Result<()> {
    let _clean_up = CleanUp;
    terminal::enable_raw_mode()?;
    let mut editor = Editor::new();
    while editor.run()? {}
    Ok(())
}
