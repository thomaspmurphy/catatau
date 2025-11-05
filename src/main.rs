use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use std::{io, path::PathBuf};

mod constants;
mod epub;
mod error;
mod ui;

use epub::EpubReader;
use ui::App;

#[derive(Parser)]
#[command(name = "catatau")]
#[command(about = "A terminal EPUB reader")]
struct Cli {
    epub_file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(false)
        .with_writer(std::io::stderr)
        .init();

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let cli = Cli::parse();

    let epub =
        EpubReader::new(&cli.epub_file).map_err(|e| format!("Failed to open EPUB file: {}", e))?;
    let mut app = App::new(epub);

    app.run()
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

    Ok(())
}
