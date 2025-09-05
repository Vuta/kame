use std::fs::{self, File, OpenOptions};
use std::io::Write;

use crate::buffer::Buffer;
use crate::message::Message;
use crate::representer::{Element, Representer};
use crate::undo::{Command, UndoManager};

const NEWLINE: u8 = b'\n';
const DIRTY_MASK: i8 = 0x01;
const SAVED_MASK: i8 = 0x02;
const MANUAL_POPUP_MASK: i8 = 0x04;
const PROMPT_MASK: i8 = 0x08;

#[derive(Debug)]
pub struct Editor {
    pub path: String,

    flags: i8,
    buffer: Buffer,
    isearch: ISearch,
    undo_manager: UndoManager,
    top: usize,
    _log: File, // TODO: Remove
}

impl Editor {
    pub fn new(buffer: String, path: String) -> Self {
        let buffer = Buffer::init(buffer);
        let log = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("tmp/log.log")
            .unwrap();
        let isearch = ISearch::new();
        let undo_manager = UndoManager::new();
        let flags = 0;

        Self {
            path,
            flags,
            buffer,
            isearch,
            undo_manager,
            top: 0,
            _log: log,
        }
    }

    pub fn update(&mut self, message: Message) {
        if self.is_in_manual_popup() && message != Message::UserManual {
            return;
        }

        self.flags &= !SAVED_MASK;

        if self.is_prompted() {
            self.match_isearch_buffer(message);
        } else {
            self.match_editing_buffer(message);
        }
    }

    fn match_editing_buffer(&mut self, message: Message) {
        match message {
            // ---------------- Editing ---------------------------------------- //
            Message::InsertNewLine => self.insert_newline(),
            Message::Insert(c) => self.insert_char(c),
            Message::InsertTab => self.insert_tab(),
            Message::DeleteUnderCursor => self.delete_under_cursor(),
            Message::DeleteBeforeCursor => self.delete_before_cursor(),
            Message::CutToEndOfLine => self.cut_to_eol(),
            Message::Undo => self.undo(),
            Message::Redo => self.redo(),

            // ---------------- Movement --------------------------------------- //
            Message::ForwardOneChar => self.forward_one_char(),
            Message::BackwardOneChar => self.backward_one_char(),
            Message::JumpToBeginningOfLine => self.jump_to_bol(),
            Message::JumpToEndOfLine => self.jump_to_eol(),
            Message::JumpToNextLine => self.jump_to_next_line(),
            Message::JumpToPreviousLine => self.jump_to_previous_line(),

            // ---------------- File operation --------------------------------- //
            Message::Noop => {}
            Message::Quit => panic!("something wrong"),
            Message::Save => self.save(),
            Message::UserManual => self.toggle_popup(),
            Message::Search => self.toggle_prompt(),
        }
    }

    fn match_isearch_buffer(&mut self, message: Message) {
        match message {
            Message::InsertNewLine => {
                if let Some(id) = self.isearch.fetch_next() {
                    self.buffer.jump(id);
                }
            }
            Message::Insert(c) => self.handle_search(Some(c)),
            Message::DeleteBeforeCursor => self.handle_search(None),
            Message::Noop => {}
            Message::Quit => panic!("something wrong"),
            _ => self.toggle_prompt(),
        }
    }

    pub fn current_isearch_term(&self) -> &String {
        &self.isearch.term
    }

    pub fn is_prompted(&self) -> bool {
        self.flags & PROMPT_MASK == PROMPT_MASK
    }

    pub fn is_in_manual_popup(&self) -> bool {
        self.flags & MANUAL_POPUP_MASK == MANUAL_POPUP_MASK
    }

    pub fn is_modified(&self) -> bool {
        self.flags & DIRTY_MASK == DIRTY_MASK
    }

    pub fn is_saved(&self) -> bool {
        self.flags & SAVED_MASK == SAVED_MASK
    }

    fn toggle_popup(&mut self) {
        self.flags ^= MANUAL_POPUP_MASK;
    }

    fn toggle_prompt(&mut self) {
        self.flags ^= PROMPT_MASK;

        self.isearch.clear();
    }

    // TODO: Optimize & unit tests
    pub fn get_current_point(&self) -> (usize, usize) {
        let mut rows = 0;
        let mut cols = 0;
        for b in self.buffer.before_insertion_point().iter() {
            if *b == NEWLINE {
                rows += 1;
                cols = 0;
            } else {
                cols += 1;
            }
        }

        (cols, rows)
    }

    pub fn make_cursor_visible(&mut self, point: (usize, usize), height: usize) -> (u16, u16) {
        let adjust_window = height / 2;

        if point.1 >= self.top + height {
            self.top += adjust_window;
            self.top = self.top.max(point.1);
        } else if point.1 < self.top {
            self.top = self.top.saturating_sub(adjust_window).min(point.1);
        }

        (point.0 as u16, (point.1.saturating_sub(self.top)) as u16)
    }

    // display from top-th line until (top + height)-th line
    // TODO: Rework this shit
    pub fn viewable_contents(&mut self, height: usize) -> Representer {
        assert!(height > 0, "invalid height");

        let top = self.top;
        let mut rows_cnt = 0;
        let mut element = Element::default();
        let mut representer = Representer::new();
        let mut searched_len = std::usize::MAX;

        for (i, b) in self.buffer.iter().enumerate() {
            if rows_cnt == top + height {
                break;
            }

            if rows_cnt >= top {
                if *b == NEWLINE {
                    element.push(*b);
                    representer.push(element);
                    element = Element::default();
                    rows_cnt += 1;

                    continue;
                }

                if !self.isearch.ids.is_empty() {
                    match self.isearch.ids.binary_search(&i) {
                        Ok(j) => {
                            searched_len = i;
                            let mut iter = self.buffer.iter();
                            iter.seek(i);
                            let mut searched = Element::isearch_type(j == self.isearch.current);

                            for _ in 0..self.isearch.term.len() {
                                searched.push(*iter.next().unwrap());
                            }

                            if !element.is_empty() {
                                representer.push(element);
                            }
                            representer.push(searched);

                            element = Element::default();
                        }
                        Err(_) => {
                            if i > searched_len && i < searched_len + self.isearch.term.len() {
                                continue;
                            }

                            element.push(*b);
                        }
                    }
                } else {
                    element.push(*b);
                }
            }

            if *b == NEWLINE {
                rows_cnt += 1;
            }
        }

        if !element.is_empty() {
            representer.push(element);
        }

        representer
    }

    fn save(&mut self) {
        let tmp_path = format!("{}{}", &self.path, ".tmp");
        let mut tmp = File::create(&tmp_path).expect("BUG!");
        tmp.write_all(self.buffer.before_insertion_point())
            .expect("BUG!");
        tmp.write_all(self.buffer.after_insertion_point())
            .expect("BUG!");

        // does not work if the original file changed its mount point during the editing, but who cares?
        fs::rename(tmp_path, &self.path).expect("BUG!");

        self.undo_manager.push(Command::Checkpoint);
        self.flags |= SAVED_MASK;
        self.flags &= !(DIRTY_MASK);
    }

    fn insert_newline(&mut self) {
        self.insert_char(NEWLINE as char);
    }

    // FIX: this is not the best way to handle tab
    fn insert_tab(&mut self) {
        for _ in 0..4 {
            self.insert_char(' ');
        }
    }

    fn handle_search(&mut self, d: Option<char>) {
        if let Some(id) = self.isearch.run(&self.buffer, d) {
            self.buffer.jump(id);
        }
    }

    fn insert_char(&mut self, c: char) {
        self.flags |= DIRTY_MASK;
        let prev_iptr = self.buffer.iptr;
        self.buffer.insert(c);

        self.undo_manager
            .push(Command::Insert((prev_iptr, c.to_string())));
    }

    fn delete_before_cursor(&mut self) {
        self.flags |= DIRTY_MASK;

        if let Some(bytes) = self.buffer.delete_before_ptr() {
            let prev_iptr = self.buffer.iptr;
            self.undo_manager
                .push(Command::DeleteBefore((prev_iptr, bytes)));
        }
    }

    fn delete_under_cursor(&mut self) {
        self.flags |= DIRTY_MASK;

        if let Some(bytes) = self.buffer.delete_after_ptr() {
            let prev_iptr = self.buffer.iptr;
            self.undo_manager
                .push(Command::DeleteAfter((prev_iptr, bytes)));
        }
    }

    fn cut_to_eol(&mut self) {
        let mut cols: usize = 0;
        for b in self.buffer.after_insertion_point().iter() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        for _ in 0..=cols.saturating_sub(1) {
            self.delete_under_cursor();
        }

        let mut cols: usize = 0;
        for b in self.buffer.before_insertion_point().iter().rev() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        if cols == 0 {
            self.delete_before_cursor();
        }
    }

    fn undo(&mut self) {
        self.undo_manager.undo(&mut self.buffer);
    }

    fn redo(&mut self) {
        self.undo_manager.redo(&mut self.buffer);
    }

    fn forward_one_char(&mut self) {
        self.buffer.move_ptr_forward();
    }

    fn backward_one_char(&mut self) {
        self.buffer.move_ptr_backward();
    }

    // TODO: Optimize + unit tests
    fn jump_to_eol(&mut self) {
        let mut cols = 0;
        for b in self.buffer.after_insertion_point().iter() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        for _ in 0..cols {
            self.forward_one_char();
        }
    }

    // TODO: Optimize + unit tests
    fn jump_to_bol(&mut self) {
        let mut cols = 0;
        for b in self.buffer.before_insertion_point().iter().rev() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        for _ in 0..cols {
            self.backward_one_char();
        }
    }

    // TODO: too slow, need optimization + unit tests
    fn jump_to_next_line(&mut self) {
        let point = self.get_current_point();
        self.jump_to_eol();
        self.forward_one_char();

        let mut cols = 0;
        for b in self.buffer.after_insertion_point().iter() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        for _ in 0..point.0.min(cols) {
            self.forward_one_char();
        }
    }

    // TODO: too slow, need optimization + unit tests
    fn jump_to_previous_line(&mut self) {
        let point = self.get_current_point();
        self.jump_to_bol();
        self.backward_one_char();

        let mut cols: usize = 0;
        for b in self.buffer.before_insertion_point().iter().rev() {
            if *b == NEWLINE {
                break;
            } else {
                cols += 1;
            }
        }

        for _ in 0..cols.saturating_sub(point.0) {
            self.backward_one_char();
        }
    }
}

#[derive(Debug)]
struct ISearch {
    term: String,
    ids: Vec<usize>,
    current: usize,
}

impl ISearch {
    fn new() -> Self {
        Self {
            term: String::with_capacity(64),
            ids: Vec::with_capacity(32),
            current: 0,
        }
    }

    fn clear(&mut self) {
        self.term.clear();
        self.ids.clear();
        self.current = 0;
    }

    fn run(&mut self, buf: &Buffer, d: Option<char>) -> Option<usize> {
        match d {
            Some(c) => {
                self.term.push(c);
            }
            None => {
                self.term.pop();
            }
        }

        if self.term.is_empty() {
            self.clear();

            None
        } else {
            let tmp = &mut buf.before_insertion_point().to_vec();
            tmp.extend(buf.after_insertion_point());
            let s = str::from_utf8(&tmp).unwrap();

            self.ids = s
                .match_indices(&self.term)
                .map(|(i, _)| i)
                .collect::<Vec<usize>>();

            if self.ids.is_empty() {
                None
            } else {
                Some(self.ids[self.current])
            }
        }
    }

    fn fetch_next(&mut self) -> Option<usize> {
        if self.ids.is_empty() {
            None
        } else {
            self.current += 1;
            self.current %= self.ids.len();

            Some(self.ids[self.current])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_viewable_contents_1() {
        let buffer = String::from("Hello world\nThis is a test\nGood night!\n");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);
        editor.forward_one_char();
        editor.forward_one_char();

        // TODO: This is not the correct way to test this method
        assert_eq!(
            editor.viewable_contents(1).decorate().to_string(),
            "Hello world"
        );
        assert_eq!(
            editor.viewable_contents(2).decorate().to_string(),
            "Hello world\nThis is a test"
        );
        assert_eq!(
            editor.viewable_contents(3).decorate().to_string(),
            "Hello world\nThis is a test\nGood night!"
        );

        editor.top = 1;
        assert_eq!(
            editor.viewable_contents(1).decorate().to_string(),
            "This is a test"
        );
        assert_eq!(
            editor.viewable_contents(2).decorate().to_string(),
            "This is a test\nGood night!"
        );
        assert_eq!(
            editor.viewable_contents(3).decorate().to_string(),
            "This is a test\nGood night!"
        );

        editor.top = 2;
        assert_eq!(
            editor.viewable_contents(1).decorate().to_string(),
            "Good night!"
        );
        assert_eq!(
            editor.viewable_contents(2).decorate().to_string(),
            "Good night!"
        );
        assert_eq!(
            editor.viewable_contents(3).decorate().to_string(),
            "Good night!"
        );
    }

    #[test]
    fn test_editor_viewable_contents_2() {
        let buffer = String::from("");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);
        editor.insert_char('a');
        editor.backward_one_char();
        editor.forward_one_char();

        assert_eq!(editor.viewable_contents(43).decorate().to_string(), "a");
    }

    #[test]
    fn test_editor_viewable_contents_3() {
        let buffer = String::from("mod buffer;\n\nBufReader");
        let path = String::from("test_tmp.txt");
        let mut editor = Editor::new(buffer, path);
        dbg!(&editor.viewable_contents(43));

        editor.toggle_prompt();
        editor.insert_char('B');
        editor.insert_char('u');

        dbg!(&editor.viewable_contents(43));
        dbg!(&editor.viewable_contents(43).decorate().to_string());
    }

    #[test]
    fn test_editor_jump_to_bol_1() {
        let buffer = String::from("");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);
        editor.jump_to_bol();

        assert_eq!(editor.buffer.iptr, 0);
        assert_eq!(editor.get_current_point(), (0, 0));
    }

    #[test]
    fn test_editor_jump_to_bol_2() {
        let buffer = String::from("a");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);
        editor.jump_to_bol();

        assert_eq!(editor.buffer.iptr, 0);
        assert_eq!(editor.get_current_point(), (0, 0));

        editor.forward_one_char();
        assert_eq!(editor.buffer.iptr, 1);
        assert_eq!(editor.get_current_point(), (1, 0));

        editor.jump_to_bol();
        assert_eq!(editor.buffer.iptr, 0);
        assert_eq!(editor.get_current_point(), (0, 0));
    }

    #[test]
    fn test_editor_jump_to_next_line_1() {
        let buffer = String::from("aaaaaaa\n\naaa\n");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);

        editor.forward_one_char();
        editor.jump_to_next_line();
        assert_eq!(editor.get_current_point(), (0, 1));
    }

    #[test]
    fn test_editor_jump_to_previous_line_1() {
        let buffer = String::from("aaaa\n\na\n");
        let path = String::from("dummy.txt");
        let mut editor = Editor::new(buffer, path);

        editor.jump_to_next_line();
        editor.jump_to_next_line();

        editor.jump_to_previous_line();
        assert_eq!(editor.get_current_point(), (0, 1));
    }

    #[test]
    fn test_editor_isearch() {
        let buffer = String::from("hello world\n\nxin chao\n");
        let path = String::from("test_tmp");
        let mut editor = Editor::new(buffer, path);

        editor.toggle_prompt();
        assert!(editor.isearch.ids.is_empty());

        editor.update(Message::Insert('o'));
        assert_eq!(editor.isearch.ids, vec![4, 7, 20]);

        let p = editor.get_current_point();
        assert_eq!(p, (4, 0));

        editor.update(Message::InsertNewLine);
        let p = editor.get_current_point();
        assert_eq!(p, (7, 0));

        editor.update(Message::InsertNewLine);
        let p = editor.get_current_point();
        assert_eq!(p, (7, 2));

        editor.update(Message::InsertNewLine);
        let p = editor.get_current_point();
        assert_eq!(p, (4, 0));

        editor.toggle_prompt();
        editor.update(Message::Insert('z'));
        assert_eq!(editor.buffer.to_string(), "hellzo world\n\nxin chao\n");
    }

    #[test]
    fn test_editor_redo() {
        let buffer = String::from("hello world");
        let path = String::from("test_tmp");
        let mut editor = Editor::new(buffer, path);

        editor.update(Message::JumpToEndOfLine);
        editor.update(Message::DeleteBeforeCursor);
        editor.update(Message::DeleteBeforeCursor);
        editor.update(Message::InsertNewLine);
        editor.update(Message::Undo);
        editor.update(Message::Redo);
        editor.update(Message::ForwardOneChar);

        assert_eq!(editor.buffer.to_string(), "hello wor\n");
    }
}
