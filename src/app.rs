use std::env;
use std::fmt::Write;
use std::time::{Duration, Instant};

use chrono::Duration as ChronoDuration;
use chrono::{DateTime, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use tui_input::Input as SingleLineInput;
use tui_textarea::TextArea;

use crate::aws_profiles;
use crate::defaults::{default_app_values, AppDefaults};
use crate::log_fetcher::QueryParams;
use crate::presentation::{format_modal_message, format_modal_value, FormattedResults};
use crate::widgets::column_picker::ColumnPickerState;

pub const FILTER_DEBOUNCE_MS: u64 = 80;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusField {
    AwsRegion,
    AwsProfile,
    TimeMode,
    RelativeRange,
    From,
    To,
    LogGroup,
    Query,
    Results,
    Filter,
}

pub struct ResultRow {
    pub cells: Vec<String>,
    pub searchable: String,
}

impl ResultRow {
    fn new(cells: Vec<String>) -> Self {
        let searchable = cells.join(" ").to_ascii_lowercase();
        Self { cells, searchable }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
}

#[derive(Default)]
pub struct QueryResults {
    pub headers: Vec<String>,
    pub rows: Vec<ResultRow>,
}

fn resolve_default_region() -> String {
    fn env_region(key: &str) -> Option<String> {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    env_region("AWS_REGION")
        .or_else(|| env_region("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|| "eu-west-1".to_string())
}

pub struct RelativeRangeOption {
    pub label: &'static str,
    pub seconds: i64,
}

const fn minutes(value: i64) -> i64 {
    value * 60
}

const fn hours(value: i64) -> i64 {
    minutes(value * 60)
}

const fn days(value: i64) -> i64 {
    hours(value * 24)
}

pub const RELATIVE_RANGE_OPTIONS: [RelativeRangeOption; 17] = [
    RelativeRangeOption {
        label: "1 minute",
        seconds: minutes(1),
    },
    RelativeRangeOption {
        label: "5 minutes",
        seconds: minutes(5),
    },
    RelativeRangeOption {
        label: "10 minutes",
        seconds: minutes(10),
    },
    RelativeRangeOption {
        label: "15 minutes",
        seconds: minutes(15),
    },
    RelativeRangeOption {
        label: "30 minutes",
        seconds: minutes(30),
    },
    RelativeRangeOption {
        label: "1 hour",
        seconds: hours(1),
    },
    RelativeRangeOption {
        label: "2 hours",
        seconds: hours(2),
    },
    RelativeRangeOption {
        label: "3 hours",
        seconds: hours(3),
    },
    RelativeRangeOption {
        label: "5 hours",
        seconds: hours(5),
    },
    RelativeRangeOption {
        label: "12 hours",
        seconds: hours(12),
    },
    RelativeRangeOption {
        label: "1 day",
        seconds: days(1),
    },
    RelativeRangeOption {
        label: "2 days",
        seconds: days(2),
    },
    RelativeRangeOption {
        label: "3 days",
        seconds: days(3),
    },
    RelativeRangeOption {
        label: "5 days",
        seconds: days(5),
    },
    RelativeRangeOption {
        label: "7 days",
        seconds: days(7),
    },
    RelativeRangeOption {
        label: "14 days",
        seconds: days(14),
    },
    RelativeRangeOption {
        label: "30 days",
        seconds: days(30),
    },
];

pub struct App {
    pub focus: FocusField,
    pub aws_profiles: Vec<String>,
    pub selected_profile_index: Option<usize>,
    pub aws_region_input: SingleLineInput,
    pub inputs_collapsed: bool,
    pub relative_mode: bool,
    pub selected_relative_index: usize,
    pub from_input: SingleLineInput,
    pub to_input: SingleLineInput,
    pub log_group_input: SingleLineInput,
    pub query_area: TextArea<'static>,
    pub query_scroll_row: u16,
    pub query_scroll_col: u16,
    pub results: QueryResults,
    pub column_visibility: Vec<bool>,
    pub results_initialized: bool,
    pub status_kind: StatusKind,
    pub filtered_indices: Vec<usize>,
    pub filter_input: SingleLineInput,
    pub filter_active: bool,
    pub filter_dirty: bool,
    pub last_filter_edit: Option<Instant>,
    pub status: String,
    pub results_navigation: bool,
    pub selected_filtered_index: Option<usize>,
    pub modal_open: bool,
    pub help_open: bool,
    pub results_scroll: usize,
    pub results_view_height: usize,
    pub submitting: bool,
    pub column_modal: Option<ColumnPickerState>,
}

impl App {
    pub fn next_focus(&mut self) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        if let Some(idx) = order.iter().position(|field| *field == self.focus) {
            let next = (idx + 1) % order.len();
            self.focus = order[next];
        } else {
            self.focus = order[0];
        }
    }

    pub fn prev_focus(&mut self) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        if let Some(idx) = order.iter().position(|field| *field == self.focus) {
            let prev = idx.checked_sub(1).unwrap_or(order.len() - 1);
            self.focus = order[prev];
        } else {
            self.focus = order[0];
        }
    }

    fn focus_order(&self) -> Vec<FocusField> {
        let mut order = Vec::new();
        if !self.inputs_collapsed {
            order.push(FocusField::AwsRegion);
            if self.show_profile_picker() {
                order.push(FocusField::AwsProfile);
            }
            order.push(FocusField::TimeMode);
            if self.relative_mode {
                order.push(FocusField::RelativeRange);
            } else {
                order.push(FocusField::From);
                order.push(FocusField::To);
            }
            order.push(FocusField::LogGroup);
            order.push(FocusField::Query);
        }
        order.push(FocusField::Results);
        if self.filter_active && !self.inputs_collapsed {
            order.push(FocusField::Filter);
        }
        order
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_kind = StatusKind::Info;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_kind = StatusKind::Error;
    }

    pub fn query_text(&self) -> String {
        self.query_area.lines().join("\n")
    }

    pub fn replace_query_text(&mut self, text: String) {
        self.query_area = TextArea::from(text.lines().map(|line| line.to_string()));
        self.query_scroll_row = 0;
        self.query_scroll_col = 0;
    }

    pub fn show_profile_picker(&self) -> bool {
        !self.aws_profiles.is_empty()
    }

    pub fn selected_profile_name(&self) -> Option<&str> {
        self.selected_profile_index
            .and_then(|idx| self.aws_profiles.get(idx))
            .map(|s| s.as_str())
    }

    pub fn move_profile_selection(&mut self, delta: i32) {
        if !self.show_profile_picker() {
            return;
        }
        let len = self.aws_profiles.len() as i32;
        if len == 0 {
            return;
        }
        let current = self.selected_profile_index.unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len - 1);
        self.selected_profile_index = Some(next as usize);
    }

    pub fn relative_options(&self) -> &'static [RelativeRangeOption] {
        &RELATIVE_RANGE_OPTIONS
    }

    pub fn current_relative_option(&self) -> &'static RelativeRangeOption {
        let options = self.relative_options();
        if options.is_empty() {
            panic!("relative options list is unexpectedly empty");
        }
        let idx = self
            .selected_relative_index
            .min(options.len().saturating_sub(1));
        &options[idx]
    }

    pub fn move_relative_selection(&mut self, delta: i32) {
        let options = self.relative_options();
        if options.is_empty() {
            return;
        }
        let len = options.len() as i32;
        let current = self.selected_relative_index as i32;
        let next = (current + delta).clamp(0, len - 1);
        self.selected_relative_index = next as usize;
    }

    pub fn toggle_relative_mode(&mut self) {
        let new_value = !self.relative_mode;
        self.set_relative_mode(new_value);
    }

    pub fn set_relative_mode(&mut self, enabled: bool) {
        if self.relative_mode == enabled {
            return;
        }
        self.relative_mode = enabled;
        let max_index = self.relative_options().len().saturating_sub(1);
        self.selected_relative_index = self.selected_relative_index.min(max_index);
        if enabled {
            if !self.inputs_collapsed {
                self.focus = FocusField::RelativeRange;
            }
        } else {
            self.refresh_absolute_range();
            if !self.inputs_collapsed {
                self.focus = FocusField::From;
            }
        }
    }

    fn refresh_absolute_range(&mut self) {
        let now = Local::now();
        let start = now - ChronoDuration::days(1);
        let from = start.format("%Y-%m-%d %H:%M:%S").to_string();
        let to = now.format("%Y-%m-%d %H:%M:%S").to_string();
        self.from_input = SingleLineInput::new(from);
        self.to_input = SingleLineInput::new(to);
    }

    pub fn set_results(&mut self, data: FormattedResults) {
        self.results_navigation = false;
        self.selected_filtered_index = None;
        self.modal_open = false;
        self.column_modal = None;
        self.results.headers = data.headers;
        self.results.rows = data.rows.into_iter().map(ResultRow::new).collect();
        self.column_visibility = vec![true; self.results.headers.len()];
        self.results_initialized = true;
        self.apply_filter_now();
        if !self.results.rows.is_empty() {
            self.focus = FocusField::Results;
            self.enter_results_navigation();
        }
    }

    pub fn clear_results(&mut self) {
        self.results = QueryResults::default();
        self.filtered_indices.clear();
        self.results_navigation = false;
        self.selected_filtered_index = None;
        self.modal_open = false;
        self.column_modal = None;
        self.results_scroll = 0;
        self.results_view_height = self.results_view_height.max(1);
        self.results_initialized = false;
        self.column_visibility.clear();
    }

    pub fn activate_filter(&mut self) {
        if !self.filter_active {
            self.filter_active = true;
        }
        self.apply_filter_now();
    }

    pub fn schedule_filter_update(&mut self) {
        self.filter_dirty = true;
        self.last_filter_edit = Some(Instant::now());
    }

    pub fn apply_filter_now(&mut self) {
        self.filter_dirty = false;
        let total_rows = self.results.rows.len();
        if total_rows == 0 {
            self.filtered_indices.clear();
            self.exit_results_navigation();
            return;
        }

        let raw_filter = self.filter_input.value();
        let mut include_tokens: Vec<String> = Vec::new();
        let mut exclude_tokens: Vec<String> = Vec::new();

        for token in raw_filter.split_whitespace() {
            if let Some(rest) = token.strip_prefix('+') {
                let normalized = rest.trim();
                if !normalized.is_empty() {
                    include_tokens.push(normalized.to_ascii_lowercase());
                }
            } else if let Some(rest) = token.strip_prefix('-') {
                let normalized = rest.trim();
                if !normalized.is_empty() {
                    exclude_tokens.push(normalized.to_ascii_lowercase());
                }
            } else {
                let normalized = token.trim();
                if !normalized.is_empty() {
                    include_tokens.push(normalized.to_ascii_lowercase());
                }
            }
        }

        if include_tokens.is_empty() && exclude_tokens.is_empty() {
            self.filtered_indices = (0..total_rows).collect();
        } else {
            self.filtered_indices = self
                .results
                .rows
                .iter()
                .enumerate()
                .filter_map(|(idx, row)| {
                    let haystack = &row.searchable;
                    if exclude_tokens.iter().any(|token| haystack.contains(token)) {
                        return None;
                    }
                    if include_tokens.is_empty()
                        || include_tokens.iter().any(|token| haystack.contains(token))
                    {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
        }

        self.sync_selection_after_filter();
    }

    pub fn on_tick(&mut self) {
        if self.filter_dirty {
            let ready = self
                .last_filter_edit
                .map(|instant| instant.elapsed() >= Duration::from_millis(FILTER_DEBOUNCE_MS))
                .unwrap_or(true);
            if ready {
                self.apply_filter_now();
            }
        }
    }

    fn sync_selection_after_filter(&mut self) {
        let count = self.filtered_indices.len();
        if count == 0 {
            self.selected_filtered_index = None;
            self.modal_open = false;
            if self.results_navigation {
                self.results_navigation = false;
            }
            self.results_scroll = 0;
        } else {
            if let Some(idx) = self.selected_filtered_index {
                if idx >= count {
                    self.selected_filtered_index = Some(count - 1);
                }
            } else if self.results_navigation {
                self.selected_filtered_index = Some(0);
            }
        }
        self.ensure_selection_visible();
    }

    pub fn enter_results_navigation(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.results_navigation = true;
        if self
            .selected_filtered_index
            .filter(|&idx| idx < self.filtered_indices.len())
            .is_none()
        {
            self.selected_filtered_index = Some(0);
        }
        self.modal_open = false;
        self.column_modal = None;
        self.ensure_selection_visible();
    }

    pub fn exit_results_navigation(&mut self) {
        self.results_navigation = false;
        self.selected_filtered_index = None;
        self.modal_open = false;
        self.ensure_selection_visible();
    }

    pub fn move_selection(&mut self, delta: i32) {
        if !self.results_navigation || self.filtered_indices.is_empty() {
            return;
        }

        self.modal_open = false;
        let current = self.selected_filtered_index.unwrap_or(0) as i32;
        let len = self.filtered_indices.len() as i32;
        let mut next = current + delta;
        if next < 0 {
            next = 0;
        } else if next >= len {
            next = len - 1;
        }

        if current != next {
            self.selected_filtered_index = Some(next as usize);
        } else if self.selected_filtered_index.is_none() {
            self.selected_filtered_index = Some(0);
        }
        self.ensure_selection_visible();
    }

    pub fn toggle_modal(&mut self) {
        if !self.results_navigation {
            return;
        }
        if self.modal_open {
            self.modal_open = false;
        } else if self.selected_row_data().is_some() {
            self.modal_open = true;
        }
    }

    pub fn close_modal(&mut self) {
        self.modal_open = false;
    }

    pub fn page_results(&mut self, delta_pages: i32) {
        if delta_pages == 0 || self.filtered_indices.is_empty() {
            return;
        }

        let view = self.results_view_height.max(1);
        if self.results_navigation {
            let step = view as i32 * delta_pages;
            if step != 0 {
                self.move_selection(step);
            }
            return;
        }

        let len = self.filtered_indices.len();
        if len <= view {
            self.results_scroll = 0;
            return;
        }

        let max_scroll = (len - view) as i32;
        let current = self.results_scroll as i32;
        let mut next = current + view as i32 * delta_pages;
        if next < 0 {
            next = 0;
        } else if next > max_scroll {
            next = max_scroll;
        }
        self.results_scroll = next as usize;
        self.clamp_results_scroll();
    }

    pub fn selected_row_data(&self) -> Option<Vec<(String, String)>> {
        let filtered_pos = self.selected_filtered_index?;
        let row_idx = *self.filtered_indices.get(filtered_pos)?;
        let row = self.results.rows.get(row_idx)?;

        let mut data = Vec::new();
        for (i, cell) in row.cells.iter().enumerate() {
            let header = self
                .results
                .headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Column {}", i + 1));
            data.push((header, cell.clone()));
        }

        Some(data)
    }

    pub fn selected_row_detail_text(&self) -> Option<String> {
        let details = self.selected_row_data()?;
        let mut output = String::new();
        for (idx, (header, value)) in details.iter().enumerate() {
            if idx > 0 {
                output.push('\n');
            }
            let _ = writeln!(&mut output, "{header}:");
            let rendered = if header == "@message" {
                format_modal_message(value)
            } else {
                format_modal_value(value)
            };
            if rendered.is_empty() {
                let _ = writeln!(&mut output, " <empty>");
            } else {
                for line in rendered {
                    let _ = writeln!(&mut output, " {line}");
                }
            }
        }
        if output.is_empty() {
            None
        } else {
            Some(output.trim_end().to_string())
        }
    }

    pub fn update_results_view_height(&mut self, height: usize) {
        let new_height = height.max(1);
        if self.results_view_height != new_height {
            self.results_view_height = new_height;
            self.ensure_selection_visible();
        } else {
            self.clamp_results_scroll();
        }
    }

    fn clamp_results_scroll(&mut self) {
        let len = self.filtered_indices.len();
        let view = self.results_view_height.max(1);
        if len == 0 || len <= view {
            self.results_scroll = 0;
            return;
        }
        let max_scroll = len - view;
        if self.results_scroll > max_scroll {
            self.results_scroll = max_scroll;
        }
    }

    fn ensure_selection_visible(&mut self) {
        self.clamp_results_scroll();
        if let Some(selected) = self.selected_filtered_index {
            if selected < self.results_scroll {
                self.results_scroll = selected;
            } else {
                let view = self.results_view_height.max(1);
                let bottom = self.results_scroll + view - 1;
                if selected > bottom {
                    let new_scroll = selected.saturating_add(1).saturating_sub(view);
                    self.results_scroll = new_scroll;
                }
            }
        } else if !self.results_navigation {
            self.results_scroll = 0;
        }
        self.clamp_results_scroll();
    }

    pub fn prepare_submission(&self) -> Result<QueryParams, String> {
        let log_group = self.log_group_input.value().trim().to_string();
        if log_group.is_empty() {
            return Err("Log group is required".into());
        }

        let region = self.aws_region_input.value().trim().to_string();
        if region.is_empty() {
            return Err("AWS region is required".into());
        }

        let query = self.query_area.lines().join("\n").trim().to_string();
        if query.is_empty() {
            return Err("Query text cannot be empty".into());
        }

        if self.relative_mode {
            let option = self.current_relative_option();
            if option.seconds <= 0 {
                return Err("Relative range must be greater than zero".into());
            }
            let end = Utc::now();
            let start = end - ChronoDuration::seconds(option.seconds);
            return Ok(QueryParams {
                start_epoch: start.timestamp(),
                end_epoch: end.timestamp(),
                log_group,
                query,
                region,
                profile: self.selected_profile_name().map(|s| s.to_string()),
            });
        }

        let start = parse_datetime(self.from_input.value())?;
        let end = parse_datetime(self.to_input.value())?;

        if end <= start {
            return Err("End time must be after start time".into());
        }

        Ok(QueryParams {
            start_epoch: start.timestamp(),
            end_epoch: end.timestamp(),
            log_group,
            query,
            region,
            profile: self.selected_profile_name().map(|s| s.to_string()),
        })
    }

    pub fn collapse_inputs(&mut self) {
        if self.inputs_collapsed {
            return;
        }
        self.inputs_collapsed = true;
        if self.focus != FocusField::Results {
            self.focus = FocusField::Results;
        }
    }

    pub fn expand_inputs(&mut self) {
        if !self.inputs_collapsed {
            return;
        }
        self.inputs_collapsed = false;
        if self.focus == FocusField::Results {
            if self.relative_mode {
                self.focus = FocusField::TimeMode;
            } else {
                self.focus = FocusField::From;
            }
        }
    }

    pub fn toggle_help(&mut self) {
        if self.help_open {
            self.help_open = false;
        } else {
            self.help_open = true;
            self.modal_open = false;
            self.column_modal = None;
        }
    }

    pub fn close_help(&mut self) {
        self.help_open = false;
    }
}

impl Default for App {
    fn default() -> Self {
        let AppDefaults {
            from,
            to,
            log_group,
            query,
        } = default_app_values();
        let aws_profiles = aws_profiles::discover_profiles();
        let mut selected_profile_index = None;
        if !aws_profiles.is_empty() {
            if let Ok(env_profile) = env::var("AWS_PROFILE") {
                let trimmed = env_profile.trim();
                if !trimmed.is_empty() {
                    if let Some(pos) = aws_profiles.iter().position(|p| p == trimmed) {
                        selected_profile_index = Some(pos);
                    }
                }
            }
            if selected_profile_index.is_none() {
                if let Some(pos) = aws_profiles.iter().position(|p| p == "default") {
                    selected_profile_index = Some(pos);
                } else {
                    selected_profile_index = Some(0);
                }
            }
        }
        let from_input = SingleLineInput::new(from);
        let to_input = SingleLineInput::new(to);
        let log_group_input = SingleLineInput::new(log_group.to_string());
        let query_area = TextArea::from(query.lines().map(|line| line.to_string()));
        let initial_status =
            "Ready. Fill in the fields and press Ctrl+Enter to search.".to_string();
        let default_relative_index = RELATIVE_RANGE_OPTIONS
            .iter()
            .position(|opt| opt.label == "1 hour")
            .unwrap_or(0);
        Self {
            focus: FocusField::LogGroup,
            aws_profiles,
            selected_profile_index,
            aws_region_input: SingleLineInput::new(resolve_default_region()),
            inputs_collapsed: false,
            relative_mode: true,
            selected_relative_index: default_relative_index,
            from_input,
            to_input,
            log_group_input,
            query_area,
            query_scroll_row: 0,
            query_scroll_col: 0,
            results: QueryResults::default(),
            column_visibility: Vec::new(),
            results_initialized: false,
            status_kind: StatusKind::Info,
            filtered_indices: Vec::new(),
            filter_input: SingleLineInput::new(String::new()),
            filter_active: false,
            filter_dirty: false,
            last_filter_edit: None,
            status: initial_status,
            results_navigation: false,
            selected_filtered_index: None,
            modal_open: false,
            help_open: false,
            results_scroll: 0,
            results_view_height: 0,
            submitting: false,
            column_modal: None,
        }
    }
}

impl App {
    pub fn ensure_column_visibility_len(&mut self) {
        let expected = self.results.headers.len();
        if self.column_visibility.len() != expected {
            self.column_visibility = vec![true; expected];
        }
    }

    pub fn visible_column_indices(&self) -> Vec<usize> {
        if self.results.headers.is_empty() {
            return Vec::new();
        }
        let mut indices: Vec<usize> = self
            .column_visibility
            .iter()
            .enumerate()
            .filter_map(|(idx, visible)| visible.then_some(idx))
            .collect();
        if indices.is_empty() {
            indices.push(0);
        }
        indices
    }

    pub fn open_column_modal(&mut self) {
        if self.results.headers.is_empty() {
            return;
        }
        self.ensure_column_visibility_len();
        let state = ColumnPickerState::new(self.column_visibility.clone());
        self.column_modal = Some(state);
        self.modal_open = false;
    }

    pub fn close_column_modal(&mut self) {
        self.column_modal = None;
    }

    pub fn column_modal_active(&self) -> bool {
        self.column_modal.is_some()
    }

    pub fn apply_column_modal(&mut self) {
        if let Some(state) = self.column_modal.take() {
            self.column_visibility = state.into_selections();
        }
    }

    pub fn column_modal_move(&mut self, delta: i32) {
        if let Some(state) = self.column_modal.as_mut() {
            state.move_selection(delta);
        }
    }

    pub fn column_modal_toggle(&mut self) {
        if let Some(state) = self.column_modal.as_mut() {
            state.toggle_selected();
        }
    }

    pub fn column_modal_state_mut(&mut self) -> Option<&mut ColumnPickerState> {
        self.column_modal.as_mut()
    }

    pub fn adjust_absolute_input(&mut self, field: FocusField, delta_seconds: i64) {
        if delta_seconds == 0 || self.relative_mode {
            return;
        }
        let target = match field {
            FocusField::From => &mut self.from_input,
            FocusField::To => &mut self.to_input,
            _ => return,
        };
        let original = target.value().to_string();
        if original.trim().is_empty() {
            return;
        }
        if let Ok(datetime_utc) = parse_datetime(&original) {
            let adjusted = datetime_utc + ChronoDuration::seconds(delta_seconds);
            let local_dt = adjusted.with_timezone(&Local);
            let formatted = local_dt.format("%Y-%m-%d %H:%M:%S").to_string();
            *target = SingleLineInput::new(formatted);
        }
    }
}

pub fn parse_datetime(input: &str) -> Result<DateTime<Utc>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Time value is required".into());
    }

    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M"))
        .or_else(|_| {
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map(|date| date.and_time(NaiveTime::MIN))
        })
        .map_err(|_| "Use YYYY-MM-DD[ HH:MM[:SS]] format".to_string())?;

    match Local.from_local_datetime(&naive) {
        LocalResult::Single(local_dt) => Ok(local_dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, _) => {
            Err("Ambiguous local time; specify a different value".into())
        }
        LocalResult::None => Err("Invalid local time".into()),
    }
}
