mod app;
mod config;
mod providers;
mod services;
mod types;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{Event, EventStream, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use app::App;
use types::StreamToken;

// ── CLI Arguments ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "skillforge",
    version,
    about = "Generate AI skills for your CLI tools — keyboard-driven TUI"
)]
struct Cli {
    /// Path to a custom config file
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // File-based logging (never to stdout — would corrupt the TUI)
    setup_logging()?;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run the app; always restore terminal on exit
    let result = run_app(&mut terminal, cli.config).await;

    // Terminal teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("skillforge error: {}", e);
    }
    result
}

// ── Event loop ────────────────────────────────────────────────────────────────

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let mut app = App::new(config_path).await?;
    let mut event_stream = EventStream::new();

    // Bounded channel for AI stream tokens (256 provides backpressure)
    let (ai_tx, mut ai_rx) = mpsc::channel::<StreamToken>(256);

    // Channel for model list results: (provider_id, Vec<model_id>)
    let (models_tx, mut models_rx) = mpsc::channel::<(String, Vec<String>)>(16);

    // Initial render
    terminal.draw(|f| ui::render(f, &mut app))?;

    loop {
        tokio::select! {
            // Keyboard / terminal events
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        // Ignore key-release events (Windows fires both press & release)
                        if key.kind == KeyEventKind::Press {
                            if app.handle_key(key, &ai_tx, &models_tx).await? {
                                break;
                            }
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Just re-render on resize — ratatui recalculates layout
                    }
                    Some(Err(e)) => {
                        tracing::error!("Terminal event error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
                terminal.draw(|f| ui::render(f, &mut app))?;
            }

            // AI stream tokens coming from a background task
            maybe_token = ai_rx.recv() => {
                match maybe_token {
                    Some(token) => {
                        app.handle_stream_token(token);
                        terminal.draw(|f| ui::render(f, &mut app))?;
                    }
                    None => {}
                }
            }

            // Model list results from provider API fetch
            maybe_models = models_rx.recv() => {
                if let Some((provider_id, models)) = maybe_models {
                    app.handle_models_loaded(provider_id, models);
                    terminal.draw(|f| ui::render(f, &mut app))?;
                }
            }

            // Periodic tick: clear transient success status messages after ~3 s
            _ = tokio::time::sleep(Duration::from_secs(3)) => {
                if let Some((_, false)) = &app.status_message {
                    if !matches!(app.state, types::AppState::Generating) {
                        app.status_message = None;
                        terminal.draw(|f| ui::render(f, &mut app))?;
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Logging ───────────────────────────────────────────────────────────────────

fn setup_logging() -> Result<()> {
    let log_dir = config::get_config_dir();
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("skillforge.log");

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("skillforge=info".parse()?))
        .with_writer(file)
        .with_ansi(false)
        .init();

    Ok(())
}
