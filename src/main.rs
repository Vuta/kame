mod buffer;
mod editor;
mod message;
mod representer;
mod undo;
mod view;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufReader, Read};

use crate::editor::Editor;
use crate::message::Message;
use crate::view::View;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        // TODO: Do better
        panic!("must accept file path");
    }

    let path = &args[1];
    let file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(path)?;
    let mut reader = BufReader::new(&file);

    let mut buffer = String::new();
    // TODO: restore terminal state before returning early (do this for all `?`)
    reader.read_to_string(&mut buffer)?;
    let mut editor = Editor::new(buffer, String::from(path));
    let size = terminal.size().unwrap();
    let view = View::new(size);

    loop {
        terminal.draw(|frame| view.render(frame, &mut editor))?;

        let message = match event::read()? {
            Event::Key(key) if key.modifiers == KeyModifiers::CONTROL => match key.code {
                KeyCode::Char('q') => Message::Quit,
                KeyCode::Char('u') => Message::Undo,
                KeyCode::Char('g') => Message::Redo,
                KeyCode::Char('d') => Message::DeleteUnderCursor,
                KeyCode::Char('k') => Message::CutToEndOfLine,
                KeyCode::Char('f') => Message::ForwardOneChar,
                KeyCode::Char('b') => Message::BackwardOneChar,
                KeyCode::Char('p') => Message::JumpToPreviousLine,
                KeyCode::Char('n') => Message::JumpToNextLine,
                KeyCode::Char('e') => Message::JumpToEndOfLine,
                KeyCode::Char('a') => Message::JumpToBeginningOfLine,
                KeyCode::Char('s') => Message::Save,
                KeyCode::Char('h') => Message::UserManual,
                KeyCode::Char('r') => Message::Search,
                _ => Message::Noop,
            },
            Event::Key(key) => match key.code {
                KeyCode::Backspace => Message::DeleteBeforeCursor,
                KeyCode::Enter => Message::InsertNewLine,
                KeyCode::Tab => Message::InsertTab,
                KeyCode::Char(c) => Message::Insert(c),
                _ => Message::Noop,
            },
            _ => Message::Noop,
        };

        if message == Message::Quit {
            break;
        }

        editor.update(message);
    }

    ratatui::restore();

    Ok(())
}
