mod app;
mod config;
mod editor;
mod export;
mod input;
mod project;
mod search;
mod session;
mod spell;
mod stats;
mod theme;
mod ui;
mod versions;

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

    // Check for subcommands first
    if args.len() >= 2 {
        match args[1].as_str() {
            "export" => return run_export(&args[2..]),
            "project" => return run_project(&args[2..]),
            _ => {}
        }
    }

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

fn run_export(args: &[String]) -> io::Result<()> {
    let mut input_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut title: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_export_help();
                return Ok(());
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--title" | "-t" => {
                i += 1;
                if i < args.len() {
                    title = Some(args[i].clone());
                }
            }
            arg if !arg.starts_with('-') => {
                input_path = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let input = match input_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: hollow export <file> [OPTIONS]");
            eprintln!("Run 'hollow export --help' for more information.");
            process::exit(1);
        }
    };

    // Default output: same name with .html extension
    let output = output_path.unwrap_or_else(|| input.with_extension("html"));

    let options = export::ExportOptions {
        title,
        css: None,
    };

    match export::export_to_html(&input, &output, &options) {
        Ok(()) => {
            println!("Exported to {}", output.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("Export failed: {}", e);
            process::exit(1);
        }
    }
}

fn run_project(args: &[String]) -> io::Result<()> {
    use project::Project;
    use std::path::Path;

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_project_help();
        return Ok(());
    }

    let subcommand = &args[0];
    let remaining = &args[1..];

    match subcommand.as_str() {
        "info" => {
            let path = remaining.first().map(|s| s.as_str()).unwrap_or(".hollow-project");
            match Project::load(path) {
                Ok(project) => {
                    println!("Project: {}", project.name);
                    println!("Documents: {}", project.documents.len());
                    for doc in &project.documents {
                        println!("  - {}", doc);
                    }
                    if let Ok(stats) = project.stats() {
                        println!("Total words: {}", stats.total_words);
                    }
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to load project: {}", e);
                    process::exit(1);
                }
            }
        }
        "init" => {
            let name = remaining.first().map(|s| s.as_str()).unwrap_or("My Project");
            let path = Path::new(".hollow-project");
            if path.exists() {
                eprintln!("Project file already exists");
                process::exit(1);
            }
            let project = Project::new(name);
            match project.save(Some(path)) {
                Ok(()) => {
                    println!("Created project: {}", name);
                    println!("Add documents with: hollow project add <file>");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to create project: {}", e);
                    process::exit(1);
                }
            }
        }
        "add" => {
            if remaining.is_empty() {
                eprintln!("Usage: hollow project add <file>");
                process::exit(1);
            }
            let path = Path::new(".hollow-project");
            match Project::load(path) {
                Ok(mut project) => {
                    for doc in remaining {
                        project.add_document(doc);
                        println!("Added: {}", doc);
                    }
                    project.save(Some(path)).map_err(|e| {
                        io::Error::other(e.to_string())
                    })?;
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to load project: {}", e);
                    process::exit(1);
                }
            }
        }
        "stats" => {
            let path = remaining.first().map(|s| s.as_str()).unwrap_or(".hollow-project");
            match Project::load(path) {
                Ok(project) => {
                    match project.stats() {
                        Ok(stats) => {
                            println!("Project: {}", project.name);
                            println!("Documents: {}", stats.document_count);
                            println!("Total words: {}", stats.total_words);
                            println!();
                            for (doc, words) in &stats.document_words {
                                println!("  {:>6} words  {}", words, doc);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to get stats: {}", e);
                            process::exit(1);
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    eprintln!("Failed to load project: {}", e);
                    process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Unknown project command: {}", subcommand);
            print_project_help();
            process::exit(1);
        }
    }
}

fn print_project_help() {
    println!(
        r#"hollow project - Manage writing projects

USAGE:
    hollow project <command> [OPTIONS]

COMMANDS:
    init [name]      Create a new project in current directory
    info [file]      Show project information
    add <file>...    Add documents to the project
    stats [file]     Show word count statistics

EXAMPLES:
    hollow project init "My Novel"
    hollow project add chapter1.md chapter2.md
    hollow project stats
    hollow project info path/to/.hollow-project"#
    );
}

fn print_export_help() {
    println!(
        r#"hollow export - Export markdown to HTML

USAGE:
    hollow export <file> [OPTIONS]

ARGS:
    <file>    Markdown file to export

OPTIONS:
    --help, -h           Show this help message
    --output, -o <file>  Output file (default: <input>.html)
    --title, -t <title>  Document title (default: first H1 heading)

EXAMPLES:
    hollow export draft.md
    hollow export draft.md -o published.html
    hollow export draft.md --title "My Essay""#
    );
}

fn print_help() {
    println!(
        r#"hollow - Distraction-free terminal writing environment

USAGE:
    hollow <file> [OPTIONS]
    hollow export <file> [OPTIONS]
    hollow project <command> [OPTIONS]

ARGS:
    <file>    File to edit (created if doesn't exist)

COMMANDS:
    export    Export markdown to HTML
    project   Manage writing projects (init, add, stats)

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

For more information, visit https://github.com/sudokatie/hollow"#
    );
}
