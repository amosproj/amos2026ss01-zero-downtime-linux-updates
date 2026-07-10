mod app;
mod client;
mod config;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use client::{Client, Config, Msg};
use config::Cli;
use futures::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{Event as CEvent, EventStream, KeyEventKind};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config {
        base_url: cli.base_url.clone(),
        jwt: cli.jwt.clone(),
    };
    if cfg.jwt.is_empty() {
        eprintln!(
            "warning: no JWT provided (set --jwt or AMOS_JWT); user API calls will fail with 401"
        );
    }
    let client = Client::new(cfg);

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, client, cli).await;
    ratatui::restore();
    result
}

async fn run(terminal: &mut DefaultTerminal, client: Client, cli: Cli) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<Msg>(1024);

    let mut app = App::new(client.clone(), tx, cli.level.to_level(), cli.max_logs);
    app.devices = client.fetch_devices().await.unwrap_or_default();
    if let Some(id) = cli.device
        && let Some(pos) = app.devices.iter().position(|d| d.id == id)
    {
        app.selected = pos + 1;
    }
    app.reconnect(); // open the initial stream

    let mut term_events = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        tokio::select! {
            maybe_msg = rx.recv() => {
                if let Some(msg) = maybe_msg {
                    app.on_msg(msg);
                    // Drain a burst without redrawing between each.
                    while let Ok(msg) = rx.try_recv() {
                        app.on_msg(msg);
                    }
                }
            }
            maybe_event = term_events.next() => {
                if let Some(Ok(CEvent::Key(k))) = maybe_event
                    && k.kind == KeyEventKind::Press
                {
                    app.on_key(k.code, k.modifiers);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
