use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Widget};

pub struct Toggle<'a> {
    label: &'a str,
    is_on: bool,
    on_text: &'a str,
    off_text: &'a str,
    focused: bool,
    block: Option<Block<'a>>,
}

impl<'a> Toggle<'a> {
    pub fn new(label: &'a str, is_on: bool) -> Self {
        Self {
            label,
            is_on,
            on_text: "ON",
            off_text: "OFF",
            focused: false,
            block: None,
        }
    }

    pub fn on_text(mut self, text: &'a str) -> Self {
        self.on_text = text;
        self
    }

    pub fn off_text(mut self, text: &'a str) -> Self {
        self.off_text = text;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<'a> Widget for Toggle<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = self.block.unwrap_or_else(Block::default);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let status_text = if self.is_on {
            self.on_text
        } else {
            self.off_text
        };
        let mut style = Style::default();

        if self.focused {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }

        let content = if self.label.is_empty() {
            status_text.to_string()
        } else {
            format!("{}: {}", self.label, status_text)
        };

        let span = Span::styled(content, style);
        buf.set_span(inner.x, inner.y, &span, inner.width);
    }
}
