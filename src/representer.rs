use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};

#[derive(Debug)]
pub struct Representer {
    elements: Vec<Element>,
}

impl Representer {
    pub fn new() -> Self {
        Self {
            elements: Vec::with_capacity(100),
        }
    }

    pub fn push(&mut self, element: Element) {
        self.elements.push(element);
    }

    pub fn decorate(&self) -> Text<'_> {
        let mut lines = Vec::new();
        let mut line = Line::raw("");

        let normal_txt = Style::default().fg(Color::Black).bg(Color::White);
        let selected_txt = Style::default().fg(Color::Red).bg(Color::Gray);

        for e in &self.elements {
            let s = unsafe { str::from_utf8_unchecked(&e.val) };

            match e.t {
                ElementType::Normal => {
                    line.push_span(Span::styled(s, normal_txt));
                }
                ElementType::IncrementalSearch(false) => {
                    line.push_span(Span::styled(s, selected_txt));
                }
                ElementType::IncrementalSearch(true) => {
                    line.push_span(Span::styled(s, selected_txt.bg(Color::Black)));
                }
            }

            if *e.val.last().unwrap() == b'\n' {
                lines.push(line);
                line = Line::raw("");
            }
        }

        if line.width() > 0 {
            lines.push(line);
        }

        Text::from(lines)
    }
}

#[derive(Debug)]
pub struct Element {
    val: Vec<u8>,
    pub t: ElementType,
}

#[derive(Debug)]
pub enum ElementType {
    Normal,
    // true means element is currently at the cursor's position
    IncrementalSearch(bool),
}

impl Element {
    pub fn default() -> Self {
        Self {
            val: Vec::new(),
            t: ElementType::Normal,
        }
    }

    pub fn isearch_type(is_current: bool) -> Self {
        Self {
            val: Vec::new(),
            t: ElementType::IncrementalSearch(is_current),
        }
    }

    pub fn push(&mut self, v: u8) {
        self.val.push(v);
    }

    pub fn is_empty(&self) -> bool {
        self.val.is_empty()
    }
}
