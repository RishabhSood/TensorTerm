#[macro_use]
mod logger;
mod app;
mod config;
mod event;
mod llm;
mod network;
mod obsidian;
mod providers;
mod scaffold_index;
mod ui;
mod vault;

use std::io;
use std::sync::Arc;

use clap::Parser;
use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use config::Config;
use event::{Event, EventHandler};

#[derive(Parser)]
#[command(name = "tensorterm", version, about = "Cyberpunk research intelligence terminal dashboard")]
struct Cli {
    /// Print the config file path and exit
    #[arg(long)]
    config_path: bool,

    /// Open the config file in $EDITOR and exit
    #[arg(long)]
    edit_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.config_path {
        println!("{}", Config::config_path().display());
        return Ok(());
    }

    if cli.edit_config {
        // Ensure the config file exists with defaults
        let _ = Config::load();
        let path = Config::config_path();
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => std::process::exit(s.code().unwrap_or(1)),
            Err(e) => {
                eprintln!("Failed to open editor '{}': {}", editor, e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    install_hooks()?;
    let config = Config::load()?;
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, config);
    restore_terminal(&mut terminal)?;
    result
}

fn install_hooks() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();
    let panic_hook = panic_hook.into_panic_hook();
    eyre_hook.install()?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        panic_hook(info);
    }));
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, config: Config) -> Result<()> {
    let (net_action_tx, net_action_rx) = tokio::sync::mpsc::channel(32);
    let (net_event_tx, net_event_rx) = tokio::sync::mpsc::unbounded_channel();

    // Build LLM providers from config
    let llm_providers = llm::build_providers(&config.llm);
    let providers_arc = Arc::new(llm_providers);

    debug_log!("Starting TensorTerm with {} LLM provider(s)", providers_arc.len());
    for (i, p) in providers_arc.iter().enumerate() {
        debug_log!("  Provider {}: {} ({})", i, p.name(), p.model());
    }
    debug_log!("Log file: {}", logger::log_path().display());

    network::spawn_worker(net_action_rx, net_event_tx, providers_arc.clone());

    let tick_rate = config.general.tick_rate_ms;

    // Move provider list into app (separate copy for state tracking)
    let app_providers = llm::build_providers(&config.llm);
    let mut app = App::new(config, net_action_tx, net_event_rx, app_providers);
    let events = EventHandler::new(tick_rate);

    // Initial feed load + HF spotlight + social feed
    app.request_feed_refresh();
    app.request_hf_spotlight();
    app.request_social_refresh();

    while app.running {
        terminal.draw(|frame| ui::render(frame, &mut app))?;
        match events.next()? {
            Event::Tick => {
                app.tick();
                app.drain_network_events();
            }
            Event::Key(key) => app.handle_key(key),
            Event::Mouse(_) | Event::Resize(_, _) => {}
        }
    }

    Ok(())
}
