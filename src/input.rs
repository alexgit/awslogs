use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arboard::Clipboard;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;
use tokio::task;
use tui_input::backend::crossterm::EventHandler;
use tui_textarea::Input as TextAreaInput;

use crate::app::{App, FocusField, QueryFileEntry, SaveDialogMode};
use crate::log_fetcher::{LogFetcher, QueryOutcome};

const QUERIES_DIR: &str = "queries";

fn queries_directory() -> Result<PathBuf, String> {
    let cwd =
        env::current_dir().map_err(|err| format!("Unable to resolve working directory: {err}"))?;
    Ok(cwd.join(QUERIES_DIR))
}

pub async fn handle_key_event(
    key: KeyEvent,
    app: &mut App,
    fetcher: &Arc<dyn LogFetcher>,
    tx: &mpsc::UnboundedSender<QueryOutcome>,
) -> Result<bool, Box<dyn Error>> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    let modifiers = key.modifiers;
    let code = key.code;
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let super_mod = modifiers.contains(KeyModifiers::SUPER);

    if app.help_open {
        if (ctrl && matches!(code, KeyCode::Char('h') | KeyCode::Char('H')))
            || matches!(code, KeyCode::Esc)
        {
            app.close_help();
        }
        return Ok(false);
    }

    if app.modal_open
        && (modifiers.is_empty() || modifiers == KeyModifiers::SHIFT)
        && matches!(code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        if let Some(text) = app.selected_row_detail_text() {
            match Clipboard::new() {
                Ok(mut clipboard) => {
                    if let Err(err) = clipboard.set_text(text) {
                        app.set_error(format!("Unable to copy row details: {err}"));
                    } else {
                        app.set_status("Copied row details to clipboard.");
                    }
                }
                Err(err) => {
                    app.set_error(format!("Unable to access clipboard: {err}"));
                }
            }
        } else {
            app.set_status("No row details to copy.");
        }
        return Ok(false);
    }

    if app.save_dialog_active() {
        match code {
            KeyCode::Esc => {
                app.close_save_dialog();
                app.set_status("Save canceled");
            }
            KeyCode::Up => {
                if let Some(state) = app.save_dialog_state_mut() {
                    state.move_selection(-1);
                }
            }
            KeyCode::Down => {
                if let Some(state) = app.save_dialog_state_mut() {
                    state.move_selection(1);
                }
            }
            KeyCode::Enter => {
                if let Err(err) = confirm_save_dialog(app).await {
                    app.set_error(err);
                }
            }
            _ => {
                if let Some(state) = app.save_dialog_state_mut() {
                    let event = Event::Key(key);
                    let _ = state.input.handle_event(&event);
                }
            }
        }
        return Ok(false);
    }

    if app.open_dialog_active() {
        match code {
            KeyCode::Esc => {
                app.close_open_dialog();
                app.set_status("Open canceled");
            }
            KeyCode::Enter => {
                if let Err(err) = confirm_open_dialog(app).await {
                    app.set_error(err);
                }
            }
            KeyCode::Up => {
                if let Some(state) = app.open_dialog_state_mut() {
                    state.move_selection(-1);
                }
            }
            KeyCode::Down => {
                if let Some(state) = app.open_dialog_state_mut() {
                    state.move_selection(1);
                }
            }
            _ => {
                if let Some(state) = app.open_dialog_state_mut() {
                    let event = Event::Key(key);
                    let previous = state.filter_input.value().to_string();
                    let _ = state.filter_input.handle_event(&event);
                    if state.filter_input.value() != previous {
                        state.apply_filter();
                    }
                }
            }
        }
        return Ok(false);
    }

    if app.column_modal_active() {
        match code {
            KeyCode::Esc => {
                app.close_column_modal();
            }
            KeyCode::Enter => {
                app.apply_column_modal();
            }
            KeyCode::Up => {
                app.column_modal_move(-1);
            }
            KeyCode::Down => {
                app.column_modal_move(1);
            }
            KeyCode::Char(' ') => {
                app.column_modal_toggle();
            }
            _ => {}
        }
        return Ok(false);
    }

    if code == KeyCode::Esc {
        if app.modal_open {
            app.close_modal();
            return Ok(false);
        }
        match app.focus {
            FocusField::Filter => {
                app.focus = FocusField::Results;
                return Ok(false);
            }
            FocusField::Results => {
                app.results_navigation = false;
                app.focus = FocusField::Query;
                return Ok(false);
            }
            _ => {}
        }
    }

    if modifiers.is_empty()
        && matches!(code, KeyCode::Char('/'))
        && app.focus == FocusField::Results
        && !app.inputs_collapsed
    {
        app.activate_filter();
        app.focus = FocusField::Filter;
        return Ok(false);
    }

    if (ctrl || super_mod) && matches!(code, KeyCode::Char('s') | KeyCode::Char('S')) {
        match gather_query_file_entries().await {
            Ok(entries) => {
                let prefill = app.saved_query_file_name();
                app.open_save_dialog_with_entries(SaveDialogMode::Save, prefill, entries);
            }
            Err(err) => app.set_error(err),
        }
        return Ok(false);
    }

    if (ctrl || super_mod) && matches!(code, KeyCode::Char('o') | KeyCode::Char('O')) {
        match gather_query_file_entries().await {
            Ok(entries) => {
                if entries.is_empty() {
                    app.set_status("No saved queries available");
                } else {
                    app.open_open_dialog(entries);
                }
            }
            Err(err) => app.set_error(err),
        }
        return Ok(false);
    }

    if app.focus == FocusField::Results && modifiers.is_empty() {
        match code {
            KeyCode::Enter => {
                if app.modal_open {
                    app.close_modal();
                } else if app.results_navigation {
                    app.toggle_modal();
                } else {
                    app.enter_results_navigation();
                }
                return Ok(false);
            }
            KeyCode::Up => {
                app.move_selection(-1);
                return Ok(false);
            }
            KeyCode::Down => {
                app.move_selection(1);
                return Ok(false);
            }
            KeyCode::PageUp => {
                app.page_results(-1);
                return Ok(false);
            }
            KeyCode::PageDown => {
                app.page_results(1);
                return Ok(false);
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                app.open_column_modal();
                return Ok(false);
            }
            KeyCode::Char('x') => {
                if app.results_navigation || app.modal_open {
                    app.exit_results_navigation();
                }
                return Ok(false);
            }
            _ => {}
        }
    }

    if app.focus == FocusField::AwsProfile && modifiers.is_empty() {
        match code {
            KeyCode::Left | KeyCode::Up => {
                app.move_profile_selection(-1);
                return Ok(false);
            }
            KeyCode::Right | KeyCode::Down => {
                app.move_profile_selection(1);
                return Ok(false);
            }
            _ => {}
        }
    }

    if app.focus == FocusField::TimeMode && modifiers.is_empty() {
        match code {
            KeyCode::Enter
            | KeyCode::Char(' ')
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down => {
                app.toggle_relative_mode();
                return Ok(false);
            }
            _ => {}
        }
    }

    if app.focus == FocusField::RelativeRange && modifiers.is_empty() {
        match code {
            KeyCode::Up => {
                app.move_relative_selection(-1);
                return Ok(false);
            }
            KeyCode::Down => {
                app.move_relative_selection(1);
                return Ok(false);
            }
            KeyCode::Enter => {
                start_query_submission(app, fetcher, tx);
                return Ok(false);
            }
            _ => {}
        }
    }

    if !app.relative_mode && modifiers.is_empty() {
        match app.focus {
            FocusField::From => match code {
                KeyCode::Up => {
                    app.adjust_absolute_input(FocusField::From, 1);
                    return Ok(false);
                }
                KeyCode::Down => {
                    app.adjust_absolute_input(FocusField::From, -1);
                    return Ok(false);
                }
                _ => {}
            },
            FocusField::To => match code {
                KeyCode::Up => {
                    app.adjust_absolute_input(FocusField::To, 1);
                    return Ok(false);
                }
                KeyCode::Down => {
                    app.adjust_absolute_input(FocusField::To, -1);
                    return Ok(false);
                }
                _ => {}
            },
            _ => {}
        }
    }

    if is_ctrl_enter(&key) {
        start_query_submission(app, fetcher, tx);
        return Ok(false);
    }

    if ctrl {
        if matches!(code, KeyCode::Char('h') | KeyCode::Char('H')) {
            app.toggle_help();
            return Ok(false);
        }
        match code {
            KeyCode::Up => {
                app.collapse_inputs();
                return Ok(false);
            }
            KeyCode::Down => {
                app.expand_inputs();
                return Ok(false);
            }
            _ => {}
        }
        match code {
            KeyCode::Char('c') => return Ok(true),
            KeyCode::Char('r') => start_query_submission(app, fetcher, tx),
            _ => {}
        }
        return Ok(false);
    }

    match code {
        KeyCode::Tab => {
            app.next_focus();
            return Ok(false);
        }
        KeyCode::BackTab => {
            app.prev_focus();
            return Ok(false);
        }
        KeyCode::Char('q') | KeyCode::Char('Q')
            if (modifiers.is_empty() || modifiers == KeyModifiers::SHIFT)
                && !focus_accepts_text_input(app.focus) =>
        {
            if app.focus != FocusField::Query {
                app.focus = FocusField::Query;
            }
            return Ok(false);
        }
        KeyCode::Char('r') | KeyCode::Char('R')
            if (modifiers.is_empty() || modifiers == KeyModifiers::SHIFT)
                && !focus_accepts_text_input(app.focus) =>
        {
            if app.focus != FocusField::Results {
                app.focus = FocusField::Results;
                app.results_navigation = false;
            }
            return Ok(false);
        }
        KeyCode::Char('t') | KeyCode::Char('T')
            if (modifiers.is_empty() || modifiers == KeyModifiers::SHIFT)
                && !focus_accepts_text_input(app.focus) =>
        {
            if app.focus != FocusField::RelativeRange {
                app.focus = FocusField::RelativeRange;
            }
            return Ok(false);
        }
        KeyCode::F(5) => {
            start_query_submission(app, fetcher, tx);
            return Ok(false);
        }
        KeyCode::Enter
            if matches!(
                app.focus,
                FocusField::AwsRegion | FocusField::From | FocusField::To | FocusField::LogGroup
            ) =>
        {
            start_query_submission(app, fetcher, tx);
            return Ok(false);
        }
        _ => {}
    }

    let event = Event::Key(key);

    match app.focus {
        FocusField::From => {
            let _ = app.from_input.handle_event(&event);
        }
        FocusField::To => {
            let _ = app.to_input.handle_event(&event);
        }
        FocusField::LogGroup => {
            let _ = app.log_group_input.handle_event(&event);
        }
        FocusField::AwsRegion => {
            let _ = app.aws_region_input.handle_event(&event);
        }
        FocusField::Query => {
            let input = TextAreaInput::from(event.clone());
            app.query_area.input(input);
        }
        FocusField::Results => {}
        FocusField::Filter => {
            let previous = app.filter_input.value().to_string();
            let _ = app.filter_input.handle_event(&event);
            if app.filter_input.value() != previous {
                app.schedule_filter_update();
            }
            if matches!(code, KeyCode::Enter) {
                app.focus = FocusField::Results;
            }
        }
        FocusField::AwsProfile => {}
        FocusField::TimeMode => {}
        FocusField::RelativeRange => {}
    }

    Ok(false)
}

fn focus_accepts_text_input(focus: FocusField) -> bool {
    matches!(
        focus,
        FocusField::Query
            | FocusField::From
            | FocusField::To
            | FocusField::LogGroup
            | FocusField::AwsRegion
            | FocusField::Filter
    )
}

async fn confirm_save_dialog(app: &mut App) -> Result<(), String> {
    let filename = if let Some(state) = app.save_dialog_state_mut() {
        state.input.value().to_string()
    } else {
        return Ok(());
    };
    if filename.is_empty() {
        app.set_status("Please enter a file name");
        return Ok(());
    }
    let destination = queries_directory()?.join(filename);
    save_query_to_path(app, destination).await?;
    app.close_save_dialog();
    Ok(())
}

async fn confirm_open_dialog(app: &mut App) -> Result<(), String> {
    let Some(path) = app.open_dialog_selected_path() else {
        app.set_status("No matching queries to open");
        return Ok(());
    };
    load_query_from_path(app, path).await?;
    app.close_open_dialog();
    Ok(())
}

async fn save_query_to_path(app: &mut App, destination: PathBuf) -> Result<(), String> {
    let contents = app.query_text();
    if contents.trim().is_empty() {
        app.set_status("Current query is empty; nothing to save");
        return Ok(());
    }
    let queries_dir = queries_directory()?;
    let path = destination.clone();
    let payload = contents;
    task::spawn_blocking(move || -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Unable to prepare save directory: {err}"))?;
        }
        fs::write(&path, payload).map_err(|err| format!("Failed to write file: {err}"))
    })
    .await
    .map_err(|err| format!("Save operation interrupted: {err}"))??;
    let display = format_query_display(&destination, &queries_dir);
    app.set_saved_query_path(destination);
    app.set_status(format!("Saved query to {display}"));
    Ok(())
}

async fn load_query_from_path(app: &mut App, path: PathBuf) -> Result<(), String> {
    let queries_dir = queries_directory()?;
    let target = path.clone();
    let contents = task::spawn_blocking(move || -> Result<String, String> {
        fs::read_to_string(&target).map_err(|err| format!("Failed to read file: {err}"))
    })
    .await
    .map_err(|err| format!("Load operation interrupted: {err}"))??;
    app.replace_query_text(contents);
    if app.inputs_collapsed {
        app.expand_inputs();
    }
    app.focus = FocusField::Query;
    app.set_saved_query_path(path.clone());
    let display = format_query_display(&path, &queries_dir);
    app.set_status(format!("Loaded query from {display}"));
    Ok(())
}

async fn gather_query_file_entries() -> Result<Vec<QueryFileEntry>, String> {
    let queries_dir = queries_directory()?;
    let entries = {
        let queries_dir = queries_dir.clone();
        task::spawn_blocking(move || -> Result<Vec<QueryFileEntry>, String> {
            fs::create_dir_all(&queries_dir)
                .map_err(|err| format!("Unable to prepare {QUERIES_DIR} directory: {err}"))?;
            let mut list = Vec::new();
            for entry in fs::read_dir(&queries_dir)
                .map_err(|err| format!("Unable to read {QUERIES_DIR}: {err}"))?
            {
                let entry = entry.map_err(|err| format!("Failed to read entry: {err}"))?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let display = entry
                    .file_name()
                    .to_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let searchable = display.to_ascii_lowercase();
                list.push(QueryFileEntry {
                    display,
                    path,
                    searchable,
                });
            }
            list.sort_by(|a, b| a.searchable.cmp(&b.searchable));
            Ok(list)
        })
    }
    .await
    .map_err(|err| format!("Listing queries interrupted: {err}"))??;
    Ok(entries)
}

fn format_query_display(path: &Path, base: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(base) {
        format!("{QUERIES_DIR}/{}", relative.display())
    } else {
        path.display().to_string()
    }
}

pub(crate) fn start_query_submission(
    app: &mut App,
    fetcher: &Arc<dyn LogFetcher>,
    tx: &mpsc::UnboundedSender<QueryOutcome>,
) {
    if app.submitting {
        app.set_status("Query already in progress");
        return;
    }

    match app.prepare_submission() {
        Ok(params) => {
            app.submitting = true;
            app.set_status("Running query...");
            app.clear_results();
            let fetcher = Arc::clone(fetcher);
            let tx = tx.clone();
            tokio::spawn(async move {
                let outcome = fetcher.run_query(params).await;
                let _ = tx.send(outcome);
            });
        }
        Err(err) => {
            app.set_error(err);
        }
    }
}

pub(crate) fn is_ctrl_enter(key: &KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    let mods = key.modifiers;
    if !(mods.contains(KeyModifiers::CONTROL)
        || mods.contains(KeyModifiers::SUPER)
        || mods.contains(KeyModifiers::ALT))
    {
        return false;
    }
    match key.code {
        KeyCode::Enter => true,
        KeyCode::Char(c) => matches!(c, 'm' | 'M' | 'j' | 'J' | '\r' | '\n'),
        _ => false,
    }
}
