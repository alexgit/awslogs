use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, StatefulWidget, Widget};

#[derive(Clone, Debug)]
pub struct ColumnPickerState {
    selections: Vec<bool>,
    selected: usize,
    scroll: usize,
}

impl ColumnPickerState {
    pub fn new(selections: Vec<bool>) -> Self {
        Self {
            selections,
            selected: 0,
            scroll: 0,
        }
    }

    pub fn into_selections(self) -> Vec<bool> {
        self.selections
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.selections.is_empty() {
            return;
        }
        let len = self.selections.len() as i32;
        let mut next = self.selected as i32 + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }
        self.selected = next as usize;
    }

    pub fn toggle_selected(&mut self) {
        if self.selections.is_empty() {
            return;
        }
        let idx = self.selected.min(self.selections.len() - 1);
        let currently_on = self.selections[idx];
        if currently_on {
            let remaining = self.selections.iter().filter(|value| **value).count();
            if remaining <= 1 {
                return;
            }
        }
        self.selections[idx] = !currently_on;
    }

    fn ensure_visible(&mut self, view_height: usize) {
        if self.selections.is_empty() || view_height == 0 {
            self.scroll = 0;
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
            return;
        }
        let view_height = view_height.min(self.selections.len());
        let bottom = self.scroll.saturating_add(view_height.saturating_sub(1));
        if self.selected > bottom {
            let needed = self.selected + 1;
            self.scroll = needed.saturating_sub(view_height);
        }
        let max_scroll = self.selections.len().saturating_sub(view_height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    fn visible_bounds(&mut self, view_height: usize) -> (usize, usize) {
        self.ensure_visible(view_height);
        let end = (self.scroll + view_height).min(self.selections.len());
        (self.scroll, end)
    }
}

pub struct ColumnVisibilityModal<'a> {
    headers: &'a [String],
}

impl<'a> ColumnVisibilityModal<'a> {
    pub fn new(headers: &'a [String]) -> Self {
        Self { headers }
    }
}

impl StatefulWidget for ColumnVisibilityModal<'_> {
    type State = ColumnPickerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title("Select columns")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let (list_area, help_area) = if inner.height > 2 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);
            (chunks[0], Some(chunks[1]))
        } else {
            (inner, None)
        };

        let view_height = list_area.height as usize;
        let (start, end) = state.visible_bounds(view_height);

        for y in 0..list_area.height {
            let row_y = list_area.y + y;
            for x in 0..list_area.width {
                buf.get_mut(list_area.x + x, row_y).set_symbol(" ");
            }
        }

        for (line_offset, idx) in (start..end).enumerate() {
            let header = self
                .headers
                .get(idx)
                .map(|s| s.as_str())
                .unwrap_or_default();
            let checked = if state.selections.get(idx).copied().unwrap_or(false) {
                'x'
            } else {
                ' '
            };
            let display = format!("[{}] {}", checked, header);

            let mut style = Style::default();
            if idx == state.selected {
                style = style
                    .fg(Color::Black)
                    .bg(Color::Rgb(255, 246, 199))
                    .add_modifier(Modifier::BOLD);
            }

            let span = Span::styled(display, style);
            buf.set_span(
                list_area.x,
                list_area.y + line_offset as u16,
                &span,
                list_area.width,
            );
        }

        if let Some(area) = help_area {
            if area.height > 0 {
                let hint = Span::styled(
                    "↑/↓ move • Space toggle • Enter apply • Esc cancel",
                    Style::default().fg(Color::DarkGray),
                );
                buf.set_span(area.x, area.y, &hint, area.width);
            }
        }
    }
}
