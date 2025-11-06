use aws_config::BehaviorVersion;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{execute, terminal};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::env;
use std::error::Error;
use std::io;
use std::sync::Arc;
mod app;
mod aws_profiles;
mod defaults;
mod help;
mod input;
mod log_fetcher;
mod presentation;
mod tui;
mod ui;
mod widgets;
use log_fetcher::{AwsLogFetcher, FakeLogFetcher, LogFetcher};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let use_fake = args.iter().any(|arg| arg == "--fake" || arg == "-f");
    let (fetcher, status_override): (Arc<dyn LogFetcher>, Option<String>) = if use_fake {
        (
            Arc::new(FakeLogFetcher::new()),
            Some("Using built-in fake data. Press Ctrl+Enter to load synthetic logs.".into()),
        )
    } else {
        (
            Arc::new(AwsLogFetcher::new(BehaviorVersion::latest())),
            None,
        )
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = tui::run_app(fetcher, status_override, &mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture,
        terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    app_result
}
