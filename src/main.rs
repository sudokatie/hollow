mod app;
mod config;
mod editor;
mod input;
mod search;
mod session;
mod ui;

use std::io::{self, stdout};
use std::path::PathBuf;
use std::process;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

const MIN_COLS: u16 = 40;
const MIN_ROWS: u16 = 10;

use app::App;
use config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Parse arguments
    let mut file_path: Option<PathBuf> = None;
    let mut width_override: Option<usize> = None;
    let mut no_autosave = false;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-v" => {
                println!("hollow {}", VERSION);
                return Ok(());
            }
            "--width" => {
                i += 1;
                if i < args.len() {
                    width_override = args[i].parse().ok();
                }
            }
            "--no-autosave" => {
                no_autosave = true;
            }
            arg if !arg.starts_with('-') => {
                file_path = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Require file path
    let file_path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: hollow <file>");
            eprintln!("Run 'hollow --help' for more information.");
            process::exit(1);
        }
    };

    // Load config with overrides
    let config = Config::load().with_overrides(width_override, no_autosave);

    // Setup panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // Check minimum terminal size (spec 10.2)
    let (cols, rows) = size()?;
    if cols < MIN_COLS || rows < MIN_ROWS {
        eprintln!(
            "Terminal too small: {}x{} (minimum: {}x{})",
            cols, rows, MIN_COLS, MIN_ROWS
        );
        process::exit(1);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run application
    let mut app = App::new(file_path, config)?;
    let result = app.run(&mut terminal);

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn print_help() {
    println!(
        r#"hollow - Distraction-free terminal writing environment

USAGE:
    hollow <file> [OPTIONS]

ARGS:
    <file>    File to edit (created if doesn't exist)

OPTIONS:
    --help, -h          Show this help message
    --version, -v       Show version
    --width <N>         Set text width (default: 80)
    --no-autosave       Disable auto-save

KEY BINDINGS:
    Ctrl+S              Save
    Ctrl+Q              Quit
    Ctrl+G              Toggle status line
    Escape              Enter Navigate mode
    i (in Navigate)     Return to Write mode
    ? (in Navigate)     Show help

For more information, visit https://github.com/katieblackabee/hollow"#
    );
}
