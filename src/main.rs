use clap::Parser;
use std::path::PathBuf;

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
    let cli = Cli::parse();

    let epub =
        EpubReader::new(&cli.epub_file).map_err(|e| format!("Failed to open EPUB file: {}", e))?;
    let mut app = App::new(epub);

    app.run()
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })?;

    Ok(())
}
