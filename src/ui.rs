use std::borrow::Cow;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use ratatui::Frame;
use tui_input::Input as SingleLineInput;

use crate::app::{App, FocusField, OpenDialogState, SaveDialogMode, SaveDialogState, StatusKind};
use crate::help;
use crate::presentation::{format_modal_message, format_modal_value};
use crate::widgets::column_picker::ColumnVisibilityModal;
use crate::widgets::toggle::Toggle;

// Longest known region identifier (ap-southeast-3) is 15 characters; add two for borders.
const AWS_REGION_FIELD_WIDTH: u16 = 18;

pub fn draw_ui(frame: &mut Frame, app: &mut App) {
    let frame_height = frame.size().height;
    let has_inputs = !app.inputs_collapsed;
    let show_status = app.submitting || matches!(app.status_kind, StatusKind::Error);
    let status_height = if show_status { 3 } else { 0 };
    let top_row_height = if has_inputs { 3 } else { 0 };
    let fixed_height = top_row_height + status_height;
    let available_for_query_and_results = frame_height.saturating_sub(fixed_height);

    let mut constraints = Vec::new();

    if has_inputs {
        let min_query_height = 5;
        let min_results_height = 6;
        let mut desired_query_height = (app.query_area.lines().len() as u16)
            .max(1)
            .saturating_add(2); // block borders
        if desired_query_height < min_query_height {
            desired_query_height = min_query_height;
        }
        let mut max_query_height = available_for_query_and_results;
        if available_for_query_and_results > min_results_height {
            max_query_height = available_for_query_and_results.saturating_sub(min_results_height);
            if max_query_height < min_query_height {
                max_query_height = min_query_height.min(available_for_query_and_results);
            }
        }
        if desired_query_height > max_query_height {
            desired_query_height = max_query_height;
        }
        let query_row_height = desired_query_height.min(available_for_query_and_results);
        constraints.push(Constraint::Length(top_row_height));
        constraints.push(Constraint::Length(query_row_height));
    }

    constraints.push(Constraint::Min(0)); // results
    if show_status {
        constraints.push(Constraint::Length(3));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.size());

    let status_chunk = if show_status {
        Some(chunks[chunks.len() - 1])
    } else {
        None
    };

    if let Some(status_chunk) = status_chunk {
        let mut help_text = Vec::new();
        let mut first_line_style = Style::default();
        let mut block = Block::default().title("Status").borders(Borders::ALL);
        if matches!(app.status_kind, StatusKind::Error) {
            let accent = Color::Rgb(200, 90, 90);
            first_line_style = first_line_style.fg(accent);
            block = block.border_style(Style::default().fg(accent));
        }
        help_text.push(Line::from(Span::styled(
            app.status.clone(),
            first_line_style,
        )));
        help_text.push(Line::from(
            "Tab: Next • Shift+Tab: Previous • Ctrl+Enter/Ctrl+R/F5: Run • Ctrl+H: Help • Ctrl+C/Esc: Quit",
        ));
        let status = Paragraph::new(help_text)
            .wrap(Wrap { trim: true })
            .block(block);
        frame.render_widget(status, status_chunk);
    }

    let render_input_field =
        |frame: &mut Frame, area: Rect, title: &str, focused: bool, input: &SingleLineInput| {
            let block = input_block(title, focused);
            let inner = block.inner(area);
            let widget = Paragraph::new(input.value()).block(block.clone());
            frame.render_widget(widget, area);
            if focused && inner.width > 0 && inner.height > 0 {
                let width = inner.width as usize;
                let scroll = input.visual_scroll(width);
                let cursor = input.visual_cursor();
                let visible_col = cursor.saturating_sub(scroll);
                let max_col = width.saturating_sub(1);
                let cursor_col = visible_col.min(max_col);
                let x = inner.x + cursor_col as u16;
                let y = inner.y;
                frame.set_cursor(x, y);
            }
        };

    let mut chunk_index = 0;
    let top_chunk = if has_inputs {
        let area = chunks[chunk_index];
        chunk_index += 1;
        Some(area)
    } else {
        None
    };
    let query_chunk = if has_inputs {
        let area = chunks[chunk_index];
        chunk_index += 1;
        Some(area)
    } else {
        None
    };
    let results_area = chunks[chunk_index];

    if let Some(top_chunk) = top_chunk {
        let mut top_constraints = Vec::new();
        top_constraints.push(Constraint::Length(AWS_REGION_FIELD_WIDTH));
        if app.show_profile_picker() {
            top_constraints.push(Constraint::Length(40));
        }
        top_constraints.push(Constraint::Length(18));
        if app.relative_mode {
            top_constraints.push(Constraint::Length(24));
        } else {
            top_constraints.push(Constraint::Length(28));
            top_constraints.push(Constraint::Length(28));
        }
        top_constraints.push(Constraint::Min(20));

        let top_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(top_constraints)
            .split(top_chunk);

        let mut column = 0;

        let region_area = top_row[column];
        column += 1;
        render_input_field(
            frame,
            region_area,
            "AWS region",
            app.focus == FocusField::AwsRegion,
            &app.aws_region_input,
        );

        if app.show_profile_picker() {
            let area = top_row[column];
            column += 1;
            let block = input_block("AWS profile", app.focus == FocusField::AwsProfile);
            let display = app.selected_profile_name().unwrap_or("Auto");
            let total = app.aws_profiles.len();
            let profile_text = if total > 1 {
                let current = app.selected_profile_index.unwrap_or(0) + 1;
                format!("{display} ({current}/{total})")
            } else {
                display.to_string()
            };
            let widget = Paragraph::new(profile_text).block(block);
            frame.render_widget(widget, area);
        }

        let toggle_area = top_row[column];
        column += 1;
        let toggle_block = input_block("Time range", app.focus == FocusField::TimeMode);
        let toggle_widget = Toggle::new("Relative", app.relative_mode)
            .on_text("ON")
            .off_text("OFF")
            .focused(app.focus == FocusField::TimeMode)
            .block(toggle_block);
        frame.render_widget(toggle_widget, toggle_area);

        if app.relative_mode {
            let area = top_row[column];
            column += 1;
            let block = input_block("Relative range", app.focus == FocusField::RelativeRange);
            let style = if app.focus == FocusField::RelativeRange {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let label = app.current_relative_option().label;
            let lines = vec![Line::from(Span::styled(label, style))];
            let widget = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
            frame.render_widget(widget, area);
        } else {
            render_input_field(
                frame,
                top_row[column],
                "From (local)",
                app.focus == FocusField::From,
                &app.from_input,
            );
            column += 1;

            render_input_field(
                frame,
                top_row[column],
                "To (local)",
                app.focus == FocusField::To,
                &app.to_input,
            );
            column += 1;
        }

        render_input_field(
            frame,
            top_row[column],
            "Log group",
            app.focus == FocusField::LogGroup,
            &app.log_group_input,
        );
    }

    let query_row = if let Some(query_chunk) = query_chunk {
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(query_chunk);

        app.query_area.set_cursor_line_style(Style::default());
        let query_title = app.query_block_title();
        let query_block = input_block(Cow::Owned(query_title), app.focus == FocusField::Query);
        if app.focus == FocusField::Query {
            app.query_area
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            let hidden_style = app.query_area.cursor_line_style();
            app.query_area.set_cursor_style(hidden_style);
        }
        app.query_area.set_block(query_block.clone());
        frame.render_widget(app.query_area.widget(), row[0]);
        let inner = query_block.inner(row[0]);
        if inner.width > 0 && inner.height > 0 {
            let (cursor_row, cursor_col) = app.query_area.cursor();
            app.query_scroll_row =
                next_scroll_position(app.query_scroll_row, cursor_row, inner.height);
            app.query_scroll_col =
                next_scroll_position(app.query_scroll_col, cursor_col, inner.width);
        }
        Some(row)
    } else {
        None
    };
    let inner_height = results_area.height.saturating_sub(2) as usize;
    let has_table_rows = !app.results.rows.is_empty() && !app.filtered_indices.is_empty();
    let rows_height = if has_table_rows {
        inner_height.saturating_sub(1)
    } else {
        inner_height
    };
    app.update_results_view_height(rows_height.max(1));
    let total_rows = app.results.rows.len();
    let visible_rows = app.filtered_indices.len();
    let results_title = if total_rows > 0 {
        let mut metrics = vec![format!("{visible_rows}/{total_rows}")];
        if let Some(selected) = app
            .selected_filtered_index
            .filter(|_| !app.filtered_indices.is_empty())
        {
            metrics.push(format!("row {}", selected + 1));
        }
        format!("Query results ({})", metrics.join(" · "))
    } else {
        "Query results".to_string()
    };
    let mut results_block = Block::default().title(results_title).borders(Borders::ALL);
    if app.focus == FocusField::Results {
        results_block = results_block.border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    }

    if app.results.rows.is_empty() {
        let message = if app.results_initialized {
            "Query returned no results."
        } else {
            "Results will appear here."
        };
        let placeholder = Paragraph::new(message)
            .wrap(Wrap { trim: false })
            .block(results_block);
        frame.render_widget(placeholder, results_area);
    } else if app.filtered_indices.is_empty() {
        let placeholder = Paragraph::new("No results match the current filter.")
            .wrap(Wrap { trim: true })
            .block(results_block);
        frame.render_widget(placeholder, results_area);
    } else {
        app.ensure_column_visibility_len();
        let visible_columns = app.visible_column_indices();
        let header_cells: Vec<Cell> = visible_columns
            .iter()
            .filter_map(|&idx| app.results.headers.get(idx))
            .map(|h| Cell::from(h.clone()).style(Style::default().add_modifier(Modifier::BOLD)))
            .collect();
        let header = Row::new(header_cells);
        let selected_idx = if app.results_navigation {
            app.selected_filtered_index
        } else {
            None
        };
        let view_height = app.results_view_height.max(1);
        let filtered_len = app.filtered_indices.len();
        let start = app.results_scroll.min(filtered_len.saturating_sub(1));
        let end = (start + view_height).min(filtered_len);
        let visible_slice = &app.filtered_indices[start..end];
        let rows: Vec<Row> = visible_slice
            .iter()
            .enumerate()
            .map(|(offset, &idx)| {
                let position = start + offset;
                let row = &app.results.rows[idx];
                let lens_active = Some(position) == selected_idx;
                let row_cells: Vec<Cell> = visible_columns
                    .iter()
                    .filter_map(|&col_idx| row.cells.get(col_idx))
                    .map(|value| {
                        if lens_active {
                            let style = Style::default()
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD);
                            Cell::from(value.clone()).style(style)
                        } else {
                            Cell::from(value.clone())
                        }
                    })
                    .collect();
                let mut table_row = Row::new(row_cells);
                if lens_active {
                    table_row = table_row.style(
                        Style::default()
                            .bg(Color::Rgb(255, 246, 199))
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                table_row
            })
            .collect();
        let widths: Vec<Constraint> = visible_columns
            .iter()
            .map(|&col| {
                if col == 0 {
                    Constraint::Length(27)
                } else {
                    Constraint::Min(8)
                }
            })
            .collect();
        let table = Table::new(rows, widths)
            .header(header)
            .block(results_block)
            .column_spacing(1);
        frame.render_widget(table, results_area);
    }

    if let Some(query_row) = &query_row {
        if app.filter_active {
            render_input_field(
                frame,
                query_row[1],
                "Filter",
                app.focus == FocusField::Filter,
                &app.filter_input,
            );
        } else {
            // Clear the right-hand side when the filter is hidden
            let empty_block = Block::default().title("Filter").borders(Borders::ALL);
            frame.render_widget(empty_block, query_row[1]);
        }
    }

    if app.help_open {
        let overlay = centered_rect(80, 85, frame.size());
        frame.render_widget(Clear, overlay);

        let heading_style = Style::default().add_modifier(Modifier::BOLD);
        let help_lines: Vec<Line> = help::HELP_TEXT
            .lines()
            .map(|line| {
                if let Some(text) = line.strip_prefix("## ") {
                    Line::from(Span::styled(text, heading_style))
                } else if let Some(text) = line.strip_prefix("# ") {
                    Line::from(Span::styled(text, heading_style))
                } else {
                    Line::from(line)
                }
            })
            .collect();

        let help = Paragraph::new(help_lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
        );
        frame.render_widget(help, overlay);
    } else if app.column_modal_active() {
        let overlay = centered_rect(60, 60, frame.size());
        frame.render_widget(Clear, overlay);
        let headers = app.results.headers.clone();
        if let Some(state) = app.column_modal_state_mut() {
            let widget = ColumnVisibilityModal::new(headers.as_slice());
            frame.render_stateful_widget(widget, overlay, state);
        }
    } else if app.open_dialog_active() {
        render_open_dialog(frame, app);
    } else if app.save_dialog_active() {
        render_save_dialog(frame, app);
    } else if app.modal_open {
        if let Some(details) = app.selected_row_data() {
            let overlay = centered_rect(80, 70, frame.size());
            frame.render_widget(Clear, overlay);

            let mut detail_lines: Vec<Line> = Vec::new();
            detail_lines.push(Line::from(""));
            for (header, value) in details.iter() {
                let header_span = Span::styled(
                    format!("{header}:"),
                    Style::default().add_modifier(Modifier::BOLD),
                );
                let rendered = if header == "@message" {
                    format_modal_message(value)
                } else {
                    format_modal_value(value)
                };
                if rendered.is_empty() {
                    detail_lines.push(Line::from(vec![header_span.clone(), Span::raw(" <empty>")]));
                } else {
                    for (idx, line) in rendered.iter().enumerate() {
                        if idx == 0 {
                            detail_lines.push(Line::from(vec![
                                header_span.clone(),
                                Span::raw(format!(" {line}")),
                            ]));
                        } else {
                            detail_lines.push(Line::from(format!("    {line}")));
                        }
                    }
                }
                detail_lines.push(Line::from(""));
            }

            if detail_lines.is_empty() {
                detail_lines.push(Line::from("No data for this row."));
            }

            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                "C: Copy • Enter/Esc: Close",
                Style::default().fg(Color::DarkGray),
            )));

            let modal = Paragraph::new(detail_lines)
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title("Row detail")
                        .borders(Borders::ALL)
                        .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                );
            frame.render_widget(modal, overlay);
        }
    }
}

fn input_block<'a>(title: impl Into<Cow<'a, str>>, focused: bool) -> Block<'a> {
    let title_cow: Cow<'a, str> = title.into();
    let base = Block::default()
        .title(Line::from(title_cow.into_owned()))
        .borders(Borders::ALL);
    if focused {
        base.border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        base
    }
}

fn next_scroll_position(prev_top: u16, cursor: usize, length: u16) -> u16 {
    if length == 0 {
        return prev_top;
    }
    let cursor = cursor.min(u16::MAX as usize) as u16;
    let end = prev_top.saturating_add(length).saturating_sub(1);
    if cursor < prev_top {
        cursor
    } else if cursor > end {
        cursor + 1 - length
    } else {
        prev_top
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let horizontal_margin = if percent_x >= 100 {
        0
    } else {
        (100 - percent_x) / 2
    };
    let vertical_margin = if percent_y >= 100 {
        0
    } else {
        (100 - percent_y) / 2
    };
    let right_margin = 100_u16
        .saturating_sub(horizontal_margin)
        .saturating_sub(percent_x);
    let bottom_margin = 100_u16
        .saturating_sub(vertical_margin)
        .saturating_sub(percent_y);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(horizontal_margin),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(right_margin),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(vertical_margin),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(bottom_margin),
        ])
        .split(horizontal[1])[1]
}

fn render_save_dialog(frame: &mut Frame, app: &mut App) {
    let overlay = centered_rect(60, 60, frame.size());
    frame.render_widget(Clear, overlay);
    let Some(state) = app.save_dialog_state_mut() else {
        return;
    };
    let title = match state.mode {
        SaveDialogMode::Save => "Save query",
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
    render_dialog_input(frame, chunks[0], "File name", &state.input);
    render_save_dialog_list(frame, chunks[1], state);
    let hint = Paragraph::new("↑/↓ select existing • Enter: Save • Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[2]);
}

fn render_save_dialog_list(frame: &mut Frame, area: Rect, state: &mut SaveDialogState) {
    let list_block = Block::default()
        .title("Existing files")
        .borders(Borders::ALL);
    let inner = list_block.inner(area);
    frame.render_widget(list_block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    if state.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "No saved queries found",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let view_height = inner.height.max(1) as usize;
        let (start, end) = state.visible_bounds(view_height);
        for idx in start..end {
            if let Some(entry) = state.entries.get(idx) {
                let selected = state.selected_index == Some(idx);
                let prefix = if selected { ">" } else { " " };
                let style = if selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(255, 246, 199))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix} {}", entry.display),
                    style,
                )));
            }
        }
    }
    let list = Paragraph::new(lines);
    frame.render_widget(list, inner);
}

fn render_open_dialog(frame: &mut Frame, app: &mut App) {
    let overlay = centered_rect(60, 70, frame.size());
    frame.render_widget(Clear, overlay);
    let Some(state) = app.open_dialog_state_mut() else {
        return;
    };
    let block = Block::default()
        .title("Open query")
        .borders(Borders::ALL)
        .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
    render_dialog_input(frame, chunks[0], "Filter", &state.filter_input);
    let list_area = chunks[1];
    render_open_dialog_list(frame, list_area, state);
    let hint = Paragraph::new("↑/↓ select • Type to filter • Enter: Open • Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[2]);
}

fn render_open_dialog_list(frame: &mut Frame, area: Rect, state: &mut OpenDialogState) {
    let list_block = Block::default()
        .title("Saved queries")
        .borders(Borders::ALL);
    let inner = list_block.inner(area);
    frame.render_widget(list_block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    if state.filtered_indices.is_empty() {
        lines.push(Line::from(Span::styled(
            "No saved queries match the filter",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let view_height = inner.height.max(1) as usize;
        let (start, end) = state.visible_bounds(view_height);
        let selected = state.selected_filtered_index;
        for filtered_idx in start..end {
            let entry_idx = state
                .filtered_indices
                .get(filtered_idx)
                .copied()
                .unwrap_or(0);
            if let Some(entry) = state.entries.get(entry_idx) {
                let prefix = if Some(filtered_idx) == selected {
                    ">"
                } else {
                    " "
                };
                let style = if Some(filtered_idx) == selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(255, 246, 199))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix} {}", entry.display),
                    style,
                )));
            }
        }
    }
    let list = Paragraph::new(lines);
    frame.render_widget(list, inner);
}

fn render_dialog_input(frame: &mut Frame, area: Rect, title: &str, input: &SingleLineInput) {
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    let widget = Paragraph::new(input.value()).block(block.clone());
    frame.render_widget(widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let scroll = input.visual_scroll(width);
    let cursor = input.visual_cursor();
    let visible_col = cursor.saturating_sub(scroll);
    let max_col = width.saturating_sub(1);
    let cursor_col = visible_col.min(max_col);
    let x = inner.x + cursor_col as u16;
    let y = inner.y;
    frame.set_cursor(x, y);
}
