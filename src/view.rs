use std::rc::Rc;

use crate::editor::Editor;

use ratatui::prelude::*;
use ratatui::{
    Frame,
    layout::{Flex, Size},
    widgets::{Block, Clear, Paragraph},
};

pub struct View {
    layout: Rc<[Rect]>,
}

impl View {
    pub fn new(size: Size) -> Self {
        let mode_line_h = 1;
        let prompt_line_h = 1;
        let main_h = size.height - mode_line_h - prompt_line_h;
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Max(main_h),
                Constraint::Min(mode_line_h),
                Constraint::Min(prompt_line_h),
            ])
            .split(Rect::new(0, 0, size.width, size.height));

        Self { layout }
    }

    pub fn render(&self, frame: &mut Frame, editor: &mut Editor) {
        // main
        let point = editor.get_current_point();
        let main_rect = self.layout[0];
        let mut cursor = editor.make_cursor_visible(point, main_rect.height as usize);
        let representer = editor.viewable_contents(main_rect.height as usize);
        let contents =
            Paragraph::new(representer.decorate()).style(Style::new().black().on_white());
        frame.render_widget(contents, self.layout[0]);

        // mode line
        let changes = if editor.is_modified() {
            "(modified)"
        } else if editor.is_saved() {
            "(saved)"
        } else {
            ""
        };
        let text = format!("~:~~ {}  L{}  {}", editor.path, point.1, changes);
        let contents = Paragraph::new(text).style(Style::new().white().on_blue().italic());
        frame.render_widget(contents, self.layout[1]);

        // prompt line
        let cmd_prompt_style = Style::new().black().on_white();
        let contents = if editor.is_prompted() {
            let prompt = format!(" search {}", editor.current_isearch_term());
            cursor = (
                prompt.len() as u16,
                main_rect.height + self.layout[1].height,
            );
            Paragraph::new(prompt)
        } else {
            Paragraph::new("")
        };
        frame.render_widget(contents.style(cmd_prompt_style), self.layout[2]);

        // user manual popup
        if editor.is_in_manual_popup() {
            let a = self.center(
                main_rect,
                Constraint::Length(main_rect.width / 2),
                Constraint::Length(main_rect.height / 2),
            );
            let style = Style::new().white();
            let text = Text::from(vec![
                Line::from("User Manual").centered().style(style),
                Line::from("Movement Shortcut").style(style),
                Line::from("  - move cursor forward: ctrl + f").style(style),
                Line::from("  - move cursor backward: ctrl + b").style(style),
                Line::from("  - move cursor up: ctrl + p").style(style),
                Line::from("  - move cursor down: ctrl + n").style(style),
            ]);
            // TODO: Change to table
            let p = Paragraph::new(text)
                .block(Block::bordered())
                .style(Style::new().white().on_light_blue());
            frame.render_widget(Clear, a);
            frame.render_widget(p, a);
        } else {
            frame.set_cursor_position(cursor);
        }
    }

    // copied from Ratatui's docs
    fn center(&self, area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
        let [area] = Layout::horizontal([horizontal])
            .flex(Flex::Center)
            .areas(area);
        let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);

        area
    }
}
