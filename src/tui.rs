use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::app::App;
use crate::input;
use crate::log_fetcher::{LogFetcher, QueryOutcome};
use crate::presentation::format_results;
use crate::ui;

pub async fn run_app(
    fetcher: Arc<dyn LogFetcher>,
    initial_status: Option<String>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let mut app = App::default();
    if let Some(status) = initial_status {
        app.set_status(status);
    }
    let mut events = EventStream::new();
    let mut ticker = interval(Duration::from_millis(100));
    let (tx, mut rx) = mpsc::unbounded_channel::<QueryOutcome>();

    loop {
        terminal.draw(|f| ui::draw_ui(f, &mut app))?;

        tokio::select! {
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if input::is_ctrl_enter(&key) {
                            input::start_query_submission(&mut app, &fetcher, &tx);
                            continue;
                        } else if input::handle_key_event(key, &mut app, &fetcher, &tx).await? {
                            break;
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    Some(Err(err)) => {
                        app.set_error(format!("Event error: {err}"));
                    }
                    _ => {}
                }
            }
            Some(outcome) = rx.recv() => {
                app.submitting = false;
                match outcome {
                    QueryOutcome::Success(data) => {
                        app.set_status("Query complete");
                        let formatted = format_results(&data);
                        app.set_results(formatted);
                    }
                    QueryOutcome::Error(err) => {
                        app.set_error(err);
                    }
                }
            }
            _ = ticker.tick() => {
                app.on_tick();
            }
        }
    }

    Ok(())
}
